// EN: src/cli/handlers/run.rs

use anyhow::{Context, Result, anyhow};
use colored::*;
use rayon::prelude::*;

use crate::{
    CancellationToken,
    core::{arg_parser::ParsedArgs, interpolator::Interpolator},
    models::{Command as ProjectCommand, ResolvedConfig},
    system::executor,
};

use super::commons;
use clap::Parser;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct RunArgs {
    /// The project context to run the script in.
    context: String,
    /// The name of the script to run.
    script: String,
    /// Parameters to pass to the script.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

/// Main entry point for the 'run' command.
pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    let run_args = RunArgs::try_parse_from(&args)?;
    let config = commons::resolve_config_from_context_or_session(
        Some(run_args.context),
        cancellation_token,
    )?;

    let script_key = &run_args.script;
    let params = &run_args.params;

    let executor = CommandExecutor::new(config, cancellation_token);

    println!(
        "\n▶️  Running script '{}' for project '{}'...",
        script_key.cyan(),
        executor.config.qualified_name.yellow()
    );
    executor.run_script(script_key, params)?;

    println!(
        "\n✅ {} Script '{}' completed successfully.",
        "Success:".green().bold(),
        script_key.cyan()
    );
    Ok(())
}

/// A struct to hold the state and configuration for a single `axes run` invocation.
struct CommandExecutor<'a> {
    config: ResolvedConfig,
    cancellation_token: &'a CancellationToken,
}

impl<'a> CommandExecutor<'a> {
    fn new(config: ResolvedConfig, cancellation_token: &'a CancellationToken) -> Self {
        Self {
            config,
            cancellation_token,
        }
    }

    /// The top-level entry point for executing a script by name.
    fn run_script(&self, script_name: &str, params: &[String]) -> Result<()> {
        self.execute_internal_script(script_name, params)
    }

    /// The recursive engine for running scripts defined in `axes.toml`.
    fn execute_internal_script(&self, script_name: &str, cli_params: &[String]) -> Result<()> {
        let command_def = self
            .config
            .scripts
            .get(script_name)
            .ok_or_else(|| anyhow!(t!("run.error.script_not_found"), script = script_name))?;

        let command_list = self.get_command_list_from_def(command_def, script_name)?;
        self.process_command_list(&command_list, cli_params)
    }

    /// Processes a list of command strings, handling recursion, parallelism, and execution.
    fn process_command_list(&self, command_list: &[String], cli_params: &[String]) -> Result<()> {
        let mut parallel_batch = Vec::new();

        for command_template in command_list {
            commons::check_for_cancellation(self.cancellation_token)?;
            let is_parallel = command_template.starts_with('>');
            let template = if is_parallel {
                command_template[1..].trim()
            } else {
                command_template.as_str()
            };

            if is_parallel {
                parallel_batch.push(template.to_string());
            } else {
                if !parallel_batch.is_empty() {
                    self.execute_parallel_batch(&parallel_batch, cli_params)?;
                    parallel_batch.clear();
                }
                self.execute_template(template, cli_params)?;
            }
        }

        if !parallel_batch.is_empty() {
            self.execute_parallel_batch(&parallel_batch, cli_params)?;
        }

        Ok(())
    }

    /// Executes a batch of command templates in parallel using Rayon.
    fn execute_parallel_batch(&self, batch: &[String], cli_params: &[String]) -> Result<()> {
        println!(
            "{}",
            format!("⚡ Running {} scripts in parallel...", batch.len()).blue()
        );

        let results: Result<Vec<()>> = batch
            .par_iter()
            .map(|template| self.execute_template(template, cli_params))
            .collect();

        results.with_context(|| "A command in the parallel batch failed.")?;
        println!("{}", "⚡ Parallel batch completed.".blue());
        Ok(())
    }

    /// The core logic that processes a single template string from a script.
    /// It distinguishes between pure script inclusion and external command execution.
    fn execute_template(&self, template: &str, cli_params: &[String]) -> Result<()> {
        // Pre-parse the CLI arguments for this specific template execution.
        let mut parsed_args = ParsedArgs::new(cli_params)?;

        // Check for pure script inclusion first.
        let re = regex::Regex::new(r"^\s*<axes::scripts::([^>]+)>\s*$").unwrap();
        if let Some(caps) = re.captures(template) {
            let script_name = &caps[1];
            // If it's a pure inclusion, we recurse, passing the *original* cli_params down.
            return self.execute_internal_script(script_name, cli_params);
        }

        // --- Standard Command Execution with Two-Pass Interpolation ---

        // Pass 1: Expand all explicit, config-based tokens.
        let mut interpolator = Interpolator::new(&self.config, &mut parsed_args);
        let expanded_with_placeholder =
            interpolator.expand_string(template, self.cancellation_token)?;

        // Pass 2: Expand the generic `<axes::params>` token using remaining args.
        let final_command = if expanded_with_placeholder.contains("<axes::params>") {
            let remaining_args = parsed_args.consume_remaining();
            expanded_with_placeholder.replace("<axes::params>", &remaining_args)
        } else {
            expanded_with_placeholder
        };

        // Final Check: Ensure all arguments passed by the user were consumed.
        if !parsed_args.all_consumed() {
            println!("{:?} - {}", parsed_args, final_command);
            return Err(anyhow!(t!("run.error.params_not_used")));
        }

        let trimmed_command = final_command.trim();
        if trimmed_command.is_empty() {
            return Ok(());
        }

        println!("\n> {}", trimmed_command.green());
        executor::execute_command(
            trimmed_command,
            &self.config.project_root,
            &self.config.env,
            self.cancellation_token,
        )?;
        Ok(())
    }

    /// Helper to extract the initial list of command strings from a `ProjectCommand` enum.
    fn get_command_list_from_def(
        &self,
        command_def: &ProjectCommand,
        script_key: &str,
    ) -> Result<Vec<String>> {
        let runnable = match command_def {
            ProjectCommand::Simple(s) => return Ok(vec![s.clone()]),
            ProjectCommand::Sequence(s) => return Ok(s.clone()),
            ProjectCommand::Extended(ext) => &ext.run,
            ProjectCommand::Platform(pc) => {
                let os_specific = if cfg!(target_os = "windows") {
                    pc.windows.as_ref()
                } else if cfg!(target_os = "linux") {
                    pc.linux.as_ref()
                } else if cfg!(target_os = "macos") {
                    pc.macos.as_ref()
                } else {
                    None
                };

                os_specific.or(pc.default.as_ref()).ok_or_else(|| {
                    anyhow!(
                        "Script '{}' has no platform implementation for the current OS and no 'default'.",
                        script_key
                    )
                })?
            }
        };

        match runnable {
            crate::models::Runnable::Single(s) => Ok(vec![s.clone()]),
            crate::models::Runnable::Sequence(s) => Ok(s.clone()),
        }
    }
}
