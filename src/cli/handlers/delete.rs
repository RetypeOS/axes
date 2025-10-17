use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::{Confirm, Input, theme::ColorfulTheme};
use std::fs;

use crate::{
    cli::handlers::commons,
    core::{context_resolver, index_manager},
    models::GlobalIndex,
};

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Deletes a project from the index and removes its '.axes' directory."
)]
struct DeleteArgs {
    /// Deletes the project and ALL its descendants recursively. This is highly destructive.
    #[arg(long)]
    recursive: bool,

    /// Instead of deleting direct children, reparents them to a new project.
    #[arg(long, conflicts_with = "recursive")]
    reparent_to: Option<String>,
}

pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. Parse arguments and resolve target project.
    let delete_args = DeleteArgs::try_parse_from(&args)?;
    let context_str =
        context.ok_or_else(|| anyhow!(t!("error.context_required"), command = "delete"))?;
    let config = commons::resolve_config_for_context(Some(context_str), index)?;

    if config.uuid == index_manager::GLOBAL_PROJECT_UUID {
        return Err(anyhow!(t!("delete.error.cannot_delete_global")));
    }

    // 2. [REFACTORED] Prepare the operational plan using the unified function.
    let plan = commons::prepare_operation_plan(
        index,
        &config,
        delete_args.recursive,
        delete_args.reparent_to.clone(),
        true, // is_destructive = true
    )?;

    // 3. Present the destructive plan to the user for confirmation.
    println!("\n{}", t!("delete.warning.destructive_header").red().bold());
    for line in &plan.summary_lines {
        println!("  - {}", line);
    }

    if !plan.paths_to_purge.is_empty() {
        println!("\n{}", t!("delete.info.files_to_be_deleted").yellow());
        for path in &plan.paths_to_purge {
            // We only delete the .axes directory for safety, not the whole project.
            println!("    • {}", path.join(crate::constants::AXES_DIR).display());
        }
    }

    // 4. Get confirmation.
    if !confirm_delete_operation(&config, delete_args.recursive)? {
        return Ok(());
    }

    // 5. EXECUTE PLAN - DESTRUCTIVE PART FIRST (FILE SYSTEM)
    log::info!(
        "Executing deletion plan for project '{}' ({})",
        config.qualified_name,
        config.uuid
    );
    let mut purged_count = 0;
    for project_root in &plan.paths_to_purge {
        let axes_dir = project_root.join(crate::constants::AXES_DIR);
        if axes_dir.exists() {
            log::debug!("Purging directory: {}", axes_dir.display());
            fs::remove_dir_all(&axes_dir)
                .with_context(|| format!("Failed to delete directory: {}", axes_dir.display()))?;
            purged_count += 1;
        } else {
            log::trace!("Skipping non-existent directory: {}", axes_dir.display());
        }
    }

    // 6. EXECUTE PLAN - INDEX MUTATION (IN-MEMORY)
    if !delete_args.recursive {
        let new_parent_uuid = match delete_args.reparent_to {
            Some(ctx) => context_resolver::resolve_context(&ctx, index)?.0,
            None => index_manager::GLOBAL_PROJECT_UUID,
        };
        // This function now returns warnings which we will display at the end.
        let reparent_op_warnings =
            index_manager::reparent_children(index, config.uuid, new_parent_uuid)?;

        // Combine warnings from planning and execution.
        let all_warnings = [&plan.reparent_warnings[..], &reparent_op_warnings[..]].concat();

        let removed_count = index_manager::remove_from_index(index, &plan.uuids_to_remove);

        // 7. Final feedback for non-recursive delete.
        println!(
            "\n{} {}",
            t!("common.success"),
            format_args!(
                t!("delete.success.header"),
                purged = purged_count,
                unregistered = removed_count
            )
        );
        for warning in all_warnings {
            println!("  - {}", warning.yellow());
        }
    } else {
        // Feedback for recursive delete
        let removed_count = index_manager::remove_from_index(index, &plan.uuids_to_remove);
        println!(
            "\n{} {}",
            t!("common.success"),
            format_args!(
                t!("delete.success.header_recursive"),
                purged = purged_count,
                unregistered = removed_count
            )
        );
    }

    Ok(())
}

/// Encapsulates the logic for user confirmation.
fn confirm_delete_operation(
    config: &crate::models::ResolvedConfig,
    is_recursive: bool,
) -> Result<bool> {
    if is_recursive {
        // Extra safety for recursive delete: user must type the project's simple name.
        let project_name = config.qualified_name.split('/').next_back().unwrap_or("");
        if project_name.is_empty() {
            // This is a safeguard against a panic if qualified_name is weird.
            return Err(anyhow!(
                "Could not determine project name for recursive delete confirmation."
            ));
        }
        let prompt = format!(t!("delete.prompt.recursive_confirm"), name = project_name);

        let confirmation: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .interact_text()?;

        if confirmation.trim() != project_name {
            println!("\n{}", t!("common.info.operation_cancelled"));
            return Ok(false);
        }
    } else {
        // Standard confirmation for non-recursive delete.
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(t!("delete.prompt.are_you_sure"))
            .default(false)
            .interact()?
        {
            println!("\n{}", t!("common.info.operation_cancelled"));
            return Ok(false);
        }
    }
    Ok(true)
}
