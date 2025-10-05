// src/cli/handlers/unregister.rs

use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::{Confirm, theme::ColorfulTheme};

use super::commons;
use crate::core::{context_resolver, index_manager};

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct UnregisterArgs {
    /// Unregisters the project and all its descendants.
    #[arg(long)]
    recursive: bool,

    /// Reparents direct children to a new project instead of unregistering them.
    #[arg(long)]
    reparent_to: Option<String>,
}

pub fn handle(context: Option<String>, args: Vec<String>) -> Result<()> {
    let unregister_args = UnregisterArgs::try_parse_from(&args)?;
    let context_str =
        context.ok_or_else(|| anyhow!(t!("error.context_required"), command = "delete"))?;
    let mut index = index_manager::load_and_ensure_global_project()?;
    let config = commons::resolve_config_and_update_index_if_needed(Some(context_str), &mut index)?;

    if config.uuid == index_manager::GLOBAL_PROJECT_UUID {
        return Err(anyhow!(t!("unregister.error.cannot_unregister_global")));
    }

    // 1. Prepare the operational plan. This is a dry run.
    let plan = commons::prepare_unregister_plan(
        &mut index,
        &config,
        unregister_args.recursive,
        unregister_args.reparent_to.clone(),
    )?;

    // 2. Present the plan to the user.
    println!("\n{}", t!("unregister.info.header").yellow().bold());
    for line in &plan.summary_lines {
        println!("  - {}", line);
    }
    for uuid in &plan.uuids_to_remove {
        if let Some(entry) = index.projects.get(uuid) {
            println!("    • {} ({})", entry.name.cyan(), entry.path.display());
        }
    }

    // 3. Get confirmation.
    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(t!("common.prompt.continue"))
        .default(false)
        .interact()?
    {
        println!("\n{}", t!("common.info.operation_cancelled"));
        return Ok(());
    }

    // 4. Execute the plan.
    if !unregister_args.recursive {
        let new_parent_uuid = match unregister_args.reparent_to {
            Some(ctx) => context_resolver::resolve_context(&ctx, &mut index)?.0,
            None => index_manager::GLOBAL_PROJECT_UUID,
        };
        // The real reparenting happens here, with automatic renames.
        index_manager::reparent_children(&mut index, config.uuid, new_parent_uuid)?;
    }

    let removed_count = index_manager::remove_from_index(&mut index, &plan.uuids_to_remove);
    index_manager::save_global_index(&index)?;

    // 5. Final feedback.
    println!(
        "\n{} {}",
        t!("common.success"),
        format_args!(t!("unregister.success.header"), count = removed_count)
    );
    for warning in plan.reparent_warnings {
        println!("  - {}", warning.yellow());
    }
    Ok(())
}
