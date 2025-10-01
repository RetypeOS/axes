// EN: src/bin/axes.rs

use anyhow::Result;
use axes::{
    cli::{Cli, handlers},
    system::executor,
};
use clap::Parser;
use colored::*;

// --- Command Definition and Registry ---

/// Defines a system command, its aliases, and its new universal handler signature.
/// The handler now accepts an optional context and a vector of its specific arguments.
struct CommandDefinition {
    name: &'static str,
    aliases: &'static [&'static str],
    handler: fn(Option<String>, Vec<String>) -> Result<()>,
}

/// The single source of truth for all system commands.
/// The registry is updated to match the new handler signature.
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
fn main() {
    //#[cfg(debug_assertions)]
    //{
        env_logger::init();
    //}
    if let Err(e) = run_cli(Cli::parse()) {
        if let Some(exec_err) = e.downcast_ref::<executor::ExecutionError>()
            && matches!(exec_err, executor::ExecutionError::Interrupted { .. })
        {
            eprintln!();
            std::process::exit(130);
        }

        eprintln!("\n{}: {}", "Error".red().bold(), e);
        let mut causes = e.chain().skip(1);
        if let Some(cause) = causes.next() {
            eprintln!("\nCaused by:");
            eprintln!("   0: {}", cause);
            for (i, cause) in causes.enumerate() {
                eprintln!("   {}: {}", i + 1, cause);
            }
        }
        std::process::exit(1);
    }
}

/// The main application dispatcher implementing the universal grammar.
fn run_cli(cli: Cli) -> Result<()> {
    log::debug!("CLI args parsed: {:?}", cli);

    // --- Argument Collection ---
    let mut all_args = Vec::new();
    if let Some(arg1) = cli.context_or_action {
        all_args.push(arg1);
    }
    if let Some(arg2) = cli.action_or_context_or_arg {
        all_args.push(arg2);
    }
    all_args.extend(cli.args);

    if all_args.is_empty() {
        println!("Welcome to axes! (TUI placeholder)");
        return Ok(());
    }

    // --- Universal Dispatch Logic Cascade ---
    let (action_name, context, handler_args) = {
        // We clone the first two arguments for checks, leaving `all_args` untouched.
        let arg1 = all_args.first().cloned();
        let arg2 = all_args.get(1).cloned();

        if let Some(arg2_val) = arg2 {
            if find_command(&arg2_val).is_some() {
                // Grammar 1: `axes <context> <action> [args...]`
                // Action is arg2. Context is arg1. Handler gets args from arg3 onwards.
                let handler_args = all_args.into_iter().skip(2).collect();
                (arg2_val, arg1, handler_args)
            } else if let Some(arg1_val) = arg1.as_ref() {
                if find_command(arg1_val).is_some() {
                    // Grammar 2: `axes <action> [args...]`
                    // Action is arg1. No context. Handler gets args from arg2 onwards.
                    let handler_args = all_args.into_iter().skip(1).collect();
                    (arg1_val.clone(), None, handler_args)
                } else {
                    // Grammar 3 (Default): `axes <script> [params...]` -> run
                    // Neither arg1 nor arg2 is an action. Default to `run`. No context.
                    // Handler for `run` gets all arguments, including the script name.
                    ("run".to_string(), None, all_args)
                }
            } else {
                // This case should be impossible if all_args is not empty, but for safety:
                ("run".to_string(), None, all_args)
            }
        } else if let Some(arg1_val) = arg1.as_ref() {
            if find_command(arg1_val).is_some() {
                // Grammar 2 (with only 1 arg): `axes <action>`
                // Action is arg1. No context. No args for handler.
                (arg1_val.clone(), None, vec![])
            } else {
                // Grammar 3 (Default with only 1 arg): `axes <script_or_context>`
                // Following the new logic, we simplify this: a single non-command argument
                // is always a script execution with an implicit context.
                // The explicit `start` command must be used: `axes my-app start`.
                ("run".to_string(), None, all_args)
            }
        } else {
            // This case is impossible because we checked for `all_args.is_empty()` at the start.
            unreachable!();
        }
    };

    // --- Dispatch to Handler ---
    if let Some(command) = find_command(&action_name) {
        (command.handler)(context, handler_args)
    } else {
        // This is now impossible, as the default case always resolves to "run".
        unreachable!();
    }
}
