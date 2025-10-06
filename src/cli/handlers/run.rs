// EN: src/cli/handlers/run.rs (FINALIZED FOR LAZY ARCHITECTURE)

use crate::{
    cli::handlers::commons,
    core::{parameters::ArgResolver, task_executor},
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
/// It receives a normalized context and the script name as the first argument,
/// then orchestrates the lazy execution of the corresponding task.
///
pub fn handle(
    context: Option<String>,
    mut args: Vec<String>,
    index: &mut GlobalIndex,
) -> Result<()> {
    // 1. The dispatcher guarantees that the script name is the first argument.
    if args.is_empty() {
        return Err(anyhow!(
            "Internal error: 'run' handler called without a script name."
        ));
    }
    let script_name = args.remove(0);
    let params = args;

    // 2. Get the LAZY `ResolvedConfig` facade for the given context.
    let config = commons::resolve_config_for_context(context, index)?;

    // 3. Lazily get the top-level task for the requested script.
    //    FIX: Start the recursion depth count at 0.
    let task = config.get_script(&script_name, 0)?.ok_or_else(|| {
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
            CommandAction::Execute(t) | CommandAction::Print(t) => t.to_vec(),
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

    // 6. Execute the task.
    task_executor::execute_task(&task, &config, &resolver, index)?;

    Ok(())
}
