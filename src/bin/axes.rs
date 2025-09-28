// EN: src/bin/axes.rs

use anyhow::Result;
use axes::{
    CancellationToken,
    cli::{Cli, handlers},
    system::executor,
};
use clap::Parser;
use colored::*;
use std::env;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

// --- Command Definition and Registry ---

/// Defines a system command, its aliases, and its synchronous handler function.
/// The handler signature is kept consistent across all commands for simplicity in the registry.
struct CommandDefinition {
    name: &'static str,
    aliases: &'static [&'static str],
    handler: fn(Vec<String>, &CancellationToken) -> Result<()>,
}

/// The single source of truth for all system commands.
/// This declarative approach makes adding, removing, or modifying commands trivial.
/// To add a new command, simply add a new entry to this static array.
static COMMAND_REGISTRY: &[CommandDefinition] = &[
    CommandDefinition {
        name: "alias",
        aliases: &[],
        handler: handlers::alias::handle,
    },
    CommandDefinition {
        name: "_cache",
        aliases: &[],
        handler: handlers::debug_cache::handle,
    },
    CommandDefinition {
        name: "delete",
        aliases: &["del"],
        handler: handlers::delete::handle,
    },
    CommandDefinition {
        name: "info",
        aliases: &[],
        handler: handlers::info::handle,
    },
    CommandDefinition {
        name: "init",
        aliases: &["new"],
        handler: handlers::init::handle,
    },
    CommandDefinition {
        name: "link",
        aliases: &[],
        handler: handlers::link::handle,
    },
    CommandDefinition {
        name: "open",
        aliases: &[],
        handler: handlers::open::handle,
    },
    CommandDefinition {
        name: "register",
        aliases: &["reg"],
        handler: handlers::register::handle,
    },
    CommandDefinition {
        name: "rename",
        aliases: &[],
        handler: handlers::rename::handle,
    },
    CommandDefinition {
        name: "run",
        aliases: &[],
        handler: handlers::run::handle,
    },
    CommandDefinition {
        name: "start",
        aliases: &[],
        handler: handlers::start::handle,
    },
    CommandDefinition {
        name: "tree",
        aliases: &["ls"],
        handler: handlers::tree::handle,
    },
    CommandDefinition {
        name: "unregister",
        aliases: &["unreg"],
        handler: handlers::unregister::handle,
    },
];

/// Finds a command definition in the registry by its name or alias.
fn find_command(name: &str) -> Option<&'static CommandDefinition> {
    COMMAND_REGISTRY
        .iter()
        .find(|cmd| cmd.name == name || cmd.aliases.contains(&name))
}

/// The main entry point of the `axes` application.
/// It sets up logging, parses arguments, dispatches to the correct handler,
/// and performs centralized error handling.
fn main() {
    // The CancellationToken is now a simple flag, primarily for future use in long-running,
    // non-process tasks. The main Ctrl+C handling is managed by the executor.
    let cancellation_token = Arc::new(AtomicBool::new(false));
    env_logger::init();

    // The entire application logic is wrapped in a Result to enable centralized error handling.
    if let Err(e) = run_cli(Cli::parse(), cancellation_token) {
        // --- Centralized Error Handling ---
        // Check if the error is a command interruption (e.g., from Ctrl+C).
        if let Some(exec_err) = e.downcast_ref::<executor::ExecutionError>()
            && matches!(exec_err, executor::ExecutionError::Interrupted { .. }) {
                // If so, exit silently with the standard exit code for interruption.
                // This provides a clean, shell-like experience for the user.
                std::process::exit(130);
            }

        // For all other errors, print a formatted message to stderr and exit with a failure code.
        eprintln!("\n{}: {}", "Error".red().bold(), e);
        std::process::exit(1);
    }
}

/// The main application dispatcher.
///
/// This function is the primary router. It determines the user's intent based on
/// command-line arguments and the environment (e.g., if inside an `axes` session),
/// then routes to the appropriate handler with the correct arguments.
fn run_cli(cli: Cli, cancellation_token: CancellationToken) -> Result<()> {
    log::debug!("CLI args parsed: {:?}", cli);

    let arg1 = match cli.context_or_action {
        Some(a) => a,
        None => {
            // If no arguments are provided, show a placeholder for the future TUI.
            println!("Welcome to axes! (TUI placeholder)");
            return Ok(());
        }
    };

    let mut remaining_args = Vec::new();
    if let Some(arg2) = cli.action_or_context_or_arg {
        remaining_args.push(arg2);
    }
    remaining_args.extend(cli.args);

    let (action_name, action_args) = if env::var("AXES_PROJECT_UUID").is_ok() {
        // --- Session Mode: Strict Grammar ---
        // The context is implicit, so `arg1` must be the action or script name.
        (arg1, remaining_args)
    } else {
        // --- Script Mode: Flexible Grammar ---
        if find_command(&arg1).is_some() {
            // Case: `axes <action> [args...]` (e.g., `axes tree --all`)
            (arg1, remaining_args)
        } else if let Some(arg2) = remaining_args.first() {
            if find_command(arg2).is_some() {
                // Case: `axes <context> <action> [args...]` (e.g., `axes my-app info`)
                let mut args_for_handler = vec![arg1.clone()];
                args_for_handler.extend(remaining_args.iter().skip(1).cloned());
                (arg2.clone(), args_for_handler)
            } else {
                // Case: `axes <context> <script> [params...]` (Shortcut for `run`)
                let mut run_args = vec![arg1.clone()];
                run_args.extend(remaining_args);
                ("run".to_string(), run_args)
            }
        } else {
            // Case: `axes <context>` (Shortcut for `start`)
            ("start".to_string(), vec![arg1])
        }
    };

    // --- Dispatch Logic ---
    if let Some(command) = find_command(&action_name) {
        // A known system command was found. Execute its handler.
        (command.handler)(action_args, &cancellation_token)
    } else {
        // Not a system command, so it's a script name. This is a shortcut for `run`.
        // This case primarily handles session mode (`axes <script>`).
        let mut run_args = vec![action_name];
        run_args.extend(action_args);
        handlers::run::handle(run_args, &cancellation_token)
    }
}
