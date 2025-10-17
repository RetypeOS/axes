use crate::{
    core::{color, commons::wrap_value, parameters::ArgResolver},
    models::{CommandAction, GlobalIndex, PlatformSpecializedTask, ResolvedConfig, RunSpec, TemplateComponent},
    system::executor,
};
use anyhow::{Context, Result, anyhow};
use colored::*;
use rayon::prelude::*;
use std::{fmt::Write};

// --- Main Public Function ---

/// Executes a platform-specialized task. This is the hot path for script execution.
pub fn execute_task(
    specialized_task: &PlatformSpecializedTask,
    config: &ResolvedConfig,
    resolver: &ArgResolver,
    index: &mut GlobalIndex,
) -> Result<()> {
    execute_task_inner(specialized_task, config, resolver, index, 0)
}

// --- Internal Recursive Executor ---

/// The internal, recursive executor for a platform-specialized task.
///
/// This function is on the "hot path" for script execution. It is highly optimized to simply
/// iterate over a flat list of `CommandExecution`s, render their templates, and dispatch them
/// sequentially or in parallel batches. All complex logic like platform selection and composition
/// has already been handled before this function is called.
///
/// # Arguments
/// * `specialized_task` - A task that has been pre-processed for the current platform.
/// * `config` - The fully resolved project configuration facade.
/// * `resolver` - The argument resolver containing values for parameter tokens.
/// * `index` - The global index, passed down for `<run(...)>` substitutions.
/// * `depth` - The current recursion depth, to prevent infinite loops from `<run(...)>` tokens.
fn execute_task_inner(
    specialized_task: &PlatformSpecializedTask,
    config: &ResolvedConfig,
    resolver: &ArgResolver,
    index: &mut GlobalIndex,
    depth: u32,
) -> Result<()> {
    // A batch of commands to be executed in parallel.
    // Storing `String` is necessary as `assemble_final_command` returns an owned string.
    let mut parallel_batch: Vec<(String, bool, bool)> = Vec::new();

    // --- OPTIMIZED HOT LOOP ---
    // This loop iterates over a simple, flat `Vec<CommandExecution>`.
    // There are no branches for platform selection, making it extremely fast.
    for command_exec in &specialized_task.commands {
        // If the current command is sequential, execute any pending parallel batch first.
        if !command_exec.run_in_parallel && !parallel_batch.is_empty() {
            execute_parallel_batch(&parallel_batch, config)?;
            parallel_batch.clear();
        }

        match &command_exec.action {
            CommandAction::Execute(template) => {
                // Render the final command string from the template components.
                let rendered_string =
                    assemble_final_command(template, config, resolver, index, depth)?;

                // Skip execution if the rendered command is empty after trimming whitespace.
                if !rendered_string.trim().is_empty() {
                    if command_exec.run_in_parallel {
                        // Add the owned string and its modifiers to the parallel batch.
                        parallel_batch.push((
                            rendered_string, // Move the owned string
                            command_exec.ignore_errors,
                            command_exec.silent_mode,
                        ));
                    } else {
                        // Execute sequentially. We can pass a `&str` slice to avoid allocations.
                        execute_single_command(
                            rendered_string.trim(),
                            command_exec.ignore_errors,
                            command_exec.silent_mode,
                            config,
                        )?;
                    }
                }
            }
            CommandAction::Print(template) => {
                let rendered_string =
                    assemble_final_command(template, config, resolver, index, depth)?;
                println!("{}", rendered_string);
            }
        }
    }

    // Execute any remaining commands in the final parallel batch.
    if !parallel_batch.is_empty() {
        execute_parallel_batch(&parallel_batch, config)?;
    }

    Ok(())
}

// --- Command Assembly (Recursive String Renderer) ---

///// "Renders" a template of components into a final, executable string.
pub fn assemble_final_command(
    template: &[TemplateComponent],
    config: &ResolvedConfig,
    resolver: &ArgResolver,
    _index: &mut GlobalIndex,
    _depth: u32,
) -> Result<String> {
    let mut final_command = String::with_capacity(template.len() * 50);
    for component in template {
        match component {
            TemplateComponent::Literal(s) => final_command.push_str(s),
            TemplateComponent::Parameter(def) => {
                let value = resolver
                    .get_specific_value(&def.original_token)
                    .unwrap_or_default();
                final_command.push_str(value);
            }
            TemplateComponent::GenericParams { literal } => {
                let values = resolver.get_generic_values();
                let joined = if *literal {
                    values
                        .iter()
                        .map(|arg| wrap_value(arg))
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    values.join(" ")
                };
                final_command.push_str(&joined);
            }
            TemplateComponent::Color(c) => final_command.push_str(color::style_to_ansi_code(*c)),
            TemplateComponent::Path => {
                final_command.push_str(&config.project_root.to_string_lossy())
            }
            TemplateComponent::Name => final_command.push_str(&config.qualified_name),
            TemplateComponent::Uuid => final_command.push_str(&config.uuid.to_string()),
            TemplateComponent::Version => {
                final_command.push_str(config.get_version()?.as_deref().unwrap_or(""))
            }
            TemplateComponent::Run(spec) => {
                let command_to_run = match spec {
                    RunSpec::Literal(cmd) => {
                        let temp_template = vec![TemplateComponent::Literal(cmd.clone())];
                        assemble_final_command(
                            &temp_template,
                            config,
                            resolver,
                            _index,
                            _depth + 1,
                        )?
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

            // --- LAZY RESOLUTION OF SYMBOLIC REFERENCES ---
            TemplateComponent::Script(name) | TemplateComponent::Var(name) => {
                // This logic is now handled by the flatten_template_recursive, which runs
                // before assemble_final_command. If we encounter a Script or Var here, it's a logic error.
                return Err(anyhow!(
                    "Internal Compiler Error: Unflattened symbolic reference '<{}::{}>' found during final command assembly.",
                    if matches!(component, TemplateComponent::Var(_)) {
                        "vars"
                    } else {
                        "scripts"
                    },
                    name
                ));
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
) -> Result<()> {
    if !silent {
        println!("{} {}", "→".blue(), command_str.green());
    }
    let env = config.get_env()?;
    executor::execute_command(command_str, ignore_errors, &config.project_root, &env)?;
    Ok(())
}

/// Prints and executes a batch of commands in parallel.
fn execute_parallel_batch(batch: &[(String, bool, bool)], config: &ResolvedConfig) -> Result<()> {
    let is_globally_silent = batch.iter().all(|(_, _, silent)| *silent);
    if !is_globally_silent {
        let mut header_block = String::with_capacity(batch.len() * 80);
        writeln!(
            header_block,
            "{}",
            format!("┌─ Running {} commands in parallel...", batch.len()).dimmed()
        )
        .unwrap();
        let inter_arrow = ("├─>").dimmed();
        for (command_str, _, silent) in batch.iter() {
            if !*silent {
                writeln!(header_block, "{} {}", inter_arrow, command_str.green()).unwrap();
            }
        }
        print!("{}", header_block);
    }

    if log::log_enabled!(log::Level::Trace) {
        log::trace!("Executing parallel batch of {} commands.", batch.len());
        for (i, (cmd, _, _)) in batch.iter().enumerate() {
            log::trace!("  - Batch[{}]: {}", i, cmd);
        }
    }

    let env = config.get_env()?;
    let results: Vec<Result<(), anyhow::Error>> = batch
        .par_iter()
        .map(|(command_str, ignore_errors, _)| {
            executor::execute_command(command_str, *ignore_errors, &config.project_root, &env)
                .map_err(anyhow::Error::from)
        })
        .collect();

    let mut errors = Vec::new();
    for (i, result) in results.into_iter().enumerate() {
        if let Err(e) = result {
            let failed_command = &batch[i].0;
            log::trace!(
                "Parallel command failed: '{}' with error: {}",
                failed_command,
                e
            );
            errors.push(anyhow!("Command '{}' failed: {}", failed_command.cyan(), e));
        }
    }

    if !errors.is_empty() {
        // Combine all errors into one final error.
        return Err(anyhow!(
            "{} command(s) in the parallel batch failed.",
            errors.len()
        ))
        .context(
            errors
                .into_iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    if !is_globally_silent {
        println!("{}", "└─ End batch.".dimmed());
    }
    Ok(())
}
