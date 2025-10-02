// src/core/task_executor.rs

use crate::{
    core::parameters::ArgResolver,
    models::{CommandAction, ResolvedConfig, RunSpec, Task, TemplateComponent},
    system::executor,
};
use anyhow::{Context, Result, anyhow};
use colored::*;
use rayon::prelude::*;
use std::fmt::Write;

/// "Renders" a template of components (the `action` part of a `CommandExecution`)
/// into a final, executable string. It resolves all dynamic and static tokens.
pub fn assemble_final_command(
    template: &[TemplateComponent],
    config: &ResolvedConfig,
    resolver: &ArgResolver,
) -> Result<String> {
    let mut final_command = String::new();
    for component in template {
        match component {
            TemplateComponent::Literal(s) => final_command.push_str(s),
            TemplateComponent::Parameter(def) => {
                let value = resolver
                    .get_specific_value(&def.original_token)
                    .ok_or_else(|| {
                        // This should be unreachable if the ArgResolver is built correctly from the Task.
                        anyhow!(
                            "Internal logic failure: resolved value not found for token '{}'",
                            def.original_token
                        )
                    })?;
                final_command.push_str(value);
            }
            TemplateComponent::GenericParams => {
                final_command.push_str(resolver.get_generic_value());
            }
            TemplateComponent::Run(spec) => {
                let command_to_run = match spec {
                    RunSpec::Literal(cmd) => {
                        // A `run` literal might itself contain static tokens like `<axes::path>`.
                        // We need to expand them before execution. We do this by recursively
                        // calling this same function on a temporary template.
                        let temp_template = vec![TemplateComponent::Literal(cmd.clone())];
                        assemble_final_command(&temp_template, config, resolver)?
                    }
                };
                let output = executor::execute_and_capture_output(
                    &command_to_run,
                    &config.project_root,
                    &config.env,
                )?;
                final_command.push_str(output.trim());
            }
            TemplateComponent::Path => {
                final_command.push_str(&config.project_root.to_string_lossy())
            }
            TemplateComponent::Name => final_command.push_str(&config.qualified_name),
            TemplateComponent::Uuid => final_command.push_str(&config.uuid.to_string()),
            TemplateComponent::Version => {
                final_command.push_str(config.version.as_deref().unwrap_or(""))
            }
        }
    }
    Ok(final_command)
}

/// Executes a complete `Task` object, handling sequential and parallel commands,
/// as well as distinguishing between shell execution and direct printing.
pub fn execute_task(task: &Task, config: &ResolvedConfig, resolver: &ArgResolver) -> Result<()> {
    let mut parallel_batch: Vec<(String, bool, bool)> = Vec::new();

    for command_exec in &task.commands {
        // First, check if there's a sequential barrier. If the current command is not parallel,
        // we must execute any pending parallel batch before proceeding.
        if !command_exec.run_in_parallel && !parallel_batch.is_empty() {
            execute_parallel_batch(&parallel_batch, config)?;
            parallel_batch.clear();
        }

        // Now, process the current command execution.
        match &command_exec.action {
            CommandAction::Execute(template) => {
                let rendered_string = assemble_final_command(template, config, resolver)?;
                let trimmed_string = rendered_string.trim();
                if trimmed_string.is_empty() {
                    continue; // Skip empty shell commands.
                }

                if command_exec.run_in_parallel {
                    parallel_batch.push((
                        trimmed_string.to_string(),
                        command_exec.ignore_errors,
                        command_exec.silent_mode,
                    ));
                } else {
                    execute_single_command(
                        trimmed_string,
                        command_exec.ignore_errors,
                        command_exec.silent_mode,
                        config,
                    )?;
                }
            }
            CommandAction::Print(template) => {
                // Print actions are always sequential and cannot be silenced.
                let rendered_string = assemble_final_command(template, config, resolver)?;
                println!("{}", rendered_string);
            }
        }
    }

    // Execute the final parallel batch if it exists.
    if !parallel_batch.is_empty() {
        execute_parallel_batch(&parallel_batch, config)?;
    }

    Ok(())
}

/// Executes a single sequential command.
fn execute_single_command(
    command_str: &str,
    ignore_errors: bool,
    silent: bool,
    config: &ResolvedConfig,
) -> Result<()> {
    if !silent {
        println!("\n→ {}", command_str.green());
    }

    executor::execute_command(
        command_str,
        ignore_errors,
        &config.project_root,
        &config.env,
    )?;

    Ok(())
}

/// Prints and executes a batch of commands in parallel.
fn execute_parallel_batch(batch: &[(String, bool, bool)], config: &ResolvedConfig) -> Result<()> {
    let is_globally_silent = batch.iter().all(|(_, _, silent)| *silent);

    if !is_globally_silent {
        let mut header_block = String::new();
        // `unwrap()` is safe here because writing to a String never fails.
        writeln!(
            header_block,
            "\n{} {}",
            "┌─".dimmed(),
            format!("Running {} commands in parallel...", batch.len()).blue()
        )
        .unwrap();

        for (command_str, _, silent) in batch.iter() {
            if !*silent {
                writeln!(header_block, "{} {}", "├─˃".dimmed(), command_str.green()).unwrap();
            }
        }
        print!("{}", header_block);
    }

    // --- Execute all commands in parallel ---
    let results: Result<Vec<()>> = batch
        .par_iter()
        .map(|(command_str, ignore_errors, _)| {
            // We already printed, silent is for UI
            executor::execute_command(
                command_str,
                *ignore_errors,
                &config.project_root,
                &config.env,
            )
            .map_err(anyhow::Error::from)
        })
        .collect();

    results.with_context(|| "A command in the parallel batch failed.")?;

    if !is_globally_silent {
        println!("{}{}", "└─".dimmed(), " Parallel batch completed.".blue());
    }

    Ok(())
}
