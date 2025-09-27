// EN: src/core/interpolator.rs

use crate::{
    CancellationToken,
    core::arg_parser::ParsedArgs,
    models::{Command as ProjectCommand, ResolvedConfig, Runnable},
    system::executor,
};
use anyhow::{Result, anyhow};
use colored::*;
use regex::Regex;
use std::collections::HashSet;

const MAX_RECURSION_DEPTH: u32 = 32;

// NOTE: Interpolator ya no tiene lifetimes complejos ni estado mutable de recursión.
// Es una estructura simple que contiene el contexto de la interpolación.
pub struct Interpolator<'a, 'p> {
    config: &'a ResolvedConfig,
    parsed_args: &'p mut ParsedArgs<'a>,
}

impl<'a, 'p> Interpolator<'a, 'p> {
    pub fn new(config: &'a ResolvedConfig, parsed_args: &'p mut ParsedArgs<'a>) -> Self {
        Self {
            config,
            parsed_args,
        }
    }

    /// The main entry point. It kicks off the recursive expansion.
    pub fn expand_string(
        &mut self,
        template: &str,
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        self.expand_string_recursive(template, &mut HashSet::new(), 0, cancellation_token)
    }

    /// The recursive engine. The state (`recursion_stack`, `depth`) is passed as arguments.
    fn expand_string_recursive(
        &mut self,
        template: &str,
        recursion_stack: &mut HashSet<String>,
        depth: u32,
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        if depth >= MAX_RECURSION_DEPTH {
            return Err(anyhow!(
                t!("interpolator.error.max_depth"),
                depth = MAX_RECURSION_DEPTH
            ));
        }

        let mut current_str = template.to_string();
        let re = Regex::new(r"<axes::(.+?)>").unwrap();

        loop {
            let mut expansion_made = false;
            let mut next_str = String::with_capacity(current_str.len());
            let mut last_match_end = 0;

            for caps in re.captures_iter(&current_str) {
                let full_match = caps.get(0).unwrap();
                let token_path = caps.get(1).unwrap().as_str();

                next_str.push_str(&current_str[last_match_end..full_match.start()]);

                if token_path == "params" {
                    next_str.push_str(full_match.as_str());
                } else {
                    let expanded_value =
                        self.expand_token(token_path, recursion_stack, depth, cancellation_token)?;
                    next_str.push_str(&expanded_value);
                    expansion_made = true;
                }

                last_match_end = full_match.end();
            }

            next_str.push_str(&current_str[last_match_end..]);

            current_str = next_str;

            if !expansion_made {
                break;
            }
        }

        Ok(current_str)
    }

    /// Expands a single token path by delegating to the correct handler.
    fn expand_token(
        &mut self,
        token_path: &str,
        recursion_stack: &mut HashSet<String>,
        depth: u32,
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        if let Some(param_spec) = token_path.strip_prefix("params::") {
            return self.expand_params_token(param_spec);
        }

        let parts: Vec<&str> = token_path.split("::").collect();
        if parts.len() > 1 {
            return self.expand_qualified_token(&parts, recursion_stack, depth, cancellation_token);
        }

        let key = parts[0];
        if let Some(value) = self.get_reserved_metadata(key) {
            return Ok(value);
        }
        if self.config.vars.contains_key(key) {
            return self.expand_qualified_token(
                &["vars", key],
                recursion_stack,
                depth,
                cancellation_token,
            );
        }
        if self.config.scripts.contains_key(key) {
            return self.expand_qualified_token(
                &["scripts", key],
                recursion_stack,
                depth,
                cancellation_token,
            );
        }

        Err(anyhow!(
            t!("interpolator.error.token_not_found"),
            token = key
        ))
    }

    /// Handles tokens like `0`, `rel='--release'`, or `target`.
    fn expand_params_token(&mut self, param_spec: &str) -> Result<String> {
        if let Some((name, value)) = param_spec.split_once('=') {
            // Mapped flag: <axes::params::rel='--release'>
            let clean_value = value.trim().trim_matches(|c| c == '\'' || c == '"');
            Ok(self
                .parsed_args
                .get_mapped_flag(name.trim(), clean_value)
                .to_string())
        } else if let Ok(index) = param_spec.parse::<usize>() {
            // Positional: <axes::params::0>
            Ok(self.parsed_args.get_positional(index).to_string())
        } else {
            // Direct/Passthrough flag: <axes::params::target>
            // We assume `param_spec` is the name of the flag.
            Ok(self
                .parsed_args
                .consume_named_passthrough(param_spec.trim()))
        }
    }

    fn expand_qualified_token(
        &mut self,
        parts: &[&str],
        recursion_stack: &mut HashSet<String>,
        depth: u32,
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        match parts.first() {
            // --- TODO: Aplicar correctamente esto con el nuevo interpolator.
            //Some(&"vars") => self
            //    .config
            //    .vars
            //    .get(parts[1])
            //    .cloned()
            //    .ok_or_else(|| anyhow!("Var not found")),
            Some(&"env") => self
                .config
                .env
                .get(parts[1])
                .cloned()
                .ok_or_else(|| anyhow!("Env not found")),
            Some(&"scripts") => {
                self.expand_script(parts[1], recursion_stack, depth, cancellation_token)
            }
            Some(&"run") => {
                self.expand_run(&parts[1..], recursion_stack, depth, cancellation_token)
            }
            Some(&key) if self.get_reserved_metadata(key).is_some() => {
                Ok(self.get_reserved_metadata(key).unwrap())
            }
            _ => Err(anyhow!(
                t!("interpolator.error.unknown_namespace"),
                ns = parts.join("::")
            )),
        }
    }

    fn expand_script(
        &mut self,
        script_name: &str,
        recursion_stack: &mut HashSet<String>,
        depth: u32,
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        if recursion_stack.contains(script_name) {
            let path = recursion_stack
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(anyhow!(
                t!("interpolator.error.cycle"),
                path = path,
                script = script_name
            ));
        }
        recursion_stack.insert(script_name.to_string());

        let command_def = self
            .config
            .scripts
            .get(script_name)
            .ok_or_else(|| anyhow!("Script not found"))?;
        let raw_content = get_raw_content_from_def(command_def, script_name)?;

        let expanded_content =
            self.expand_string_recursive(&raw_content, recursion_stack, depth, cancellation_token)?;

        recursion_stack.remove(script_name);
        Ok(expanded_content)
    }

    fn expand_run(
        &mut self,
        sub_path: &[&str],
        recursion_stack: &mut HashSet<String>,
        depth: u32,
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        let command_to_run = if sub_path.first() == Some(&"scripts") {
            let script_name = sub_path
                .get(1)
                .ok_or_else(|| anyhow!("<axes::run::scripts::> missing key"))?;
            self.expand_script(script_name, recursion_stack, depth, cancellation_token)?
        } else {
            sub_path.join("::")
        };

        let final_command = self.expand_string_recursive(
            &command_to_run,
            recursion_stack,
            depth,
            cancellation_token,
        )?;

        println!(
            "    {}",
            format!(" Executing for substitution: '{}'", final_command).dimmed()
        );
        let output = executor::execute_and_capture_output(
            &final_command,
            &self.config.project_root,
            &self.config.env,
            cancellation_token,
        )?;
        Ok(output.trim().to_string())
    }

    fn get_reserved_metadata(&self, key: &str) -> Option<String> {
        match key {
            "name" => Some(self.config.qualified_name.clone()),
            "uuid" => Some(self.config.uuid.to_string()),
            // Always return a clean, canonical path.
            "path" => Some(
                dunce::simplified(&self.config.project_root)
                    .to_string_lossy()
                    .to_string(),
            ),
            "version" => self.config.version.clone(),
            _ => None,
        }
    }
}

/// Helper to extract the raw string content from a ProjectCommand.
fn get_raw_content_from_def(command_def: &ProjectCommand, script_name: &str) -> Result<String> {
    match command_def {
        ProjectCommand::Simple(s) => Ok(s.clone()),
        ProjectCommand::Sequence(s) => Ok(s.join(" && ")),
        ProjectCommand::Extended(ext) => get_raw_content_from_runnable(&ext.run),
        ProjectCommand::Platform(pc) => {
            let os_runnable = if cfg!(target_os = "windows") {
                pc.windows.as_ref()
            } else if cfg!(target_os = "linux") {
                pc.linux.as_ref()
            } else if cfg!(target_os = "macos") {
                pc.macos.as_ref()
            } else {
                None
            };
            let runnable = os_runnable.or(pc.default.as_ref()).ok_or_else(|| {
                anyhow!("Script '{}' has no platform implementation.", script_name)
            })?;
            get_raw_content_from_runnable(runnable)
        }
    }
}

fn get_raw_content_from_runnable(runnable: &Runnable) -> Result<String> {
    match runnable {
        Runnable::Single(s) => Ok(s.clone()),
        Runnable::Sequence(s) => Ok(s.join(" && ")),
    }
}
