use crate::{
    cli::handlers::commons,
    core::{parameters::ArgResolver, task_executor},
    models::{CommandAction, GlobalIndex, ResolvedConfig, Task, TemplateComponent},
    system::shell,
};
use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use std::sync::Arc;

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Starts an interactive project session, running `at_start` and `at_exit` hooks."
)]
struct StartArgs {
    /// The context of the project to start a session in. Defaults to the current project.
    context: Option<String>,
    /// Display the `at_start` and `at_exit` hooks without executing them.
    #[arg(long, name = "dry-run")]
    dry_run: bool,
    /// Parameters to pass to the `at_start` and `at_exit` hooks.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

// --- Main Handler ---

pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. Parse args and perform pre-flight checks.
    let start_args = StartArgs::try_parse_from(&args)?;

    let final_context = start_args
        .context
        .or(context)
        .unwrap_or_else(|| ".".to_string());

    if std::env::var("AXES_PROJECT_UUID").is_ok() {
        return Err(anyhow!(t!("start.error.nested_session")));
    }

    // 2. Lazily resolve project configuration and get the merged options.
    let config = commons::resolve_config_for_context(Some(final_context), index)?;
    let options = config.get_options()?;
    let task_start = options.at_start;
    let task_exit = options.at_exit;

    // 3. Collect parameter definitions from BOTH hooks for a unified resolver.
    let mut all_definitions = Vec::new();

    // The tasks must be flattened *before* collecting parameters to resolve compositions.
    let flattened_start = if let Some(task) = &task_start {
        Some(config.flatten_task(task)?)
    } else {
        None
    };
    let flattened_exit = if let Some(task) = &task_exit {
        Some(config.flatten_task(task)?)
    } else {
        None
    };

    if let Some(task) = &flattened_start {
        all_definitions.extend(commons::collect_parameter_defs_from_task(task));
    }
    if let Some(task) = &flattened_exit {
        all_definitions.extend(commons::collect_parameter_defs_from_task(task));
    }

    let has_generic_params = flattened_start
        .iter()
        .chain(flattened_exit.iter())
        .flat_map(|task| &task.commands)
        .any(|plat_exec| {
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

    let resolver = ArgResolver::new(&all_definitions, &start_args.params, has_generic_params)?;

    // 4. Dispatch to either dry-run or actual session launch.
    if start_args.dry_run {
        dry_run_session(&config, flattened_start, flattened_exit, &resolver, index)
    } else {
        shell::launch_session(&config, flattened_start, flattened_exit, &resolver, index)
            .map_err(Into::into)
    }
}

// --- Subcommand Logic ---

/// Displays the execution plan for session hooks without running them.
fn dry_run_session(
    config: &ResolvedConfig,
    task_start: Option<Arc<Task>>,
    task_exit: Option<Arc<Task>>,
    resolver: &ArgResolver,
    index: &mut GlobalIndex,
) -> Result<()> {
    println!(
        "\n📋 Dry-run for session in '{}'",
        config.qualified_name.cyan()
    );

    display_hook_plan("at_start", task_start, config, resolver, index)?;
    display_hook_plan("at_exit", task_exit, config, resolver, index)?;

    Ok(())
}

/// Helper for `dry_run_session` to display the plan for a single hook.
fn display_hook_plan(
    hook_name: &str,
    task: Option<Arc<Task>>,
    config: &ResolvedConfig,
    resolver: &ArgResolver,
    index: &mut GlobalIndex,
) -> Result<()> {
    println!("\n--- Hook: [{}] ---", hook_name.yellow());

    let task = match task {
        Some(t) => t,
        None => {
            println!("{}", "Not defined.".dimmed());
            return Ok(());
        }
    };

    if task.commands.is_empty() {
        println!("{}", "Defined but empty.".dimmed());
        return Ok(());
    }

    // Iterate over the universal AST and render the command for the current platform.
    for plat_exec in &task.commands {
        if let Some(command_exec) = config.select_platform_exec(plat_exec) {
            let (action_prefix, template) = match &command_exec.action {
                CommandAction::Print(t) => ("# ".dimmed(), t),
                CommandAction::Execute(t) => ("".normal(), t),
            };
            let rendered_string =
                task_executor::assemble_final_command(template, config, resolver, index, 0)?;
            println!("{}{}", action_prefix, rendered_string.green());
        }
    }
    Ok(())
}
