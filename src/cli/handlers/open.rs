// src/cli/handlers/open.rs (REBUILT FOR FUNCTIONALITY AND MODULARITY)

use crate::{
    cli::handlers::commons,
    core::{parameters::ArgResolver, task_executor},
    models::{GlobalIndex, ResolvedOpenWithConfig},
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

pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    let open_args = OpenArgs::try_parse_from(&args)?;
    let config = commons::resolve_config_for_context(context.clone(), index)?;
    let options = config.get_options()?;

    if open_args.list {
        // If --list is passed, display available commands and exit.
        return list_open_commands(&config.qualified_name, &options.open_with);
    }

    // Otherwise, proceed to execute a command.
    execute_open_command(open_args, config, options.open_with, index)
}

// --- Subcommand Logic ---

/// [NEW] Handles the logic for listing available `open_with` commands.
fn list_open_commands(project_name: &str, open_with: &ResolvedOpenWithConfig) -> Result<()> {
    println!("\nAvailable `open` commands for '{}':", project_name.cyan());

    if open_with.commands.is_empty() {
        println!("  {}", "No commands defined.".dimmed());
        return Ok(());
    }

    let mut sorted_keys: Vec<_> = open_with.commands.keys().collect();
    sorted_keys.sort();

    let max_len = sorted_keys
        .iter()
        .map(|k| measure_text_width(k))
        .max()
        .unwrap_or(0);

    for key in sorted_keys {
        let task = open_with.commands.get(key).unwrap();
        let padding = " ".repeat(max_len - measure_text_width(key));

        print!("  - {}{} ", key.green(), padding);

        if Some(key.as_str()) == open_with.default.as_deref() {
            print!("{} ", t!("common.label.default").yellow());
        }

        if let Some(desc) = &task.desc {
            print!("{}", desc.dimmed());
        }
        println!();
    }
    Ok(())
}

/// Handles the logic for executing a specific `open_with` command.
fn execute_open_command(
    open_args: OpenArgs,
    config: crate::models::ResolvedConfig,
    open_with: ResolvedOpenWithConfig,
    index: &mut GlobalIndex,
) -> Result<()> {
    // 1. Determine which application key to use.
    let app_key_to_use = match open_args.app_key {
        Some(key) => key,
        None => open_with
            .default
            .clone()
            .ok_or_else(|| anyhow!(t!("open.error.no_default")))?,
    };

    // 2. Get the compiled Task, providing a more specific error if the key is not found.
    let task = open_with
        .commands
        .get(&app_key_to_use)
        .ok_or_else(|| {
            // Check if the missing key was the one configured as default for a better error.
            if Some(&app_key_to_use) == open_with.default.as_ref() {
                anyhow!(
                    t!("open.error.default_not_found"),
                    key = app_key_to_use.cyan()
                )
            } else {
                anyhow!(t!("open.error.key_not_found"), key = app_key_to_use.cyan())
            }
        })?
        .clone(); // Clone the Arc<Task>

    // 3. Collect parameter definitions using the new shared utility function.
    let definitions = commons::collect_parameter_defs_from_task(&task, &config);

    let has_generic_params = task.commands.iter().any(|plat_exec| {
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
                crate::models::CommandAction::Execute(t)
                | crate::models::CommandAction::Print(t) => t,
            };
            template
                .iter()
                .any(|c| matches!(c, crate::models::TemplateComponent::GenericParams { .. }))
        })
    });

    // 4. Create the argument resolver.
    let resolver = ArgResolver::new(&definitions, &open_args.params, has_generic_params)?;

    // 5. Execute the task.
    println!(
        "\n🚀 Opening '{}' with '{}'...",
        config.qualified_name.cyan(),
        app_key_to_use.cyan()
    );
    task_executor::execute_task(&task, &config, &resolver, index)?;

    Ok(())
}
