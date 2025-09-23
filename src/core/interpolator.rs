// EN: src/core/interpolator.rs

// EN: src/core/interpolator.rs

use crate::{
    CancellationToken, // We need the executor for <axes::run::...>
    models::{Command as ProjectCommand, ResolvedConfig, Runnable},
    system::executor,
};
use anyhow::{Context, Result, anyhow};
use regex::Regex;
use std::collections::HashSet;

use colored::Colorize;

const MAX_RECURSION_DEPTH: u32 = 32;

#[derive(Clone)]
pub struct Interpolator<'a> {
    config: &'a ResolvedConfig,
    // For direct cycle detection (a -> b -> a)
    recursion_stack: HashSet<String>,
    // For runaway recursion protection (a -> b -> c -> ...)
    recursion_depth: u32,
}

impl<'a> Interpolator<'a> {
    pub fn new(config: &'a ResolvedConfig) -> Self {
        Self {
            config,
            recursion_stack: HashSet::new(),
            recursion_depth: 0,
        }
    }

    /// Creates a new interpolator for a deeper recursion level.
    fn new_for_recursion(&self) -> Self {
        Self {
            config: self.config,
            recursion_stack: self.recursion_stack.clone(),
            recursion_depth: self.recursion_depth + 1,
        }
    }

    /// Recursively expands all `<axes::...>` tokens in a string.
    pub fn expand_string(
        &mut self,
        template: &str,
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        // Protection against runaway recursion.
        if self.recursion_depth >= MAX_RECURSION_DEPTH {
            return Err(anyhow!(
                "Maximum recursion depth ({}) exceeded during expansion. Check for indirect cycles.",
                MAX_RECURSION_DEPTH
            ));
        }

        let mut current_str = template.to_string();
        let re = Regex::new(r"<axes::(.+?)>").unwrap();

        while let Some(captures) = re.captures(&current_str.clone()) {
            let full_match = captures.get(0).unwrap().as_str();
            let token_path = captures.get(1).unwrap().as_str();

            // Create a new interpolator for the sub-expansion to manage its own depth.
            let mut sub_interpolator = self.new_for_recursion();
            let expanded_value = sub_interpolator.expand_token(token_path, cancellation_token)?;
            current_str = current_str.replace(full_match, &expanded_value);
        }

        Ok(current_str)
    }

    /// Expands a single token path (e.g., "name" or "scripts::test").
    fn expand_token(
        &mut self,
        token_path: &str,
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        let parts: Vec<&str> = token_path.split("::").collect();

        if parts.len() > 1 {
            return self.expand_qualified_token(&parts, cancellation_token);
        }

        let key = parts[0];

        // Precedence: Reserved -> Vars -> Scripts
        if let Some(value) = self.get_reserved_metadata(key) {
            return Ok(value);
        }
        if self.config.vars.contains_key(key) {
            return self.expand_qualified_token(&["vars", key], cancellation_token);
        }
        if self.config.scripts.contains_key(key) {
            return self.expand_qualified_token(&["scripts", key], cancellation_token);
        }

        Err(anyhow!("<axes::{}> not found.", token_path))
    }

    fn expand_qualified_token(
        &mut self,
        parts: &[&str],
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        match parts.first() {
            Some(&"vars") => {
                let key = parts
                    .get(1)
                    .ok_or_else(|| anyhow!("<axes::vars::> is missing a key."))?;
                self.config
                    .vars
                    .get(*key)
                    .cloned()
                    .ok_or_else(|| anyhow!("<axes::vars::{}> not found.", key))
            }
            Some(&"env") => {
                let key = parts
                    .get(1)
                    .ok_or_else(|| anyhow!("<axes::env::> is missing a key."))?;
                self.config
                    .env
                    .get(*key)
                    .cloned()
                    .ok_or_else(|| anyhow!("<axes::env::{}> not found.", key))
            }
            Some(&"scripts") => {
                let key = parts
                    .get(1)
                    .ok_or_else(|| anyhow!("<axes::scripts::> is missing a key."))?;
                self.expand_script(key, cancellation_token)
            }
            // NOTE: The new, powerful run command
            Some(&"run") => {
                let sub_path = &parts[1..];
                if sub_path.is_empty() {
                    return Err(anyhow!(
                        "<axes::run::> must be followed by a path or script."
                    ));
                }
                self.expand_run(sub_path, cancellation_token)
            }
            Some(&key) if self.get_reserved_metadata(key).is_some() => {
                Ok(self.get_reserved_metadata(key).unwrap())
            }
            _ => Err(anyhow!(
                "Unknown token namespace: '<axes::{}::...>'.",
                parts.join("::")
            )),
        }
    }

    /// Expands the content of an internal script, with cycle detection.
    fn expand_script(
        &mut self,
        script_name: &str,
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        if self.recursion_stack.contains(script_name) {
            let path = self
                .recursion_stack
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(anyhow!(
                "Cyclical script reference detected: {} -> {}",
                path,
                script_name
            ));
        }
        self.recursion_stack.insert(script_name.to_string());

        let command_def = self
            .config
            .scripts
            .get(script_name)
            .ok_or_else(|| anyhow!("<axes::scripts::{}> not found.", script_name))?;

        let raw_content = match command_def {
            ProjectCommand::Simple(s) => s.clone(),
            ProjectCommand::Sequence(s) => s.join(" && "),
            ProjectCommand::Extended(ext) => match &ext.run {
                Runnable::Single(s) => s.clone(),
                Runnable::Sequence(s) => s.join(" && "),
            },
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
                match runnable {
                    Runnable::Single(s) => s.clone(),
                    Runnable::Sequence(s) => s.join(" && "),
                }
            }
        };

        // Recursively expand the content of the script itself.
        let expanded_content = self.expand_string(&raw_content, cancellation_token)?;

        self.recursion_stack.remove(script_name);

        Ok(expanded_content)
    }

    /// Executes a command and returns its output for substitution.
    fn expand_run(
        &mut self,
        sub_path: &[&str],
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        let command_to_run =
            if sub_path.first() == Some(&"scripts") {
                // Case: <axes::run::scripts::my_script>
                let script_name = sub_path
                    .get(1)
                    .ok_or_else(|| anyhow!("<axes::run::scripts::> is missing a key."))?;
                self.expand_script(script_name, cancellation_token)?
            } else {
                // Case: <axes::run::./get_version.sh>
                sub_path.join("::")
            };

        // Recursively expand any tokens *within* the command to be run.
        let final_command = self.expand_string(&command_to_run, cancellation_token)?;

        println!(
            "    {}",
            format!(" script Executing for substitution: '{}'", final_command).dimmed()
        );

        let output = executor::execute_and_capture_output(
            &final_command,
            &self.config.project_root,
            &self.config.env,
            cancellation_token,
        )
        .with_context(|| format!("Execution of '{}' for substitution failed.", final_command))?;

        // Clean up the output by trimming whitespace and newlines.
        Ok(output.trim().to_string())
    }

    /// Helper to get a value from the project's reserved metadata.
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
