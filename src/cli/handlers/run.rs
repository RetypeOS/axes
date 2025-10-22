use crate::{
    cli::handlers::commons,
    core::{parameters::ArgResolver, task_executor},
    models::{CommandAction, GlobalIndex, PlatformSpecializedTask, ResolvedConfig},
    state::AppStateGuard,
};
use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;

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
    /// The name of the script to run.
    script_name: Option<String>,

    /// Display the execution plan without running any commands.
    #[arg(long, name = "dry-run")]
    dry_run: bool,

    /// Parameters and flags to pass to the script.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

// --- Main Handler ---

/// Main entry point for the 'run' command.
/// Dispatches to list, dry-run, or execute a script based on arguments.
pub fn handle(
    context: Option<String>,
    mut args: Vec<String>,
    state_guard: &mut AppStateGuard,
) -> Result<()> {
    let script_name_opt = if args.is_empty() {
        None
    } else {
        Some(args.remove(0))
    };
    let script_params = args;
    let config = commons::resolve_config_for_context(context, state_guard)?;

    match script_name_opt {
        Some(script_name) => {
            let (is_dry_run, final_params) = parse_dry_run_flag(&script_params);

            let task_universal = config.get_script(&script_name)?.ok_or_else(|| {
                anyhow!(
                    t!("run.error.not_found"),
                    script = script_name.cyan(),
                    project = config.qualified_name.yellow()
                )
            })?;

            let task_flattened = config.flatten_task(&task_universal)?;

            let resolver = commons::build_resolver_for_task(&task_flattened, &final_params)?;

            let task_specialized = config.specialize_task_for_platform(&task_flattened);

            if is_dry_run {
                dry_run_script(&script_name, &task_specialized, &config, &resolver)
            } else {
                execute_script(&script_name, &task_specialized, &config, &resolver)
            }
        }
        None => {
            // --- No script name provided: list available scripts ---
            // Any arguments passed (like `--dry-run`) are ignored in this mode.
            if !script_params.is_empty() {
                log::warn!("Arguments provided to 'axes run' without a script name are ignored.");
            }
            list_available_scripts(&config, state_guard.index())
        }
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

    let mut sorted_keys: Vec<_> = scripts.keys().collect();
    sorted_keys.sort();

    for script_name in sorted_keys {
        let task = scripts.get(script_name).unwrap();
        print!("  - {}", script_name.green());

        let source_project_name =
            crate::cli::handlers::info::find_task_source("scripts", script_name, config, index)?;
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

        if let Some(d) = &task.desc
            && !d.trim().is_empty()
        {
            print!(": {}", d.dimmed());
        }
        println!();
    }

    println!("\n{}", t!("run.info.how_to_run").dimmed());
    Ok(())
}

/// Prepares and executes a script, conditionally printing the context header.
fn execute_script(
    script_name: &str,
    task: &PlatformSpecializedTask,
    config: &ResolvedConfig,
    resolver: &ArgResolver,
) -> Result<()> {
    if task.commands.is_empty() {
        println!("{}", t!("run.info.empty_script").yellow());
        return Ok(());
    }

    // Resolver is now passed in.

    let is_globally_silent = task.commands.iter().all(|cmd| cmd.silent_mode);
    log::debug!(
        "Script '{}' is_globally_silent = {}",
        script_name,
        is_globally_silent
    );

    if !is_globally_silent {
        let prefix_path = format_prefix_path(&config.qualified_name);
        println!("\n[{}:{}]", prefix_path.dimmed(), script_name.cyan());
    }

    task_executor::execute_task(task, config, resolver)?;
    Ok(())
}

/// Manually parses and removes the `--dry-run` flag from a list of parameters.
/// Clap is not suitable here as we need to separate the handler's flags from the script's parameters.
fn parse_dry_run_flag(params: &[String]) -> (bool, Vec<String>) {
    let mut is_dry_run = false;
    let mut final_params = Vec::with_capacity(params.len());

    for param in params {
        if param == "--dry-run" {
            is_dry_run = true;
        } else {
            final_params.push(param.clone());
        }
    }
    (is_dry_run, final_params)
}

/// Displays the fully-resolved execution plan for a script.
fn dry_run_script(
    script_name: &str,
    task: &PlatformSpecializedTask,
    config: &ResolvedConfig,
    resolver: &ArgResolver,
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

    println!("---");
    // Iterate over the universal AST, but only render the command for the current platform
    for command_exec in &task.commands {
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

        // assemble_final_command now uses the pre-built resolver
        let rendered_string = task_executor::assemble_final_command(template, config, resolver, 0)?;

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
    println!("---");
    Ok(())
}

fn format_prefix_path(qualified_name: &str) -> String {
    let mut parts = qualified_name.split('/');
    // Use `nth` to efficiently get the part at a specific index from the end.
    // `rev()` reverses the iterator, `nth(1)` gets the second-to-last element.
    if let Some(second_to_last) = parts.clone().rev().nth(1) {
        // More than 2 parts exist.
        // `parts.last()` is now efficient as `split` is a DoubleEndedIterator.
        let last = parts.next_back().unwrap_or(""); // Safe to unwrap
        format!(".../{}/{}", second_to_last, last)
    } else {
        // 1 or 2 parts, return the whole name.
        qualified_name.to_string()
    }
}
