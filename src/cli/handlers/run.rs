use crate::{
    cli::handlers::commons,
    core::{parameters::ArgResolver, task_executor},
    // FIX: Removed unused `index_manager` and `config_resolver` imports
    // FIX: Added `ParameterDef` for collecting definitions
    models::{CommandAction, GlobalIndex, ParameterDef, TemplateComponent},
};
use anyhow::{Result, anyhow};
use colored::*;

/// Parses a script path string like "my-app/api/build" into its context and script name.
/// This is a helper function used internally by the main dispatcher (`bin/axes.rs`).
pub fn parse_script_path(full_path: &str) -> (Option<&str>, &str) {
    if let Some((context, script_name)) = full_path.rsplit_once('/') {
        (Some(context), script_name)
    } else {
        (None, full_path)
    }
}

///
/// Main entry point for the 'run' command.
/// It receives a normalized context and the script name as the first argument.
///
pub fn handle(context: Option<String>, mut args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. The dispatcher guarantees that the script name is the first argument.
    if args.is_empty() {
        return Err(anyhow!("Internal error: 'run' handler called without a script name."));
    }
    let script_name = args.remove(0);
    let params = args; // The rest of the arguments are parameters for the script.

    // 2. Resolve the configuration for the given context.
    let config = commons::resolve_config_for_context(context, index)?;

    // 3. Get the pre-compiled Task for the requested script from the resolved config.
    let task = config.scripts.get(&script_name).ok_or_else(|| {
        anyhow!(
            "Script '{}' not found in project '{}'.",
            script_name.cyan(),
            config.qualified_name.yellow()
        )
    })?;

    if task.commands.is_empty() {
        println!("{}", "Script is empty. Nothing to execute.".yellow());
        return Ok(());
    }

    // 4. Collect all parameter definitions from every command in the task.
    let all_definitions: Vec<ParameterDef> = task
        .commands
        .iter()
        .flat_map(|cmd| match &cmd.action {
            CommandAction::Execute(t) | CommandAction::Print(t) => t.iter().cloned().collect::<Vec<_>>(),
        })
        .filter_map(|component| match component {
            TemplateComponent::Parameter(def) => Some(def),
            _ => None,
        })
        .collect();

    let has_generic_params = task
        .commands
        .iter()
        .flat_map(|cmd| match &cmd.action {
            CommandAction::Execute(t) | CommandAction::Print(t) => t.iter(),
        })
        .any(|c| matches!(c, TemplateComponent::GenericParams));

    // 5. Create a single `ArgResolver` for the entire task.
    let resolver = ArgResolver::new(&all_definitions, &params, has_generic_params)?;

    // 6. Execute the task using the shared task executor.
    task_executor::execute_task(task, &config, &resolver)?;

    Ok(())
}