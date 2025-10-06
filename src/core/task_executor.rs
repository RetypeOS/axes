// EN: src/core/task_executor.rs (REBUILT FOR LAZY EXECUTION)

use crate::{
    core::parameters::ArgResolver,
    models::{CommandAction, GlobalIndex, ResolvedConfig, RunSpec, Task, TemplateComponent},
    system::executor,
};
use anyhow::{Context, Result, anyhow};
use colored::*;
use rayon::prelude::*;
use std::{fmt::Write, sync::Arc};

// --- Main Public Function ---

/// Executes a complete `Task` object, handling sequential, parallel, and composed commands.
/// This is the entry point for running a script.
pub fn execute_task(
    task: &Arc<Task>,
    config: &ResolvedConfig,
    resolver: &ArgResolver,
    index: &mut GlobalIndex,
) -> Result<()> {
    // Start the execution with an initial depth of 0.
    execute_task_inner(task, config, resolver, index, 0)
}

// --- Internal Recursive Executor ---

/// Inner recursive function for task execution that carries the call stack.
fn execute_task_inner(
    task: &Arc<Task>,
    config: &ResolvedConfig,
    resolver: &ArgResolver,
    index: &mut GlobalIndex,
    depth: u32, // Add depth parameter
) -> Result<()> {
    let mut parallel_batch: Vec<(String, bool, bool)> = Vec::new();

    for command_exec in &task.commands {
        // If the current command is sequential, execute any pending parallel batch first.
        if !command_exec.run_in_parallel && !parallel_batch.is_empty() {
            execute_parallel_batch(&parallel_batch, config)?;
            parallel_batch.clear();
        }

        match &command_exec.action {
            CommandAction::Execute(template) => {
                if template.len() == 1
                    && let TemplateComponent::Script(script_name) = &template[0]
                {
                    // FIX: Pass incremented depth to the recursive call.
                    let sub_task = config.get_script(script_name, depth + 1)?.ok_or_else(|| {
                        anyhow!("Script '{}' not found for composition.", script_name)
                    })?;

                    // Recursive call to execute the sub-task.
                    execute_task_inner(&sub_task, config, resolver, index, depth + 1)?;
                    continue; // Skip to the next command in the outer task.
                }

                // Not a pure composition, so assemble the command string.
                let rendered_string =
                    assemble_final_command(template, config, resolver, index, depth)?;
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
                // FIX: Pass current depth to `assemble_final_command`.
                let rendered_string =
                    assemble_final_command(template, config, resolver, index, depth)?;
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

// --- Command Assembly (Recursive String Renderer) ---

/// "Renders" a template of components into a final, executable string.
/// It recursively resolves all dynamic and static tokens, including symbolic references.
#[allow(clippy::only_used_in_recursion)]
pub fn assemble_final_command(
    template: &[TemplateComponent],
    config: &ResolvedConfig,
    resolver: &ArgResolver,
    index: &mut GlobalIndex,
    depth: u32,
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
                            "Internal logic error: resolved value not found for token '{}'",
                            def.original_token
                        )
                    })?;
                final_command.push_str(value);
            }
            TemplateComponent::GenericParams => {
                final_command.push_str(resolver.get_generic_value())
            }
            TemplateComponent::Run(spec) => {
                let command_to_run = match spec {
                    RunSpec::Literal(cmd) => {
                        let temp_template = vec![TemplateComponent::Literal(cmd.clone())];
                        // Recursively assemble to expand tokens inside the `run` literal.
                        assemble_final_command(&temp_template, config, resolver, index, depth + 1)?
                    }
                };
                let env = config.get_env()?;
                let output = executor::execute_and_capture_output(
                    &command_to_run,
                    &config.project_root,
                    &env,
                )?;
                final_command.push_str(output.trim());
            }
            TemplateComponent::Path => {
                final_command.push_str(&config.project_root.to_string_lossy())
            }
            TemplateComponent::Name => final_command.push_str(&config.qualified_name),
            TemplateComponent::Uuid => final_command.push_str(&config.uuid.to_string()),
            TemplateComponent::Version => {
                // Lazily get the version by searching up the hierarchy.
                final_command.push_str(config.get_version()?.as_deref().unwrap_or(""));
            }

            // --- LAZY RESOLUTION OF SYMBOLIC REFERENCES ---
            TemplateComponent::Script(script_name) => {
                log::debug!("Resolving inline script reference: '{}'", script_name);
                let script_task = config
                    .get_script(script_name, depth + 1)?
                    .ok_or_else(|| anyhow!("Referenced script '{}' not found.", script_name))?;
                if script_task.commands.len() > 1 {
                    return Err(anyhow!(
                        "Inline script composition for '<axes::scripts::{}>' is not supported because it is a multi-line script.",
                        script_name
                    ));
                }
                if let Some(command) = script_task.commands.first() {
                    let sub_template = match &command.action {
                        CommandAction::Execute(t) | CommandAction::Print(t) => t,
                    };
                    final_command.push_str(&assemble_final_command(
                        sub_template,
                        config,
                        resolver,
                        index,
                        depth + 1,
                    )?);
                }
            }
            TemplateComponent::Var(var_name) => {
                log::debug!("Resolving var reference: '{}'", var_name);
                let var_task = config
                    .get_var(var_name, depth + 1)?
                    .ok_or_else(|| anyhow!("Referenced variable '{}' not found.", var_name))?;
                if var_task.commands.len() > 1 {
                    return Err(anyhow!(
                        "Variable '{}' must expand to a single-line value.",
                        var_name
                    ));
                }
                if let Some(command) = var_task.commands.first() {
                    let sub_template = match &command.action {
                        CommandAction::Execute(t) | CommandAction::Print(t) => t,
                    };
                    final_command.push_str(&assemble_final_command(
                        sub_template,
                        config,
                        resolver,
                        index,
                        depth + 1,
                    )?);
                }
            }
        }
    }
    Ok(final_command)
}

// --- Execution Helpers ---

/// Executes a single sequential command.
fn execute_single_command(
    command_str: &str,
    ignore_errors: bool,
    silent: bool,
    config: &ResolvedConfig,
    //index: &mut GlobalIndex,
) -> Result<()> {
    if !silent {
        println!("\n→ {}", command_str.green());
    }
    let env = config.get_env()?;
    executor::execute_command(command_str, ignore_errors, &config.project_root, &env)?;
    Ok(())
}

/// Prints and executes a batch of commands in parallel.
fn execute_parallel_batch(
    batch: &[(String, bool, bool)],
    config: &ResolvedConfig,
    //index: &mut GlobalIndex,
) -> Result<()> {
    let is_globally_silent = batch.iter().all(|(_, _, silent)| *silent);
    if !is_globally_silent {
        let mut header_block = String::new();
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

    let env = config.get_env()?;
    let results: Result<Vec<()>> = batch
        .par_iter()
        .map(|(command_str, ignore_errors, _)| {
            executor::execute_command(command_str, *ignore_errors, &config.project_root, &env)
                .map_err(anyhow::Error::from)
        })
        .collect();

    results.with_context(|| "A command in the parallel batch failed.")?;

    if !is_globally_silent {
        println!("{}{}", "└─".dimmed(), " Parallel batch completed.".blue());
    }
    Ok(())
}
