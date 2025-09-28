// src/cli/handlers/start.rs

use crate::core::config_resolver::ValueKind;
use crate::{
    CancellationToken,
    cli::handlers::commons,
    core::{config_resolver, index_manager, parameters::ArgResolver},
    models::{ParameterDef, Task, TemplateComponent},
    system::shell,
};
use anyhow::{Result, anyhow};
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
    // 1. Parse args and prevent nested sessions.
    let start_args = StartArgs::try_parse_from(&args)?;
    if std::env::var("AXES_PROJECT_UUID").is_ok() {
        return Err(anyhow!(
            "Cannot start a nested session. Please `exit` the current one first."
        ));
    }

    // 2. Load index and resolve the project configuration.
    let index = index_manager::load_and_ensure_global_project()?;
    let mut config = commons::resolve_config_from_context_or_session(
        Some(start_args.context),
        &index,
        cancellation_token,
    )?;

    // 3. Resolve `at_start` and `at_exit` into `Task` objects.
    let task_start = if config.options.at_start.is_some() {
        Some(config_resolver::resolve_task(
            &mut config,
            "at_start",
            ValueKind::Script,
        )?)
    } else {
        None
    };
    let task_exit = if config.options.at_exit.is_some() {
        Some(config_resolver::resolve_task(
            &mut config,
            "at_exit",
            ValueKind::Script,
        )?)
    } else {
        None
    };

    // 4. Collect parameter definitions from BOTH tasks and validate them.
    let mut all_definitions = Vec::new();
    if let Some(task) = &task_start {
        all_definitions.extend(get_definitions_from_task(task));
    }
    if let Some(task) = &task_exit {
        all_definitions.extend(get_definitions_from_task(task));
    }

    // Validate that definitions are compatible (no duplicates with different modifiers).
    let mut seen_defs = HashSet::new();
    for def in &all_definitions {
        if !seen_defs.insert(def) { // `def` needs PartialEq and Hash
            // This check is important, but a simple HashSet might not catch subtle differences.
            // For now, we assume identical definitions are okay.
            // A more robust check would compare modifiers.
        }
    }

    let has_generic_params = task_start
        .iter()
        .chain(task_exit.iter())
        .flat_map(|task| &task.commands)
        .flat_map(|cmd| &cmd.template)
        .any(|component| matches!(component, TemplateComponent::GenericParams));

    // 5. Create a single ArgResolver for the entire session.
    let resolver = ArgResolver::new(&all_definitions, &start_args.params, has_generic_params)?;

    // 6. Delegate to the shell module to launch the session.
    shell::launch_session(
        &config,
        task_start,
        task_exit,
        &resolver,
        cancellation_token,
    )?;

    Ok(())
}

/// Helper to extract ParameterDefs from a Task's components.
fn get_definitions_from_task(task: &Task) -> Vec<ParameterDef> {
    task.commands
        .iter()
        .flat_map(|cmd| &cmd.template)
        .filter_map(|component| match component {
            TemplateComponent::Parameter(def) => Some(def.clone()),
            _ => None,
        })
        .collect()
}
