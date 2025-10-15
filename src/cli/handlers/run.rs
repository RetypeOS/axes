// EN: src/cli/handlers/run.rs (FINALIZED FOR CLEAN UX)

use crate::{
    cli::handlers::commons,
    core::{parameters::ArgResolver, task_executor},
    models::{CommandAction, GlobalIndex, ResolvedConfig, Task, TemplateComponent},
};
use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use std::sync::Arc;

// --- Helper for the main dispatcher ---

/// Parses a script path string like "my-app/api/build" into its context and script name.
/// This is a helper function used internally by the main dispatcher (`bin/axes.rs`).
pub fn parse_script_path(full_path: &str) -> (Option<&str>, &str) {
    if let Some((context, script_name)) = full_path.rsplit_once('/') {
        (Some(context), script_name)
    } else {
        (None, full_path)
    }
}

// --- Command Argument Parsing ---

#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    about = "Runs a script in the project's context. If no script is provided, lists available scripts."
)]
struct RunArgs {
    script_name: Option<String>,
    #[arg(long, name = "dry-run")]
    dry_run: bool,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

// --- Main Handler ---

/// Main entry point for the 'run' command.
/// Dispatches to list, dry-run, or execute a script based on arguments.
pub fn handle(
    context: Option<String>,
    mut args: Vec<String>,
    index: &mut GlobalIndex,
) -> Result<()> {
    let full_args = if args.is_empty() {
        vec![]
    } else {
        let script_name = args.remove(0);
        let mut temp_args = vec![script_name];
        temp_args.extend(args);
        temp_args
    };

    let run_args = RunArgs::try_parse_from(&full_args)?;
    let config = commons::resolve_config_for_context(context, index)?;

    match run_args.script_name {
        Some(script_name) => {
            let task = config.get_script(&script_name)?.ok_or_else(|| {
                anyhow!(
                    t!("run.error.not_found"),
                    script = script_name.cyan(),
                    project = config.qualified_name.yellow()
                )
            })?;

            // The crucial new step: flatten the task before any further processing.
            let flattened_task = config.flatten_task(&task)?;

            if run_args.dry_run {
                dry_run_script(
                    &script_name,
                    &flattened_task,
                    &config,
                    &run_args.params,
                    index,
                )
            } else {
                execute_script(
                    &script_name,
                    &flattened_task,
                    &config,
                    &run_args.params,
                    index,
                )
            }
        }
        None => list_available_scripts(&config, index),
    }
}

// --- Subcommand Logic ---

/// Lists all available scripts for the current project context.
fn list_available_scripts(config: &ResolvedConfig, index: &GlobalIndex) -> Result<()> {
    let scripts = config.get_all_scripts()?;
    println!(
        "\nAvailable scripts for '{}':",
        config.qualified_name.cyan()
    );

    if scripts.is_empty() {
        println!("  {}", t!("info.label.no_scripts").dimmed());
        return Ok(());
    }

    let mut sorted_keys: Vec<_> = scripts.keys().cloned().collect();
    sorted_keys.sort();

    for script_name in sorted_keys {
        let task = scripts.get(&script_name).unwrap();
        print!("  - {}", script_name.green());

        let source_project_name =
            crate::cli::handlers::info::find_task_source("scripts", &script_name, config, index)?;
        if source_project_name != config.qualified_name {
            print!(
                " {}",
                format!(
                    "[{}]",
                    format_args!(t!("common.label.inherited"), from = source_project_name)
                )
                .dimmed()
            );
        }

        if let Some(d) = &task.desc {
            if !d.trim().is_empty() {
                print!(": {}", d.dimmed());
            }
        }
        println!();
    }

    println!("\n{}", t!("run.info.how_to_run").dimmed());
    Ok(())
}

/// Prepares and executes a script, conditionally printing the context header.
fn execute_script(
    script_name: &str,
    task: &Arc<Task>,
    config: &ResolvedConfig,
    params: &[String],
    index: &mut GlobalIndex,
) -> Result<()> {
    if task.commands.is_empty() {
        println!("{}", t!("run.info.empty_script").yellow());
        return Ok(());
    }

    let all_definitions = commons::collect_parameter_defs_from_task(task, config);

    // FIX: Apply the same fix here
    let has_generic_params = task.commands.iter().any(|plat_exec| {
        [
            plat_exec.default.as_ref(),
            plat_exec.windows.as_ref(),
            plat_exec.linux.as_ref(),
            plat_exec.macos.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|cmd_exec| {
            let template = match &cmd_exec.action {
                CommandAction::Execute(t) | CommandAction::Print(t) => t,
            };
            template
                .iter()
                .any(|c| matches!(c, TemplateComponent::GenericParams { .. }))
        })
    });

    let resolver = ArgResolver::new(&all_definitions, params, has_generic_params)?;

    // Check if the script will be silent on the current platform
    let is_globally_silent = task.commands.iter().all(|plat_exec| {
        config
            .select_platform_exec(plat_exec)
            .map_or(true, |cmd| cmd.silent_mode)
    });

    if !is_globally_silent {
        let prefix_path = format_prefix_path(&config.qualified_name);
        println!("\n[{}:{}]", prefix_path.dimmed(), script_name.cyan());
    }

    task_executor::execute_task(task, config, &resolver, index)?;
    Ok(())
}

/// Displays the fully-resolved execution plan for a script.
fn dry_run_script(
    script_name: &str,
    task: &Arc<Task>, // Receives the FLATTENED task
    config: &ResolvedConfig,
    params: &[String],
    index: &mut GlobalIndex,
) -> Result<()> {
    let prefix_path = format_prefix_path(&config.qualified_name);
    println!(
        "\n📋 Dry-run for [{}:{}]",
        prefix_path.dimmed(),
        script_name.cyan()
    );

    if task.commands.is_empty() {
        println!("\n{}", t!("run.info.empty_script").yellow());
        return Ok(());
    }

    let all_definitions = commons::collect_parameter_defs_from_task(task, config);
    let has_generic_params = task.commands.iter().any(|plat_exec| {
        [
            plat_exec.default.as_ref(),
            plat_exec.windows.as_ref(),
            plat_exec.linux.as_ref(),
            plat_exec.macos.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|cmd_exec| {
            let template = match &cmd_exec.action {
                CommandAction::Execute(t) | CommandAction::Print(t) => t,
            };
            template
                .iter()
                .any(|c| matches!(c, TemplateComponent::GenericParams { .. }))
        })
    });
    let resolver = ArgResolver::new(&all_definitions, params, has_generic_params)?;

    println!("---");
    // Iterate over the universal AST, but only render the command for the current platform
    for plat_exec in &task.commands {
        if let Some(command_exec) = config.select_platform_exec(plat_exec) {
            let mut prefixes = String::new();
            if command_exec.silent_mode {
                prefixes.push('@');
            }
            if command_exec.ignore_errors {
                prefixes.push('-');
            }
            if command_exec.run_in_parallel {
                prefixes.push('>');
            }

            let (action_prefix, template) = match &command_exec.action {
                CommandAction::Print(t) => ("# ".dimmed(), t),
                CommandAction::Execute(t) => ("".normal(), t),
            };

            let rendered_string =
                task_executor::assemble_final_command(template, config, &resolver, index, 0)?;

            if prefixes.is_empty() {
                println!("{}{}", action_prefix, rendered_string.green());
            } else {
                println!(
                    "{} {}{}",
                    prefixes.dimmed(),
                    action_prefix,
                    rendered_string.green()
                );
            }
        }
    }
    println!("---");
    Ok(())
}

fn format_prefix_path(qualified_name: &str) -> String {
    let parts: Vec<&str> = qualified_name.split('/').collect();
    match parts.len() {
        1 => parts[0].to_string(),
        2 => parts.join("/"),
        _ => format!(".../{}", &parts[parts.len() - 2..].join("/")),
    }
}
