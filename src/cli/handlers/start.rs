//! # Handler for the `start` command
//!
//! This module provides the logic for the `axes start` command, which initiates an
//! interactive shell session within a project's context. It is responsible for setting up
//! the session, running lifecycle hooks (`at_start`, `at_exit`), and managing parameters.
//!
//! ## Core Logic
//!
//! 1.  **Argument Parsing & Pre-flight Checks**: It parses arguments like the target `context`,
//!     the `--dry-run` flag, and any trailing parameters for the hooks. It also prevents
//!     the creation of nested sessions.
//! 2.  **Configuration & Hook Resolution**: It resolves the project's full configuration to
//!     obtain the `at_start` and `at_exit` tasks defined in the `[options]` table.
//! 3.  **Task Flattening**: Crucially, it flattens both the `at_start` and `at_exit` tasks.
//!     This resolves any script compositions (`<scripts::...>`) within the hooks before
//!     parameters are processed.
//! 4.  **Unified Parameter Resolution**: It collects all parameter definitions (`<params::...>`)
//!     from *both* the start and exit hooks. This allows a single set of parameters passed to
//!     `axes start` to be used in both hooks, creating a consistent session context. An
//!     `ArgResolver` is then built from this unified contract.
//! 5.  **Dispatch**:
//!     - If `--dry-run` is used, it calls `dry_run_session` to print the resolved execution
//!       plan for both hooks without running them.
//!     - Otherwise, it delegates the entire session lifecycle management (init script
//!       generation, shell spawning, cleanup) to the `system::shell::launch_session` function.

use crate::{
    cli::handlers::commons,
    core::{parameters::ArgResolver, task_executor},
    models::{CommandAction, ResolvedConfig, Task, TemplateComponent},
    state::AppStateGuard,
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

/// The main handler for the `start` command.
///
/// It orchestrates the entire process of setting up a project session, from resolving
/// configuration and hooks to building a unified argument resolver and dispatching to
//  either a dry run or a live session.
///
/// # Arguments
/// * `context` - The project context for the session, provided by the dispatcher.
/// * `args` - Command-specific arguments (e.g., `--dry-run`, and parameters for the hooks).
/// * `index` - A mutable guard to the application state. **Note**: `index` is the parameter name
///   due to a macro limitation and should be treated as `state_guard`.
pub fn handle(
    context: Option<String>,
    args: Vec<String>,
    index: &mut AppStateGuard<'_>,
) -> Result<()> {
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
        dry_run_session(&config, flattened_start, flattened_exit, &resolver)
    } else {
        shell::launch_session(&config, flattened_start, flattened_exit, &resolver)
            .map_err(Into::into)
    }
}

// --- Subcommand Logic ---

/// Displays the fully-resolved execution plan for the `at_start` and `at_exit` hooks
/// without actually executing them.
///
/// # Arguments
/// * `config` - The resolved configuration of the project.
/// * `task_start` - The flattened `at_start` task.
/// * `task_exit` - The flattened `at_exit` task.
/// * `resolver` - The unified argument resolver for the session.
fn dry_run_session(
    config: &ResolvedConfig,
    task_start: Option<Arc<Task>>,
    task_exit: Option<Arc<Task>>,
    resolver: &ArgResolver<'_>,
) -> Result<()> {
    println!(
        "\n📋 Dry-run for session in '{}'",
        config.qualified_name.cyan()
    );

    display_hook_plan("at_start", task_start, config, resolver)?;
    display_hook_plan("at_exit", task_exit, config, resolver)?;

    Ok(())
}

/// A helper for `dry_run_session` that displays the execution plan for a single hook.
///
/// # Arguments
/// * `hook_name` - The name of the hook being displayed (e.g., "`at_start`").
/// * `task` - The flattened `Task` for the hook.
/// * `config` - The resolved configuration of the project.
/// * `resolver` - The unified argument resolver.
fn display_hook_plan(
    hook_name: &str,
    task: Option<Arc<Task>>,
    config: &ResolvedConfig,
    resolver: &ArgResolver<'_>,
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
                task_executor::assemble_final_command(template, config, resolver, 0)?;
            println!("{}{}", action_prefix, rendered_string.green());
        }
    }
    Ok(())
}
