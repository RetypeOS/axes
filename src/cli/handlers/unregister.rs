use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::{Confirm, theme::ColorfulTheme};

use crate::{
    cli::handlers::commons,
    core::{context_resolver, index_manager},
    models::{GlobalIndex, ResolvedConfig},
    state::AppStateGuard,
};

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Removes a project and its descendants from the axes index without deleting files."
)]
struct UnregisterArgs {
    /// The context of the project to unregister.
    context: Option<String>,

    /// Unregisters the project and ALL its descendants recursively.
    #[arg(long)]
    recursive: bool,

    /// Instead of unregistering direct children, reparents them to a new project.
    #[arg(long, conflicts_with = "recursive")]
    reparent_to: Option<String>,
}

// --- Main Handler ---

pub fn handle(
    context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard,
) -> Result<()> {
    // 1. Parse & Resolve
    let unregister_args = UnregisterArgs::try_parse_from(&args)?;
    let final_context = unregister_args
        .context
        .or(context)
        .ok_or_else(|| anyhow!(t!("error.context_required"), command = "unregister"))?;
    let config = commons::resolve_config_for_context(Some(final_context), state_guard)?;

    if config.uuid == index_manager::GLOBAL_PROJECT_UUID {
        return Err(anyhow!(t!("unregister.error.cannot_unregister_global")));
    }

    // 2. Plan
    let plan = commons::prepare_operation_plan(
        state_guard,
        &config,
        unregister_args.recursive,
        unregister_args.reparent_to.clone(),
        false, // is_destructive
    )?;

    // 3. Confirm
    if !confirm_unregister_operation(state_guard.index(), &plan)? {
        return Ok(());
    }

    // 4. Execute
    execute_unregister_plan(
        state_guard,
        &config,
        &plan,
        unregister_args.recursive,
        &unregister_args.reparent_to,
    )?;

    Ok(())
}

/// Displays the plan and asks for user confirmation.
fn confirm_unregister_operation(
    index: &GlobalIndex,
    plan: &commons::OperationPlan,
) -> Result<bool> {
    println!("\n{}", t!("unregister.info.header").yellow().bold());
    for line in &plan.summary_lines {
        println!("  - {}", line);
    }

    println!("\n{}", t!("unregister.info.projects_to_remove").dimmed());
    for uuid in &plan.uuids_to_remove {
        if let Some(entry) = index.projects.get(uuid) {
            let qualified_name = index_manager::build_qualified_name(*uuid, index)
                .unwrap_or_else(|| entry.name.clone());
            println!("    • {} ({})", qualified_name.cyan(), entry.path.display());
        }
    }

    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(t!("common.prompt.continue"))
        .default(false)
        .interact()?
    {
        println!("\n{}", t!("common.info.operation_cancelled"));
        return Ok(false);
    }
    Ok(true)
}

/// Encapsulates the execution logic after confirmation.
fn execute_unregister_plan(
    state_guard: &mut AppStateGuard,
    config: &ResolvedConfig,
    plan: &commons::OperationPlan,
    is_recursive: bool,
    reparent_to: &Option<String>,
) -> Result<()> {
    log::info!(
        "Executing unregister plan for project '{}' ({})",
        config.qualified_name,
        config.uuid
    );

    let mut all_warnings = plan.reparent_warnings.clone();

    if !is_recursive {
        let new_parent_uuid = reparent_to
            .as_ref()
            .map(|ctx| context_resolver::resolve_context(ctx, state_guard).map(|(uuid, _)| uuid))
            .transpose()?
            .unwrap_or(index_manager::GLOBAL_PROJECT_UUID);

        log::debug!(
            "Reparenting children of '{}' to '{}'",
            config.uuid,
            new_parent_uuid
        );
        let reparent_op_warnings = index_manager::reparent_children(
            state_guard.index_mut(),
            config.uuid,
            new_parent_uuid,
        )?;
        all_warnings.extend(reparent_op_warnings);
    }

    log::debug!(
        "Removing {} UUID(s) from the index.",
        plan.uuids_to_remove.len()
    );
    let removed_count =
        index_manager::remove_from_index(state_guard.index_mut(), &plan.uuids_to_remove);

    // Final feedback
    println!(
        "\n{} {}",
        t!("common.success").green().bold(),
        format_args!(t!("unregister.success.header"), count = removed_count)
    );
    for warning in all_warnings {
        println!("  - {}", warning.yellow());
    }
    Ok(())
}
