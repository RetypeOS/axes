// EN: src/cli/handlers/start.rs

use anyhow::{Context, Result, anyhow};
use std::env;

use super::commons;
use crate::{system::shell, CancellationToken};

use clap::Parser;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct StartArgs {
    /// The project context to start a session in.
    context: Option<String>,
}

pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    // 1. Parse args.
    let start_args = StartArgs::try_parse_from(&args)?;

    // 2. Robustness: Prevent nested sessions.
    if env::var("AXES_PROJECT_UUID").is_ok() && start_args.context.is_some() {
        return Err(anyhow!(
            "Cannot start a nested session. Please `exit` the current session before starting a new one."
        ));
    }

    // 3. Resolve the project configuration. This requires a context.
    let config = commons::resolve_config_from_context_or_session(start_args.context, cancellation_token)?;

    // 4. Provide feedback to the user before launching the shell.
    println!(
        t!("start.info.starting_session"),
        name = config.qualified_name
    );

    // 5. Delegate the core logic to the shell module.
    shell::launch_interactive_shell(&config).with_context(|| {
        anyhow!(
            t!("start.error.session_failed"),
            name = config.qualified_name
        )
    })
}
