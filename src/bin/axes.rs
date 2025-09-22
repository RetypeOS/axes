// EN: src/bin/axes.rs

use anyhow::Result;
use axes::cli::{handlers, Cli};
use axes::t;
use axes::CancellationToken;
use clap::Parser;
use colored::*;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio;
use dialoguer::Error as DialoguerError;

// --- Command Definition and Registry ---

/// A token representing the cancellation state of the application.
/// It is shared between the `ctrlc` handler and the command handlers.

/// Defines a system command, its aliases, and its handler function.
/// The handler now accepts a `CancellationToken` to allow for safe interruption.
struct CommandDefinition {
    name: &'static str,
    aliases: &'static [&'static str],
    handler: fn(Vec<String>, &CancellationToken) -> Result<()>,
}

/// The single source of truth for all system commands.
/// To add a new command, simply add a new entry here.
static COMMAND_REGISTRY: &[CommandDefinition] = &[
    CommandDefinition { name: "alias", aliases: &[], handler: handlers::alias::handle },
    CommandDefinition { name: "delete", aliases: &["del"], handler: handlers::delete::handle },
    CommandDefinition { name: "info", aliases: &[], handler: handlers::info::handle },
    CommandDefinition { name: "init", aliases: &["new"], handler: handlers::init::handle },
    CommandDefinition { name: "link", aliases: &[], handler: handlers::link::handle },
    CommandDefinition { name: "open", aliases: &[], handler: handlers::open::handle },
    CommandDefinition { name: "register", aliases: &["reg"], handler: handlers::register::handle },
    CommandDefinition { name: "rename", aliases: &[], handler: handlers::rename::handle },
    CommandDefinition { name: "run", aliases: &[], handler: handlers::run::handle },
    CommandDefinition { name: "start", aliases: &[], handler: handlers::start::handle },
    CommandDefinition { name: "tree", aliases: &["ls"], handler: handlers::tree::handle },
    CommandDefinition { name: "unregister", aliases: &["unreg"], handler: handlers::unregister::handle },
];

/// Finds a command definition in the registry by its name or alias.
fn find_command(name: &str) -> Option<&'static CommandDefinition> {
    COMMAND_REGISTRY.iter().find(|cmd| cmd.name == name || cmd.aliases.contains(&name))
}

/// The main entry point of the application.
#[tokio::main]
async fn main() -> Result<()> {
    let cancellation_token = Arc::new(AtomicBool::new(true));
    env_logger::init();
    let cli = Cli::parse();

    let signal_token = cancellation_token.clone();
    let main_logic_token = cancellation_token.clone();

    tokio::select! {
        // Tarea A: La lógica principal de la aplicación.
        result = run_cli_wrapper(cli, main_logic_token) => {
            if let Err(e) = result {
                if cancellation_token.load(Ordering::SeqCst) == false {
                    std::process::exit(130);
                } else {
                    eprintln!("\n{}: {}", "Error".red().bold(), e);
                    std::process::exit(1);
                }
            }
        }

        // Tarea B: Esperar la señal de Ctrl+C.
        _ = tokio::signal::ctrl_c() => {
            if signal_token.load(Ordering::SeqCst) {
                signal_token.store(false, Ordering::SeqCst);
                println!(
                    "\n{}",
                    t!("common.info.cancellation_requested_interactive").yellow()
                );
            }
        }
    }
    Ok(())
}

/// A wrapper function needed because `tokio::select!` requires the futures to be `Send`.
/// `run_cli` en sí no es `async`, por lo que lo envolvemos en un `tokio::task::spawn_blocking`.
async fn run_cli_wrapper(cli: Cli, cancellation_token: CancellationToken) -> Result<()> {
    tokio::task::spawn_blocking(move || run_cli(cli, cancellation_token)).await?
}

/// The main application dispatcher.
///
/// This function is the primary router for the application. It determines the user's
/// intent based on command-line arguments and the environment, then routes
/// to the appropriate handler. It is designed to be highly maintainable and declarative.
///
/// # Arguments
/// * `cli`: The parsed command-line arguments from `clap`.
/// * `cancellation_token`: A shared token to signal graceful shutdown on Ctrl+C.
fn run_cli(cli: Cli, cancellation_token: CancellationToken) -> Result<()> {
    log::debug!("CLI args parsed: {:?}", cli);

    let arg1 = match cli.context_or_action {
        Some(a) => a,
        None => {
            println!("{}", t!("common.info.tui_placeholder"));
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
        // `arg1` is always the action or script name.
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
        // A system command was found. Execute its handler.
        (command.handler)(action_args, &cancellation_token)
    } else {
        // Not a system command, so it's a shortcut for `run`.
        let mut run_args = vec![action_name];
        run_args.extend(action_args);
        handlers::run::handle(run_args, &cancellation_token)
    }
}