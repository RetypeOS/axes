// src/cli/handlers/delete.rs

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::{fs, path::PathBuf};

use crate::{
    
    cli::handlers::commons,
    constants::AXES_DIR,
    core::{context_resolver, index_manager},
};

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct DeleteArgs {
    /// The project context to delete.
    context: String,

    /// Deletes the project and all its descendants.
    #[arg(long)]
    recursive: bool,

    /// Reparents direct children to a new project instead of deleting them.
    #[arg(long)]
    reparent_to: Option<String>,
}

pub fn handle(args: Vec<String>) -> Result<()> {
    // Parse args.
    let delete_args = DeleteArgs::try_parse_from(&args)?;
    let mut index = index_manager::load_and_ensure_global_project()?;

    // Solve config.
    let config = commons::resolve_config_from_context_or_session(
        Some(delete_args.context),
        &index,
        
    )?;

    if config.uuid == index_manager::GLOBAL_PROJECT_UUID {
        return Err(anyhow!(t!("delete.error.cannot_delete_global")));
    }

    //println!("\n{}", t!("delete.warning.destructive_header").red().bold());

    // 1. Prepare the plan.
    let plan = commons::prepare_unregister_plan(
        &index,
        &config,
        delete_args.recursive,
        delete_args.reparent_to.clone(),
        
    )?;

    let paths_to_purge: Vec<PathBuf> = plan
        .uuids_to_remove
        .iter()
        .filter_map(|uuid| index.projects.get(uuid).map(|e| e.path.join(AXES_DIR)))
        .collect();

    // 2. Present the destructive plan.
    println!("\n{}", t!("delete.warning.destructive_header").red().bold());
    for line in &plan.summary_lines {
        println!("  - {}", line);
    }
    println!("{}", t!("delete.info.files_to_be_deleted").yellow());
    for path in &paths_to_purge {
        println!("    • {}", path.display());
    }

    // 3. Get MORE confirmation.
    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(t!("delete.prompt.are_you_sure"))
        .default(false)
        .interact()?
    {
        println!("\n{}", t!("common.info.operation_cancelled"));
        return Ok(());
    }

    // 4. EXECUTE PLAN - DESTRUCTIVE PART FIRST
    let mut purged_count = 0;
    for path in paths_to_purge {
        if path.exists() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("Failed to delete: {}", path.display()))?;
            purged_count += 1;
        }
    }

    // 5. EXECUTE PLAN - INDEX PART
    if !delete_args.recursive {
        let new_parent_uuid = match delete_args.reparent_to {
            Some(ctx) => context_resolver::resolve_context(&ctx, &index)?.0,
            None => index_manager::GLOBAL_PROJECT_UUID,
        };
        index_manager::reparent_children(&mut index, config.uuid, new_parent_uuid)?;
    }

    let removed_count = index_manager::remove_from_index(&mut index, &plan.uuids_to_remove);
    index_manager::save_global_index(&index)?;

    // 6. Final feedback.
    println!(
        "\n{} {}",
        t!("common.success"),
        format_args!(
            t!("delete.success.header"),
            purged = purged_count,
            unregistered = removed_count
        )
    );
    for warning in plan.reparent_warnings {
        println!("  - {}", warning.yellow());
    }
    Ok(())
}
