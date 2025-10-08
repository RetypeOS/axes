// src/cli/handlers/unregister.rs

use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::{Confirm, theme::ColorfulTheme};

use crate::{
    cli::handlers::commons,
    core::{context_resolver, index_manager},
    models::GlobalIndex,
};

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Removes a project and its descendants from the axes index without deleting files."
)]
struct UnregisterArgs {
    /// Unregisters the project and ALL its descendants recursively.
    #[arg(long)]
    recursive: bool,

    /// Instead of unregistering direct children, reparents them to a new project.
    #[arg(long, conflicts_with = "recursive")]
    reparent_to: Option<String>,
}

// --- Main Handler ---

pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. Parse arguments and resolve the target project lazily.
    let unregister_args = UnregisterArgs::try_parse_from(&args)?;
    let context_str =
        context.ok_or_else(|| anyhow!(t!("error.context_required"), command = "unregister"))?;

    // Use the architecturally correct lazy resolver.
    let config = commons::resolve_config_for_context(Some(context_str), index)?;

    // Safety check: prevent unregistering the global project.
    if config.uuid == index_manager::GLOBAL_PROJECT_UUID {
        return Err(anyhow!(t!("unregister.error.cannot_unregister_global")));
    }

    // 2. Prepare the operational plan. This is a "dry run".
    // We use the same planning logic as `delete`, but will ignore the file paths.
    let plan = commons::prepare_deletion_plan(
        index,
        &config,
        unregister_args.recursive,
        unregister_args.reparent_to.clone(),
    )?;

    // 3. Present the plan to the user for confirmation.
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

    // 4. Get confirmation.
    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(t!("common.prompt.continue"))
        .default(false)
        .interact()?
    {
        println!("\n{}", t!("common.info.operation_cancelled"));
        return Ok(());
    }

    // 5. EXECUTE PLAN - INDEX MUTATION (IN-MEMORY)
    let mut all_warnings = plan.reparent_warnings;
    if !unregister_args.recursive {
        let new_parent_uuid = match unregister_args.reparent_to {
            Some(ctx) => context_resolver::resolve_context(&ctx, index)?.0,
            None => index_manager::GLOBAL_PROJECT_UUID,
        };
        // The real reparenting happens here, with automatic renames.
        let reparent_op_warnings =
            index_manager::reparent_children(index, config.uuid, new_parent_uuid)?;
        all_warnings.extend(reparent_op_warnings);
    }

    let removed_count = index_manager::remove_from_index(index, &plan.uuids_to_remove);

    // CRITICAL: No `save_global_index` call. `main` handles this.

    // 6. Final feedback.
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
