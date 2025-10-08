// src/cli/handlers/link.rs (REBUILT FOR ARCHITECTURE, SAFETY, AND UX)

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;

use crate::{
    cli::handlers::commons,
    core::{context_resolver, index_manager},
    models::GlobalIndex,
};

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Moves a project to be a child of another project."
)]
struct LinkArgs {
    /// The context of the new parent project.
    new_parent: String,
}

// --- Main Handler ---

pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. Parse arguments and resolve the project to be moved.
    let link_args = LinkArgs::try_parse_from(&args)?;
    let context_str =
        context.ok_or_else(|| anyhow!(t!("error.context_required"), command = "link"))?;

    // Use the architecturally correct lazy resolver.
    let config = commons::resolve_config_for_context(Some(context_str), index)?;
    let project_to_move_uuid = config.uuid;
    let old_qualified_name = config.qualified_name.clone();

    // 2. Resolve the new parent project.
    let (new_parent_uuid, new_parent_qualified_name) =
        context_resolver::resolve_context(&link_args.new_parent, index).with_context(|| {
            anyhow!(
                t!("link.error.cannot_resolve_parent"),
                parent = link_args.new_parent
            )
        })?;

    // 3. Perform critical safety checks BEFORE attempting the operation.
    if project_to_move_uuid == index_manager::GLOBAL_PROJECT_UUID {
        return Err(anyhow!(t!("link.error.cannot_link_global")));
    }
    if project_to_move_uuid == new_parent_uuid {
        return Err(anyhow!(t!("link.error.link_to_self")));
    }

    let project_to_move_entry = index.projects.get(&project_to_move_uuid).unwrap(); // Safe
    if project_to_move_entry.parent == Some(new_parent_uuid) {
        println!(
            "{}",
            format!(
                t!("link.info.already_child"),
                name = old_qualified_name,
                parent = new_parent_qualified_name
            )
            .yellow()
        );
        return Ok(());
    }

    println!(
        "\n{}",
        format_args!(
            t!("link.info.attempting"),
            name = old_qualified_name.cyan(),
            new_parent = new_parent_qualified_name.cyan()
        )
    );

    // 4. Perform the link operation in the index manager (which contains cycle checks).
    index_manager::link_project(index, project_to_move_uuid, new_parent_uuid)
        .with_context(|| anyhow!(t!("link.error.link_failed"), name = old_qualified_name))?;

    // 5. Update the local project reference file (`project_ref.bin`) for consistency.
    let mut project_ref =
        index_manager::get_or_create_project_ref(&config.project_root, project_to_move_uuid, index)
            .with_context(|| t!("error.local_ref_failed"))?;

    project_ref.parent_uuid = Some(new_parent_uuid);
    if let Err(e) = index_manager::write_project_ref(&config.project_root, &project_ref) {
        // This is not a fatal error, but the user must be warned.
        eprintln!(
            "\n{}",
            anyhow!(t!("link.warning.local_ref_update_failed"), error = e)
                .to_string()
                .yellow()
        );
    }

    // CRITICAL: The main function will save the updated global index. No need to do it here.

    // 6. Provide clear, detailed feedback to the user, including the new qualified name.
    let new_qualified_name = index_manager::build_qualified_name(project_to_move_uuid, index)
        .unwrap_or_else(|| t!("common.label.unknown").to_string());

    println!("\n{}", t!("common.success").green().bold());
    println!("  {:<15} {}", "Project Moved:".blue(), old_qualified_name);
    println!(
        "  {:<15} {}",
        "New Parent:".blue(),
        new_parent_qualified_name
    );
    println!(
        "  {:<15} {}",
        "New Full Path:".blue(),
        new_qualified_name.cyan()
    );
    println!("\n  {}", t!("common.info.caches_will_regenerate").dimmed());

    Ok(())
}
