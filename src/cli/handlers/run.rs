// src/cli/handlers/run.rs

use crate::{
    CancellationToken,
    cli::handlers::commons,
    core::{
        config_resolver::{self, ValueKind},
        index_manager,
        parameters::ArgResolver,
        task_executor,
    },
    models::TemplateComponent,
};
use anyhow::{Context, Result};
use clap::Parser;
use colored::*;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct RunArgs {
    /// The project context to run the script in.
    context: String,
    /// The name of the script to run.
    script: String,
    /// Parameters to pass to the script.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

///
/// Main entry point for the 'run' command.
/// Orchestrates the entire script execution process based on the "Task" model.
///
pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    // 1. Initial argument parsing and configuration resolution.
    let run_args = RunArgs::try_parse_from(&args)?;
    let index = index_manager::load_and_ensure_global_project()?;
    let mut config = commons::resolve_config_from_context_or_session(
        Some(run_args.context.clone()),
        &index,
        cancellation_token,
    )?;

    println!(
        "\n▶️  Running script '{}' for project '{}'...",
        run_args.script.cyan(),
        config.qualified_name.yellow()
    );

    // 2. Resolve the script into a `Task` object using the new expander logic.
    let task = config_resolver::resolve_task(&mut config, &run_args.script, ValueKind::Script)?;

    if task.commands.is_empty() {
        println!("{}", "Script is empty. Nothing to execute.".yellow());
        return Ok(());
    }

    // 3. Collect all parameter definitions from every command in the task.
    let all_definitions: Vec<_> = task
        .commands
        .iter()
        .flat_map(|cmd| &cmd.template)
        .filter_map(|component| match component {
            TemplateComponent::Parameter(def) => Some(def.clone()),
            _ => None,
        })
        .collect();

    let has_generic_params = task
        .commands
        .iter()
        .flat_map(|cmd| &cmd.template)
        .any(|c| matches!(c, TemplateComponent::GenericParams));

    // 4. Create a single `ArgResolver` for the entire task.
    let resolver = ArgResolver::new(&all_definitions, &run_args.params, has_generic_params)?;

    // 5. Execute the task using the shared task executor.
    task_executor::execute_task(&task, &config, &resolver, cancellation_token)?;

    // 6. Persist any changes to the cache (from lazy expansions).
    // This is now a no-op since the cache is in-memory only, but we keep it for potential future use.
    config_resolver::save_config_cache(&config, &index)
        .with_context(|| "Failed to save updated configuration cache.")?;

    println!(
        "\n✅ {} Script '{}' completed successfully.",
        "Success:".green().bold(),
        run_args.script.cyan()
    );
    Ok(())
}
