// EN: src/cli/handlers/run.rs

use anyhow::{Context, Result, anyhow};
use colored::*;
use rayon::prelude::*;

use crate::{
    CancellationToken,
    core::interpolator::Interpolator,
    models::{Command as ProjectCommand, ResolvedConfig},
    system::executor,
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
    let config = commons::resolve_config_from_context_or_session(
        Some(run_args.context),
        cancellation_token,
    )?;

    // 3. Parse script name and parameters from arguments.
    let script_key = &run_args.script;
    let params = &run_args.params;

    // 4. Create the top-level executor for this run.
    let executor = CommandExecutor::new(config, cancellation_token);

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

    fn run_script(&self, script_name: &str, params: &[String]) -> Result<()> {
        let mut interpolator = Interpolator::new(&self.config);
        self.execute_internal_script(script_name, params, &mut interpolator)
    }

    fn execute_internal_script(
        &self,
        script_name: &str,
        cli_params: &[String],
        interpolator: &mut Interpolator,
    ) -> Result<()> {
        let command_def = self
            .config
            .commands
            .get(script_name)
            .ok_or_else(|| anyhow!(t!("run.error.script_not_found"), script = script_name))?;

        let command_list = self.get_command_list_from_def(command_def, script_name)?;

        // NOTE: Pass the cancellation token down.
        self.process_command_list(&command_list, cli_params, interpolator)
    }

    fn process_command_list(
        &self,
        command_list: &[String],
        cli_params: &[String],
        interpolator: &mut Interpolator,
    ) -> Result<()> {
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
                    self.execute_parallel_batch(&parallel_batch, cli_params, interpolator)?;
                    parallel_batch.clear();
                }
                // NOTE: Pass the cancellation token down.
                self.execute_template(template, cli_params, interpolator)?;
            }
        }

        if !parallel_batch.is_empty() {
            self.execute_parallel_batch(&parallel_batch, cli_params, interpolator)?;
        }

        Ok(())
    }

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
                let mut task_interpolator = interpolator.clone();
                // NOTE: Pass the cancellation token to each parallel task.
                self.execute_template(template, cli_params, &mut task_interpolator)
            })
            .collect();

        results.with_context(|| "A command in the parallel batch failed.")?;
        println!("{}", "⚡ Parallel batch completed.".to_string().blue());
        Ok(())
    }

    fn execute_template(
        &self,
        template: &str,
        cli_params: &[String],
        interpolator: &mut Interpolator,
    ) -> Result<()> {
        // NOTE: Corrected regex to handle whitespace correctly
        let re = regex::Regex::new(r"^\s*<axes::(commands|scripts)::([^>]+)>\s*$").unwrap();
        if let Some(caps) = re.captures(template) {
            let script_name = &caps[2];
            return self.execute_internal_script(script_name, cli_params, interpolator);
        }

        let expanded_command = interpolator.expand_string(template, self.cancellation_token)?;
        let final_command = if !cli_params.is_empty() {
            format!("{} {}", expanded_command, cli_params.join(" "))
        } else {
            expanded_command
        };

        let trimmed_command = final_command.trim();
        if trimmed_command.is_empty() {
            return Ok(());
        }

        println!("\n> {}", trimmed_command.green());
        // NOTE: Pass the cancellation token to the external command executor.
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
