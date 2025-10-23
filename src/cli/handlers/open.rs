//! # Handler for the `open` command
//!
//! This module provides the logic for the `axes open` command, which allows users to
//! launch a project in a configured application, such as a code editor, file explorer,
//! or a new terminal session.
//!
//! ## Core Logic
//!
//! 1.  **Argument Parsing**: It parses an optional `app_key` to specify which application to use,
//!     a `--list` flag to show available options, and trailing arguments to be passed to the
//!     underlying script.
//! 2.  **Configuration Resolution**: It resolves the project's configuration to access the
//!     `[options.open_with]` table, which contains the scripts for each `app_key`.
//! 3.  **Command Dispatching**:
//!     - If `--list` is used, it prints a formatted list of all available `open` commands.
//!     - Otherwise, it determines which `app_key` to use (the one provided, or the configured
//!       default) and proceeds to execution.
//! 4.  **Task Execution**: It retrieves the corresponding `Task` (AST) for the chosen `app_key`,
//!     flattens it to resolve any compositions, specializes it for the current platform, builds
//!     an argument resolver, and finally passes it to the `task_executor` to be run.

use crate::{
    cli::handlers::commons,
    core::task_executor,
    models::{ResolvedConfig, ResolvedOpenWithConfig},
    state::AppStateGuard,
};
use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::console::measure_text_width;

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Opens the project using a configured application or script."
)]
struct OpenArgs {
    /// The application key to use (e.g., 'editor', 'shell'). If omitted, uses the configured default.
    app_key: Option<String>,
    /// List all available 'open with' commands for this project.
    #[arg(long, short)]
    list: bool,
    /// Parameters to pass to the open command.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

// --- Main Handler ---

/// The main handler for the `open` command.
///
/// It determines whether to list available `open` commands or to execute one based
/// on the parsed arguments.
///
/// # Arguments
/// * `context` - The project context in which to operate, provided by the dispatcher.
/// * `args` - Command-specific arguments (e.g., `editor -- -p 1234`).
/// * `state_guard` - A mutable guard to the application state, needed for config resolution.
pub fn handle(
    context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard<'_>,
) -> Result<()> {
    let open_args = OpenArgs::try_parse_from(&args)?;
    let config = commons::resolve_config_for_context(context, state_guard)?;
    let options = config.get_options()?;

    if open_args.list {
        list_open_commands(&config.qualified_name, &options.open_with);
        return Ok(());
    }

    execute_open_command(open_args, config, options.open_with)
}

// --- Subcommand Logic ---

/// Prints a formatted list of all available `open_with` commands for a project.
///
/// # Arguments
/// * `project_name` - The qualified name of the project.
/// * `open_with` - The resolved `[options.open_with]` configuration.
fn list_open_commands(project_name: &str, open_with: &ResolvedOpenWithConfig) {
    println!("\nAvailable `open` commands for '{}':", project_name.cyan());
    if open_with.commands.is_empty() {
        println!("  {}", "No commands defined.".dimmed());
        return;
    }

    let mut sorted_keys: Vec<_> = open_with.commands.keys().collect();
    sorted_keys.sort();
    let max_len = sorted_keys
        .iter()
        .map(|k| measure_text_width(k))
        .max()
        .unwrap_or(0);

    for key in sorted_keys {
        let task = open_with
            .commands
            .get(key)
            .expect("Key should exist as we are iterating over map keys");
        let padding = " ".repeat(max_len - measure_text_width(key));
        print!("  - {}{} ", key.green(), padding);
        if Some(key.as_str()) == open_with.default.as_deref() {
            print!("{} ", t!("common.label.default").yellow());
        }
        if let Some(desc) = &task.desc
            && !desc.trim().is_empty()
        {
            print!("{}", desc.dimmed());
        }

        println!();
    }
}

/// Handles the logic for executing a specific `open_with` command.
///
/// This function performs the multi-step process of preparing and running a task:
/// it finds the correct task, flattens it, builds an argument resolver, specializes it
/// for the current OS, and finally executes it.
///
/// # Arguments
/// * `open_args` - The parsed arguments for the `open` command.
/// * `config` - The fully resolved configuration for the project.
/// * `open_with` - The resolved `[options.open_with]` section of the configuration.
fn execute_open_command(
    open_args: OpenArgs,
    config: ResolvedConfig,
    open_with: ResolvedOpenWithConfig,
) -> Result<()> {
    // 1. Determine which application key to use.
    let app_key_to_use = open_args
        .app_key
        .as_deref()
        .or(open_with.default.as_deref())
        .ok_or_else(|| anyhow!(t!("open.error.no_default")))?;

    // 2. Get the universal Task AST.
    let task_universal = open_with.commands.get(app_key_to_use).ok_or_else(|| {
        if Some(app_key_to_use) == open_with.default.as_deref() {
            anyhow!(
                t!("open.error.default_not_found"),
                key = app_key_to_use.cyan()
            )
        } else {
            anyhow!(t!("open.error.key_not_found"), key = app_key_to_use.cyan())
        }
    })?;

    // 3. Flatten the task to resolve compositions.
    let task_flattened = config.flatten_task(task_universal)?;

    // 4. Build the argument resolver from the *universal* flattened task.
    let resolver = commons::build_resolver_for_task(&task_flattened, &open_args.params)?;

    // 5. [OPTIMIZATION] Specialize the task for the current platform.
    let task_specialized = config.specialize_task_for_platform(&task_flattened);

    // 6. Execute the final, specialized task.
    println!(
        "\n🚀 Opening '{}' with '{}'...",
        config.qualified_name.cyan(),
        app_key_to_use.cyan()
    );
    task_executor::execute_task(&task_specialized, &config, &resolver)?;

    Ok(())
}
