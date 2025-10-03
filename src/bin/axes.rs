// EN: src/bin/axes.rs

use anyhow::Result;
use axes::{
    cli::{
        Cli,
        handlers::{self, run::parse_script_path},
    },
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
    #[cfg(debug_assertions)]
    {
        env_logger::init();
    }
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

/// The main application dispatcher implementing the new universal grammar.
fn run_cli(cli: Cli) -> Result<()> {
    log::debug!("CLI args parsed: {:?}", cli);

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
            // Regla 1 (Escape Hatch): `axes <ruta_script> -- [params...]`
            // Normalizamos la ruta de script aquí mismo.
            let (ctx_part, script_part) = parse_script_path(arg1);
            let mut params = vec![script_part.to_string()];
            params.extend(all_args.iter().skip(2).cloned());
            (
                find_command("run").unwrap(),
                ctx_part.map(|s| s.to_string()),
                params,
            )
        } else if let Some(command) = find_command(arg2_val) {
            // Regla 2 (Acción Explícita): `axes <contexto> <acción> [args...]`
            // El `run` explícito (`axes proj run build`) entra por aquí.
            let params = all_args.iter().skip(2).cloned().collect();
            (command, Some(arg1.to_string()), params)
        } else if let Some(command) = find_command(arg1) {
            // Regla 3 (Acción Global): `axes <acción> [args...]`
            let params = all_args.iter().skip(1).cloned().collect();
            (command, None, params)
        } else {
            // Regla 4 (Por Defecto, Script Implícito): `axes <ruta_script> [params...]`
            // Normalizamos la ruta de script.
            let (ctx_part, script_part) = parse_script_path(arg1);
            let mut params = vec![script_part.to_string()];
            params.extend(all_args.iter().skip(1).cloned());
            (find_command("run").unwrap(), ctx_part, params)
        }
    } else if let Some(command) = find_command(arg1) {
        // Regla 3 con un solo argumento
        (command, None, vec![])
    } else {
        // Regla 4 con un solo argumento
        let (ctx_part, script_part) = parse_script_path(arg1);
        (
            find_command("run").unwrap(),
            ctx_part.map(|s| s.to_string()),
            vec![script_part.to_string()],
        )
    };

    // --- Dispatch to Handler ---
    (command_def.handler)(context, handler_args)
}
