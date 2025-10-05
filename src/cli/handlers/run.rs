// src/cli/handlers/run.rs

use crate::{
    cli::handlers::commons,
    core::{config_resolver, index_manager, parameters::ArgResolver, task_executor},
    models::{CommandAction, TemplateComponent},
};
use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct RunArgs {
    /// The name of the script to run.
    script: String,
    /// Parameters to pass to the script.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

// Esta función es llamada por el despachador.
pub fn parse_script_path(full_path: &str) -> (Option<String>, &str) {
    if let Some((context, script_name)) = full_path.rsplit_once('/') {
        (Some(context.to_string()), script_name)
    } else {
        (None, full_path)
    }
}

///
/// Main entry point for the 'run' command.
/// It now receives the full script path (e.g., "my-app/build") as its context
/// and is responsible for parsing it.
///
pub fn handle(context: Option<String>, mut args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        return Err(anyhow!(
            "Internal error: 'run' handler called without a script name."
        ));
    }

    let script_name = args.remove(0);
    let params = args;

    let mut index = index_manager::load_and_ensure_global_project()?;
    let mut config = commons::resolve_config_and_update_index_if_needed(context, &mut index)?;

    // 4. Resolve the script into a `Task` object.
    let task = config_resolver::resolve_script_task(&mut config, &script_name, &index)?;

    if task.commands.is_empty() {
        println!("{}", "Script is empty. Nothing to execute.".yellow());
        return Ok(());
    }

    // 5. Collect parameter definitions from the task.
    let all_definitions: Vec<_> = task
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

    // 6. Create the `ArgResolver`. The `params` are now directly the `args` vector
    //    passed to this handler.
    let resolver = ArgResolver::new(&all_definitions, &params, has_generic_params)?;

    // 7. Execute the task.
    task_executor::execute_task(&task, &config, &resolver)?;
    
    Ok(())
}
