// EN: src/cli/handlers/open.rs

use anyhow::{Result, anyhow};
use colored::*;

use crate::core::interpolator::Interpolator;
use crate::system::executor;
use crate::CancellationToken;

use clap::Parser;

use super::commons;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct OpenArgs {
    /// The project context to open.
    context: String,
    /// The application key from [options.open_with] to use.
    app_key: Option<String>,
}

/// The main handler for the `open` command.
pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    // 1. Validate arguments: `open` accepts zero or one argument.
    let open_args = OpenArgs::try_parse_from(&args)?;

    // 2. Resolve the project configuration, which is mandatory.
    let config = commons::resolve_config_from_context_or_session(Some(open_args.context), cancellation_token)?;

    // 3. Determine which command template to use based on the provided key or the default.
    let command_template = match open_args.app_key.as_deref() {
        Some(key) => {
            // User provided a specific key, e.g., "vsc"
            if key == "default" {
                return Err(anyhow!(t!("open.error.default_is_reserved")));
            }
            config
                .options
                .open_with
                .get(key)
                .ok_or_else(|| anyhow!(t!("open.error.action_not_found"), key = key))?
                .clone()
        }
        None => {
            // No key provided, so we must use the default.
            // First, get the *name* of the default key.
            let default_key_name = config
                .options
                .open_with
                .get("default")
                .ok_or_else(|| anyhow!(t!("open.error.no_app_no_default")))?;

            // Then, use that name to get the actual command template.
            config
                .options
                .open_with
                .get(default_key_name)
                .ok_or_else(|| {
                    anyhow!(t!("open.error.default_key_invalid"), key = default_key_name)
                })?
                .clone()
        }
    };

    // 4. Interpolate the command template using the new interpolator.
    let mut interpolator = Interpolator::new(&config);
    let final_command = interpolator.expand_string(&command_template)?;

    if final_command.trim().is_empty() {
        println!(
            "{}",
            "Warning: The 'open' command is empty after expansion. Nothing to do.".yellow()
        );
        return Ok(());
    }

    // 5. Execute the final command.
    println!("\n> {}", final_command.green());
    executor::execute_command(&final_command, &config.project_root, &config.env)?;

    Ok(())
}
