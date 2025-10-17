use crate::{
    cli::handlers::commons,
    core::task_executor,
    models::{GlobalIndex, ResolvedConfig, ResolvedOpenWithConfig},
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
    app_key: Option<String>,
    #[arg(long, short)]
    list: bool,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}
pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    let open_args = OpenArgs::try_parse_from(&args)?;
    let config = commons::resolve_config_for_context(context.clone(), index)?;
    let options = config.get_options()?;
    if open_args.list {
        return list_open_commands(&config.qualified_name, &options.open_with);
    }
    execute_open_command(open_args, config, options.open_with, index)
}
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
    config: ResolvedConfig,
    open_with: ResolvedOpenWithConfig,
    index: &mut GlobalIndex,
) -> Result<()> {
    // 1. Determine which application key to use.
    let app_key_to_use = open_args
        .app_key
        .as_deref()
        .or(open_with.default.as_deref())
        .ok_or_else(|| anyhow!(t!("open.error.no_default")))?;

    // 2. Get the universal Task AST.
    let task_universal = open_with.commands.get(app_key_to_use).ok_or_else(|| {
        // ROBUSTNESS: Check if the missing key was the configured default for a better error message.
        if Some(app_key_to_use) == open_with.default.as_deref() {
            anyhow!(
                t!("open.error.default_not_found"),
                key = app_key_to_use.cyan()
            )
        } else {
            anyhow!(t!("open.error.key_not_found"), key = app_key_to_use.cyan())
        }
    })?;

    // 3. CRITICAL: Flatten the task to resolve any compositions BEFORE further processing.
    let task_flattened = config.flatten_task(task_universal)?;

    let resolver = commons::build_resolver_for_task(&task_flattened, &open_args.params)?;

    // 6. Execute the flattened task.
    println!(
        "\n🚀 Opening '{}' with '{}'...",
        config.qualified_name.cyan(),
        app_key_to_use.cyan()
    );
    // Pass the flattened task to the executor.
    task_executor::execute_task(&task_flattened, &config, &resolver, index)?;

    Ok(())
}
