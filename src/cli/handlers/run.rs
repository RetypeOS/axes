// EN: src/cli/handlers/run.rs

use anyhow::{Context, Result, anyhow};
use colored::*;
use rayon::prelude::*;

use crate::{
    core::interpolator::Interpolator,
    models::{Command as ProjectCommand, ResolvedConfig},
    system::executor, CancellationToken,
};

use clap::Parser;

use super::commons;

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
    // 1. Parse args.
    let run_args = RunArgs::try_parse_from(&args)?;

    // 2. Solve config.
    let config = commons::resolve_config_from_context_or_session(Some(run_args.context), cancellation_token)?;

    // 3. Parse script name and parameters from arguments.
    let script_key = &run_args.script;
    let params = &run_args.params;

    // 4. Create the top-level executor for this run.
    let executor = CommandExecutor::new(config);

    // 5. Start the execution chain.
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
struct CommandExecutor {
    config: ResolvedConfig,
}

impl CommandExecutor {
    fn new(config: ResolvedConfig) -> Self {
        Self { config }
    }

    /// The top-level entry point for executing a script by its name.
    /// It initializes the interpolator for the entire run.
    fn run_script(&self, script_name: &str, params: &[String]) -> Result<()> {
        let mut interpolator = Interpolator::new(&self.config);
        self.execute_internal_script(script_name, params, &mut interpolator)
    }

    /// The recursive engine for running scripts defined in `axes.toml`.
    /// It fetches the script's definition and passes its commands to be processed.
    fn execute_internal_script(
        &self,
        script_name: &str,
        cli_params: &[String],
        interpolator: &mut Interpolator,
    ) -> Result<()> {
        let command_def = self
            .config
            .commands // TODO: Rename to `scripts` in a future refactor
            .get(script_name)
            .ok_or_else(|| anyhow!(t!("run.error.script_not_found"), script = script_name))?;

        let command_list = self.get_command_list_from_def(command_def, script_name)?;

        self.process_command_list(&command_list, cli_params, interpolator)
    }

    /// Processes a list of command strings, handling recursion, parallelism, and execution.
    fn process_command_list(
        &self,
        command_list: &[String],
        cli_params: &[String],
        interpolator: &mut Interpolator,
    ) -> Result<()> {
        let mut parallel_batch = Vec::new();

        for command_template in command_list {
            let is_parallel = command_template.starts_with('>');
            let template = if is_parallel {
                command_template[1..].trim()
            } else {
                command_template.as_str()
            };

            if is_parallel {
                parallel_batch.push(template.to_string());
            } else {
                // Execute any pending parallel batch before a sequential command.
                if !parallel_batch.is_empty() {
                    self.execute_parallel_batch(&parallel_batch, cli_params, interpolator)?;
                    parallel_batch.clear();
                }
                // Execute the sequential command.
                self.execute_template(template, cli_params, interpolator)?;
            }
        }

        // Execute any final parallel batch at the end of the list.
        if !parallel_batch.is_empty() {
            self.execute_parallel_batch(&parallel_batch, cli_params, interpolator)?;
        }

        Ok(())
    }

    /// Executes a batch of command templates in parallel using Rayon.
    fn execute_parallel_batch(
        &self,
        batch: &[String],
        cli_params: &[String],
        interpolator: &mut Interpolator,
    ) -> Result<()> {
        println!(
            "{}",
            format!("⚡ Running {} commands in parallel...", batch.len()).blue()
        );

        let results: Result<Vec<()>> = batch
            .par_iter()
            .map(|template| {
                // Each parallel task needs its own mutable interpolator state to avoid data races.
                // Cloning is efficient as the config is behind a reference and the stack is small.
                let mut task_interpolator = interpolator.clone();
                self.execute_template(template, cli_params, &mut task_interpolator)
            })
            .collect();

        results.with_context(|| "A command in the parallel batch failed.")?;
        println!("{}", format!("⚡ Parallel batch completed.").blue());
        Ok(())
    }

    /// The core logic that processes a single template string from a script.
    /// It distinguishes between pure script inclusion and external command execution.
    fn execute_template(
        &self,
        template: &str,
        cli_params: &[String],
        interpolator: &mut Interpolator,
    ) -> Result<()> {
        // Regex to detect if a string is a pure, single script inclusion.
        // TODO: Change "commands" to "scripts" in a future refactor
        let re = regex::Regex::new(r"^\s*<axes::(commands|scripts)::([^>]+)>\s*$").unwrap();
        if let Some(caps) = re.captures(template) {
            let script_name = &caps[2];
            // This is a pure recursion. We pass the interpolator down to maintain the cycle detection stack.
            // CLI params are passed along, as this is a direct inclusion.
            return self.execute_internal_script(script_name, cli_params, interpolator);
        }

        // If not a pure inclusion, it's a command to be expanded and executed externally.
        let expanded_command = interpolator.expand_string(template)?;

        // CLI params are appended only to external commands, not to internal script definitions.
        let final_command = if !cli_params.is_empty() {
            format!("{} {}", expanded_command, cli_params.join(" "))
        } else {
            expanded_command
        };

        let trimmed_command = final_command.trim();
        if trimmed_command.is_empty() {
            return Ok(()); // Nothing to execute (e.g., an empty line in a script)
        }

        println!("\n> {}", trimmed_command.green());
        executor::execute_command(trimmed_command, &self.config.project_root, &self.config.env)?;
        Ok(())
    }

    /// Helper to extract the initial list of command strings from a `ProjectCommand` enum.
    fn get_command_list_from_def<'a>(
        &self,
        command_def: &'a ProjectCommand,
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
