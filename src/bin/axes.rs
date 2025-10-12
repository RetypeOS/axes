// EN: src/bin/axes.rs

use std::sync::{Arc, Mutex};

use anyhow::Result;
use axes::{
    cli::{
        Cli,
        handlers::{self, run::parse_script_path},
    },
    core::index_manager,
    models::GlobalIndex,
    system::executor,
};
use clap::Parser;
use colored::*;
use lazy_static::lazy_static;

// Use a thread-safe global static for the index.
lazy_static! {
    static ref GLOBAL_INDEX: Arc<Mutex<GlobalIndex>> = {
        let index =
            index_manager::load_and_ensure_global_project().expect("Failed to load global index.");
        Arc::new(Mutex::new(index))
    };
}

// --- Command Definition and Registry ---

/// Defines a system command, its aliases, and its new universal handler signature.
/// The handler now accepts an optional context and a vector of its specific arguments.
struct CommandDefinition {
    name: &'static str,
    aliases: &'static [&'static str],
    handler: fn(Option<String>, Vec<String>, &mut GlobalIndex) -> Result<()>,
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
        name: "cache",
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
    CommandDefinition {
        name: "repair",
        aliases: &["rep"],
        handler: handlers::repair::handle,
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
    #[cfg(debug_assertions)]
    {
        env_logger::init(); // This is often too verbose, let's keep it commented for now.
    }

    // 1. Load the index and clone its initial state.
    let index_initial_state = {
        let index_guard = GLOBAL_INDEX.lock().unwrap();
        (*index_guard).clone()
    };

    if let Err(e) = run_cli(Cli::parse()) {
        // --- Graceful handling for clap's informational exits (`--help`, `--version`) ---
        // Before treating the error as a generic failure, check if it's a special
        // error from clap that should result in a clean exit.
        if let Some(clap_err) = e.downcast_ref::<clap::Error>() {
            // `use_stderr()` is clap's idiomatic way to distinguish between:
            // - `false`: Informational exits like --help (print to stdout, exit 0).
            // - `true`: Actual parsing errors (print to stderr, exit 1).
            if !clap_err.use_stderr() {
                // This is a --help or --version request.
                clap_err.print().expect("Failed to print clap help/version");
                std::process::exit(0);
            }
        }

        // --- Existing error handling for all other application errors ---
        if let Some(exec_err) = e.downcast_ref::<executor::ExecutionError>()
            && matches!(exec_err, executor::ExecutionError::Interrupted { .. })
        {
            eprintln!(); // Print a newline after ^C for clean terminal output.
            std::process::exit(130); // Standard exit code for Ctrl+C.
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

    // 3. At the very end, check if the index has changed and save if needed.
    let index_final_state = GLOBAL_INDEX.lock().unwrap();
    if *index_final_state != index_initial_state {
        if let Err(e) = index_manager::save_global_index(&index_final_state) {
            eprintln!(
                "\n{}: Failed to save updated global index: {}",
                "Critical Error".red().bold(),
                e
            );
            std::process::exit(1);
        }
        log::debug!("Global index was modified and has been saved.");
    }
}

/// The main application dispatcher implementing the new universal grammar.
fn run_cli(cli: Cli) -> Result<()> {
    log::debug!("CLI args parsed: {:?}", cli);

    // Get a mutable guard to the global index early.
    // It will be passed down to handlers that need to mutate the state.
    let mut index_guard = GLOBAL_INDEX.lock().unwrap();

    let all_args = cli.args;

    if all_args.is_empty() {
        println!("Welcome to axes! (TUI placeholder)");
        return Ok(());
    }

    let arg1 = &all_args[0];
    let arg2 = all_args.get(1);

    // --- New Dispatch Logic Cascade ---
    let (command_def, context, handler_args) = if let Some(arg2_val) = arg2 {
        if arg2_val == "--" {
            // Rule 1 (Escape Hatch): `axes <script_path> -- [params...]`
            let (ctx_part, script_part) = parse_script_path(arg1);
            let mut params = vec![script_part.to_string()];
            params.extend(all_args.iter().skip(2).cloned());
            (
                find_command("run").unwrap(),
                // FIX: Convert Option<&str> to Option<String>
                ctx_part.map(|s| s.to_string()),
                params,
            )
        } else if let Some(command) = find_command(arg2_val) {
            // Rule 2 (Explicit Action): `axes <context> <action> [args...]`
            let params = all_args.iter().skip(2).cloned().collect();
            // This branch correctly returns Option<String>
            (command, Some(arg1.to_string()), params)
        } else if let Some(command) = find_command(arg1) {
            // Rule 3 (Global Action): `axes <action> [args...]`
            let params = all_args.iter().skip(1).cloned().collect();
            // This branch correctly returns Option<String> (via `None`)
            (command, None, params)
        } else {
            // Rule 4 (Default, Implicit Script): `axes <script_path> [params...]`
            let (ctx_part, script_part) = parse_script_path(arg1);
            let mut params = vec![script_part.to_string()];
            params.extend(all_args.iter().skip(1).cloned());
            (
                find_command("run").unwrap(),
                // FIX: Convert Option<&str> to Option<String>
                ctx_part.map(|s| s.to_string()),
                params,
            )
        }
    } else if let Some(command) = find_command(arg1) {
        // Rule 3 with a single argument
        (command, None, vec![])
    } else {
        // Rule 4 with a single argument
        let (ctx_part, script_part) = parse_script_path(arg1);
        (
            find_command("run").unwrap(),
            // FIX: Convert Option<&str> to Option<String>
            ctx_part.map(|s| s.to_string()),
            vec![script_part.to_string()],
        )
    };

    // --- Dispatch to Handler ---
    (command_def.handler)(context, handler_args, &mut index_guard)
}
