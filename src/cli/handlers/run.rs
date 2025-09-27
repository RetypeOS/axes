// EN: src/cli/handlers/run.rs

use crate::{
    CancellationToken,
    cli::handlers::commons,
    core::{config_resolver, index_manager, parameters::ArgResolver},
    models::TemplateComponent,
    system::executor,
};
use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;
use rayon::prelude::*;

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
    let config = commons::resolve_config_from_context_or_session(
        Some(run_args.context.clone()),
        &index,
        cancellation_token,
    )?;

    println!(
        "\n▶️  Running script '{}' for project '{}'...",
        run_args.script.cyan(),
        config.qualified_name.yellow()
    );

    // 2. Resolve the script into a `Task` object. This triggers lazy expansion if needed.
    // We clone the task to break the mutable borrow on `config`.
    let task = config_resolver::resolve_task(&config, &run_args.script)?.clone();

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
    // This validates all CLI parameters against all definitions at once.
    let resolver = ArgResolver::new(&all_definitions, &run_args.params, has_generic_params)?;

    // 5. Execute the task, handling sequential and parallel commands.
    let mut parallel_batch: Vec<String> = Vec::new();

    for command_exec in &task.commands {
        // Assemble the final command string for this specific command.
        let final_command_str = assemble_final_command(&command_exec.template, &resolver)?;
        let trimmed_command = final_command_str.trim();

        if trimmed_command.is_empty() {
            continue; // Skip empty commands.
        }

        if command_exec.run_in_parallel {
            parallel_batch.push(trimmed_command.to_string());
        } else {
            // A sequential command acts as a barrier. Execute the pending parallel batch first.
            if !parallel_batch.is_empty() {
                execute_parallel_batch(&parallel_batch, &config, cancellation_token)?;
                parallel_batch.clear();
            }
            // Then execute the sequential command.
            execute_single_command(
                trimmed_command,
                command_exec.ignore_errors,
                &config,
                cancellation_token,
            )?;
        }
    }

    // Execute any remaining parallel commands at the end of the task.
    if !parallel_batch.is_empty() {
        execute_parallel_batch(&parallel_batch, &config, cancellation_token)?;
    }

    // 6. Persist any changes to the cache (from lazy expansions).
    config_resolver::save_config_cache(&config, &index)
        .with_context(|| "Failed to save updated configuration cache.")?;

    println!(
        "\n✅ {} Script '{}' completed successfully.",
        "Success:".green().bold(),
        run_args.script.cyan()
    );
    Ok(())
}

///
/// Assembles the final command string for a single `CommandExecution` by replacing
/// parameter tokens with their resolved values from the `ArgResolver`.
///
fn assemble_final_command(
    template: &[TemplateComponent],
    resolver: &ArgResolver,
) -> Result<String> {
    let mut final_command = String::new();
    for component in template {
        match component {
            TemplateComponent::Literal(s) => final_command.push_str(s),
            TemplateComponent::Parameter(def) => {
                let value = resolver
                    .get_specific_value(&def.original_token)
                    .ok_or_else(|| {
                        anyhow!(
                            "Internal logic failure: resolved value not found for token '{}'",
                            def.original_token
                        )
                    })?;
                final_command.push_str(value);
            }
            TemplateComponent::GenericParams => {
                final_command.push_str(resolver.get_generic_value());
            }
        }
    }
    Ok(final_command)
}

///
/// Executes a single command line.
///
fn execute_single_command(
    command_str: &str,
    ignore_errors: bool,
    config: &crate::models::ResolvedConfig,
    cancellation_token: &CancellationToken,
) -> Result<()> {
    let command_to_run = if ignore_errors {
        // Prepend the '-' marker for the executor to handle.
        format!("-{}", command_str)
    } else {
        command_str.to_string()
    };

    println!("\n> {}", command_str.green());
    Ok(executor::execute_command(
        &command_to_run,
        &config.project_root,
        &config.env,
        cancellation_token,
    )?)
}

///
/// Executes a batch of command strings in parallel using Rayon.
///
fn execute_parallel_batch(
    batch: &[String],
    config: &crate::models::ResolvedConfig,
    cancellation_token: &CancellationToken,
) -> Result<()> {
    println!("\n⚡ Running {} scripts in parallel...", batch.len());

    let results: Result<Vec<()>> = batch
        .par_iter()
        .map(|command_str| {
            // Note: Parallel commands cannot use `ignore_errors` currently. This is a design choice.
            // We print the command inside the parallel task for better log clarity.
            println!("  > {}", command_str.cyan());
            executor::execute_command(
                command_str,
                &config.project_root,
                &config.env,
                cancellation_token,
            )
            .map_err(anyhow::Error::from)
        })
        .collect();

    results.with_context(|| "A command in the parallel batch failed.")?;
    println!("{}", "⚡ Parallel batch completed.".blue());
    Ok(())
}
