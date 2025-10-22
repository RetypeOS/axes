use anyhow::Result;

use crate::{
    cli::handlers::{self, run::parse_script_path},
    state::AppStateGuard,
};

// --- Command Definition and Registry (Moved from main.rs) ---

/// Defines a system command, its aliases, and its new universal handler signature.
struct CommandDefinition {
    name: &'static str,
    aliases: &'static [&'static str],
    handler: fn(Option<String>, Vec<String>, &mut AppStateGuard) -> Result<()>,
}

/// The single source of truth for all system commands.
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

/// The main application dispatcher implementing the new universal grammar.
pub fn dispatch(all_args: Vec<String>, index: &mut AppStateGuard) -> Result<()> {
    log::debug!("Dispatching args: {:?}", all_args);

    if all_args.is_empty() {
        println!("Welcome to axes! (TUI placeholder)");
        return Ok(());
    }

    let arg1 = &all_args[0];
    let arg2 = all_args.get(1);

    // --- Dispatch Logic Cascade (Moved from main.rs) ---
    let (command_def, context, handler_args) = if let Some(arg2_val) = arg2 {
        if arg2_val == "--" {
            // Rule 1 (Escape Hatch): `axes <script_path> -- [params...]`
            let (ctx_part, script_part) = parse_script_path(arg1);
            let mut params = vec![script_part.to_string()];
            params.extend(all_args.iter().skip(2).cloned());
            (
                find_command("run").unwrap(),
                ctx_part.map(|s| s.to_string()),
                params,
            )
        } else if let Some(command) = find_command(arg2_val) {
            // Rule 2 (Explicit Action): `axes <context> <action> [args...]`
            let params = all_args.iter().skip(2).cloned().collect();
            (command, Some(arg1.to_string()), params)
        } else if let Some(command) = find_command(arg1) {
            // Rule 3 (Global Action): `axes <action> [args...]`
            let params = all_args.iter().skip(1).cloned().collect();
            (command, None, params)
        } else {
            // Rule 4 (Default, Implicit Script): `axes <script_path> [params...]`
            let (ctx_part, script_part) = parse_script_path(arg1);
            let mut params = vec![script_part.to_string()];
            params.extend(all_args.iter().skip(1).cloned());
            (
                find_command("run").unwrap(),
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
            ctx_part.map(|s| s.to_string()),
            vec![script_part.to_string()],
        )
    };

    // --- Dispatch to Handler ---
    (command_def.handler)(context, handler_args, index)
}
