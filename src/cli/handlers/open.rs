// src/cli/handlers/open.rs (CORRECTO Y FUNCIONAL)

use crate::{
    cli::handlers::commons,
    core::{parameters::ArgResolver, task_executor},
    models::{CommandAction, GlobalIndex, ParameterDef, TemplateComponent},
};
use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct OpenArgs {
    /// The application key from [options.open_with] to use. If omitted, uses the configured default.
    app_key: Option<String>,
    /// Parameters to pass to the open command.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. Parse args and resolve config (lazy).
    let open_args = OpenArgs::try_parse_from(&args)?;
    let config = commons::resolve_config_for_context(context, index)?;
    // `get_options` now correctly returns a `ResolvedOptionsConfig`.
    let options = config.get_options()?;

    // 2. Determine which `open_with` command to use. This now compiles successfully.
    let app_key_to_use = open_args
        .app_key
        .or(options.open_with.default)
        .ok_or_else(|| {
            anyhow!(
                "No application key provided and no 'default' is configured in [options.open_with]."
            )
        })?;

    // 3. Get the compiled Task from the `commands` map. This also compiles successfully.
    let task = options
        .open_with
        .commands
        .get(&app_key_to_use)
        .ok_or_else(|| {
            anyhow!(
                "Application key '{}' not found in [options.open_with] definitions.",
                app_key_to_use.cyan()
            )
        })?
        .clone(); // Clone the Arc<Task>

    // 4. Collect parameter definitions and resolve arguments for the task.
    let definitions: Vec<ParameterDef> = task
        .commands
        .iter()
        .flat_map(|cmd| match &cmd.action {
            CommandAction::Execute(t) | CommandAction::Print(t) => t.iter().collect::<Vec<_>>(),
        })
        .filter_map(|component| match component {
            TemplateComponent::Parameter(def) => Some(def.clone()),
            _ => None,
        })
        .collect();

    let has_generic_params = task
        .commands
        .iter()
        .flat_map(|cmd| match &cmd.action {
            CommandAction::Execute(t) | CommandAction::Print(t) => t.iter().collect::<Vec<_>>(),
        })
        .any(|c| matches!(c, TemplateComponent::GenericParams));

    let resolver = ArgResolver::new(&definitions, &open_args.params, has_generic_params)?;

    // 5. Execute the task.
    println!("\n🚀 Opening with '{}'...", app_key_to_use.cyan());
    task_executor::execute_task(&task, &config, &resolver, index)?;

    Ok(())
}
