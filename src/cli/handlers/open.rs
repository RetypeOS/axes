// src/cli/handlers/open.rs

use crate::{
    CancellationToken,
    cli::handlers::commons,
    core::{
        config_resolver::{self, ValueKind},
        index_manager,
        parameters::ArgResolver,
        task_executor,
    },
    models::{CacheableValue, ParameterDef, TemplateComponent},
};
use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct OpenArgs {
    /// The project context to open.
    context: String,
    /// The application key from [options.open_with] to use. If omitted, uses 'default'.
    app_key: Option<String>,
    /// Parameters to pass to the open command.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    params: Vec<String>,
}

pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    // 1. Parse args and resolve config.
    let open_args = OpenArgs::try_parse_from(&args)?;
    let index = index_manager::load_and_ensure_global_project()?;
    let mut config = commons::resolve_config_from_context_or_session(
        Some(open_args.context),
        &index,
        cancellation_token,
    )?;

    // 2. Determine which `open_with` command to use, correctly handling the 'default' key.
    let app_key_from_user = open_args.app_key.as_deref().unwrap_or("default");

    let final_key = if app_key_from_user == "default" {
        config.options.open_with.get("default")
            .and_then(|val| match val {
                // The 'default' key MUST be a simple string (Raw Simple Command)
                CacheableValue::Raw(fc) if fc.command_lines.len() == 1 => Some(fc.command_lines[0].clone()),
                _ => None,
            })
            .ok_or_else(|| anyhow!("No 'default' application key is defined in [options.open_with]. It should be a simple string like 'default = \"editor\"'."))?
    } else {
        app_key_from_user.to_string()
    };

    let task_key = format!("options::open_with::{}", final_key);

    // 3. Resolve the command into a Task.
    let task = config_resolver::resolve_task(&mut config, &task_key, ValueKind::Script)?;

    // 4. Collect definitions and resolve arguments for the task.
    let definitions: Vec<ParameterDef> = task
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

    let resolver = ArgResolver::new(&definitions, &open_args.params, has_generic_params)?;

    // 5. Execute the task.
    println!("\n🚀 Opening with '{}'...", final_key.cyan());
    task_executor::execute_task(&task, &config, &resolver, cancellation_token)?;

    Ok(())
}
