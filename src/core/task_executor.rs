use crate::{
    core::{color, commons::wrap_value, parameters::ArgResolver},
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
    execute_task_inner(task, config, resolver, index, 0)
}

// --- Internal Recursive Executor ---

fn execute_task_inner(
    task: &Arc<Task>,
    config: &ResolvedConfig,
    resolver: &ArgResolver,
    index: &mut GlobalIndex,
    depth: u32,
) -> Result<()> {
    let mut parallel_batch: Vec<(String, bool, bool)> = Vec::new();

    // The main loop now iterates over PlatformExecution blocks.
    for plat_exec in &task.commands {
        // Runtime platform selection
        let command_exec = match config.select_platform_exec(plat_exec) {
            Some(cmd) => cmd,
            None => {
                // If there's no command for the current platform (and no default),
                // we just skip it. This allows for platform-specific optional steps.
                log::debug!("Skipping command for current platform.");
                continue;
            }
        };

        if !command_exec.run_in_parallel && !parallel_batch.is_empty() {
            execute_parallel_batch(&parallel_batch, config)?;
            parallel_batch.clear();
        }

        match &command_exec.action {
            CommandAction::Execute(template) => {
                // NOTE: Pure composition is now handled by `flatten_task` before execution.
                // The executor only deals with commands that need to be rendered and run.
                let rendered_string =
                    assemble_final_command(template, config, resolver, index, depth)?;
                if !rendered_string.trim().is_empty() {
                    if command_exec.run_in_parallel {
                        parallel_batch.push((
                            rendered_string,
                            command_exec.ignore_errors,
                            command_exec.silent_mode,
                        ));
                    } else {
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
