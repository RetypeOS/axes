// EN: src/cli/handlers/start.rs

use crate::{
    cli::handlers::commons,
    core::{
        config_resolver,
        index_manager,
        parameters::{ArgResolver},
    },
    models::{ParameterDef, Task, TemplateComponent},
    system::shell,
    CancellationToken,
};
use anyhow::{anyhow, Result};
use clap::Parser;
//use colored::*;
use std::collections::HashSet;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct StartArgs {
    /// The project context to start a session in.
    context: String,
    /// Parameters to pass to the at_start and at_exit scripts.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

///
/// Main entry point for the `start` command.
/// Orchestrates session setup, including parameter resolution for `at_start` and `at_exit`.
///
pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    // 1. Parse args and resolve config.
    let start_args = StartArgs::try_parse_from(&args)?;
    if std::env::var("AXES_PROJECT_UUID").is_ok() {
        return Err(anyhow!("Cannot start a nested session. Please `exit` the current one first."));
    }
    let index = index_manager::load_and_ensure_global_project()?;
    let mut config =
        commons::resolve_config_from_context_or_session(Some(start_args.context), &index, cancellation_token)?;

    // 2. Resolve `at_start` and `at_exit` into `Task` objects. This is lazy.
    let task_start = if config.options.at_start.is_some() {
        Some(config_resolver::resolve_task(&mut config, "options::at_start")?.clone())
    } else {
        None
    };
    let task_exit = if config.options.at_exit.is_some() {
        Some(config_resolver::resolve_task(&mut config, "options::at_exit")?.clone())
    } else {
        None
    };

    // 3. Collect and validate parameter definitions from BOTH tasks.
    let mut all_definitions = Vec::new();
    if let Some(task) = &task_start {
        all_definitions.extend(get_definitions_from_task(task));
    }
    if let Some(task) = &task_exit {
        all_definitions.extend(get_definitions_from_task(task));
    }

    // Validate that definitions are compatible (no duplicates).
    let mut seen_defs = HashSet::new();
    for def in &all_definitions {
        if !seen_defs.insert(&def.kind) {
            return Err(anyhow!(
                "Incompatible parameter definitions: The parameter '{:?}' is defined in both `at_start` and `at_exit` in a conflicting way or duplicated.",
                def.kind
            ));
        }
    }
    
    let has_generic_params = task_start.iter().chain(task_exit.iter())
        .flat_map(|task| &task.commands)
        .flat_map(|cmd| &cmd.template)
        .any(|component| matches!(component, TemplateComponent::GenericParams));
    // 4. Create a single ArgResolver for the entire session.
    let resolver = ArgResolver::new(&all_definitions, &start_args.params, has_generic_params)?;

    // 5. Delegate to the shell module to launch the session.
    shell::launch_session(&config, task_start, task_exit, &resolver, cancellation_token)?;
    
    // 6. Persist cache changes.
    config_resolver::save_config_cache(&config, &index)?;

    Ok(())
}

/// Helper to extract ParameterDefs from a Task's components.
fn get_definitions_from_task(task: &Task) -> Vec<ParameterDef> {
    task.commands.iter()
        .flat_map(|cmd| &cmd.template)
        .filter_map(|component| match component {
            TemplateComponent::Parameter(def) => Some(def.clone()),
            _ => None,
        })
        .collect()
}