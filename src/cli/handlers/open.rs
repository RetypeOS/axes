// src/cli/handlers/open.rs

use crate::{
    cli::handlers::commons,
    core::{config_resolver, index_manager, parameters::ArgResolver, task_executor},
    models::{CommandAction, GlobalIndex, ParameterDef, TemplateComponent},
};
use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct OpenArgs {
    /// The application key from [options.open_with] to use. If omitted, uses 'default'.
    app_key: Option<String>,
    /// Parameters to pass to the open command.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. Parse args and resolve config.
    let open_args = OpenArgs::try_parse_from(&args)?;
    let context_str = context.unwrap_or_else(|| ".".to_string());
    let mut index = index_manager::load_and_ensure_global_project()?;
    let mut config = commons::resolve_config_and_update_index_if_needed(Some(context_str), &mut index)?;

    // 2. Determine which `open_with` command to use, correctly handling the 'default' key.
    let app_key_from_user = open_args.app_key.as_deref().unwrap_or("default");

    let final_key = if app_key_from_user == "default" {
        app_key_from_user.to_string()
    } else {
        app_key_from_user.to_string()
    };

    let task = config.options.open_with.get(&final_key)
        .ok_or_else(|| anyhow!("Application key '{}' not found in [options.open_with].", final_key))?
        .clone(); // Clone to own the task

    // 3. Resolve the command into a Task.
    //let task = config_resolver::resolve_open_with_task(&mut config, &final_key, &index)?;

    // 4. Collect definitions and resolve arguments for the task.
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
    println!("\n🚀 Opening with '{}'...", final_key.cyan());
    task_executor::execute_task(&task, &config, &resolver)?;

    Ok(())
}
