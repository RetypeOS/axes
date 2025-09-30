// src/core/task_executor.rs

use crate::{
    core::parameters::ArgResolver,
    models::{CacheableValue, ResolvedConfig, RunSpec, Task, TemplateComponent},
    system::executor,
};
use anyhow::{Context, Result, anyhow};
use colored::*;
use rayon::prelude::*;

///
/// Assembles the final command string for a single `CommandExecution` by replacing
/// parameter tokens with their resolved values from the `ArgResolver`.
///
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
                    RunSpec::Literal(cmd) => cmd.clone(),
                    RunSpec::Script(script_name) => {
                        // Find the script in the config and flatten it to a string.
                        let cacheable = config.scripts.get(script_name).ok_or_else(|| {
                            anyhow!("Script '{}' referenced in <axes::run::...> not found.", script_name)
                        })?;
                        match cacheable {
                            CacheableValue::Raw(fc) => fc.command_lines.join(" && "),
                            // This case is unlikely if validation passed, but we handle it.
                            CacheableValue::Expanded(task) => task
                                .commands
                                .iter()
                                .map(|cmd| cmd.template.iter().map(crate::core::config_resolver::template_component_to_string).collect::<String>())
                                .collect::<Vec<_>>()
                                .join(" && "),
                        }
                    }
                };
 
                let output = executor::execute_and_capture_output(
                    &command_to_run,
                    &config.project_root,
                    &config.env,
                )?;
 
                // Clean the output (remove trailing newlines/spaces) before injection.
                final_command.push_str(output.trim());
            }
        }
    }
    Ok(final_command)
}

///
/// Executes a complete `Task` object, handling sequential and parallel commands.
/// This is the main execution engine used by `run`, `start`, `open`, etc.
///
pub fn execute_task(task: &Task, config: &ResolvedConfig, resolver: &ArgResolver) -> Result<()> {
    let mut parallel_batch: Vec<String> = Vec::new();

    for command_exec in &task.commands {
        let final_command_str = assemble_final_command(&command_exec.template, config, resolver)?;
        let trimmed_command = final_command_str.trim();

        if trimmed_command.is_empty() {
            continue;
        }

        if command_exec.run_in_parallel {
            parallel_batch.push(trimmed_command.to_string());
        } else {
            if !parallel_batch.is_empty() {
                execute_parallel_batch(&parallel_batch, config)?;
                parallel_batch.clear();
            }
            execute_single_command(trimmed_command, command_exec.ignore_errors, config)?;
        }
    }

    if !parallel_batch.is_empty() {
        execute_parallel_batch(&parallel_batch, config)?;
    }

    Ok(())
}

fn execute_single_command(
    command_str: &str,
    ignore_errors: bool,
    config: &ResolvedConfig,
) -> Result<()> {
    let command_to_run = if ignore_errors {
        format!("-{}", command_str)
    } else {
        command_str.to_string()
    };

    println!("\n> {}", command_str.green());
    Ok(executor::execute_command(
        &command_to_run,
        &config.project_root,
        &config.env,
    )?)
}

fn execute_parallel_batch(batch: &[String], config: &ResolvedConfig) -> Result<()> {
    println!("\n⚡ Running {} scripts in parallel...", batch.len());

    let results: Result<Vec<()>> = batch
        .par_iter()
        .map(|command_str| {
            println!("  > {}", command_str.cyan());
            executor::execute_command(command_str, &config.project_root, &config.env)
                .map_err(anyhow::Error::from)
        })
        .collect();

    results.with_context(|| "A command in the parallel batch failed.")?;
    println!("{}", "⚡ Parallel batch completed.".blue());
    Ok(())
}
