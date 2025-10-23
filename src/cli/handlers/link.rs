//! # Handler for the `link` command
//!
//! This module provides the logic for the `axes link` command, which changes the parent
//! of a registered project, effectively moving it to a new location within the project
//! hierarchy.
//!
//! ## Core Logic
//!
//! 1.  **Context Resolution**: It resolves two contexts: the project to be moved and the
//!     new parent project. This requires mutable access to the application state as it
//!     updates `last_used` metadata.
//! 2.  **Pre-flight Validation**: Before performing any mutation, the `validate_link_operation`
//!     function runs a series of critical safety checks:
//!     - Prevents linking the "global" project.
//!     - Prevents a project from being linked to itself.
//!     - Checks if the link is a no-op (i.e., the project is already a child of the
//!       target parent).
//!     The underlying `index_manager::link_project` performs further checks for circular
//!     dependencies and name collisions.
//! 3.  **Index Mutation**: If validation passes, it calls `index_manager::link_project`, which
//!     atomically updates both the in-memory `GlobalIndex` and the on-disk `project_ref.bin`
//!     file of the moved project.
//! 4.  **User Feedback**: It provides a clear summary of the operation, showing the project's
//!     old and new fully qualified names.

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;
use uuid::Uuid;

use crate::{
    cli::handlers::commons,
    core::{context_resolver, index_manager},
    models::GlobalIndex,
    state::AppStateGuard,
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

/// The main handler for the `link` command.
///
/// It orchestrates the process of resolving the target project and the new parent,
/// validating the operation, performing the link, and providing feedback to the user.
///
/// # Arguments
/// * `context` - The context of the project to be moved, provided by the dispatcher.
/// * `args` - Command-specific arguments, containing the context of the new parent.
/// * `state_guard` - A mutable guard to the application state.
pub fn handle(
    context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard<'_>,
) -> Result<()> {
    // 1. Parse arguments and resolve the project to be moved.
    let link_args = LinkArgs::try_parse_from(&args)?;
    let context_str =
        context.ok_or_else(|| anyhow!(t!("error.context_required"), command = "link"))?;

    let config = commons::resolve_config_for_context(Some(context_str), state_guard)?;
    let project_to_move_uuid = config.uuid;
    let old_qualified_name = config.qualified_name.clone();

    // 2. Resolve the new parent project.
    let (new_parent_uuid, new_parent_qualified_name) =
        context_resolver::resolve_context(&link_args.new_parent, state_guard).with_context(
            || {
                anyhow!(
                    t!("link.error.cannot_resolve_parent"),
                    parent = link_args.new_parent
                )
            },
        )?;

    // 3. Perform critical pre-flight safety checks.
    if validate_link_operation(
        state_guard.index(),
        project_to_move_uuid,
        new_parent_uuid,
        &old_qualified_name,
        &new_parent_qualified_name,
    )?
    .is_none()
    {
        // A `None` result from validation indicates a no-op (already a child), so we exit.
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

    // 4. Perform the link operation. The index manager now handles the `project_ref.bin` update internally.
    index_manager::link_project(
        state_guard.index_mut(),
        project_to_move_uuid,
        new_parent_uuid,
    )
    .with_context(|| anyhow!(t!("link.error.link_failed"), name = old_qualified_name))?;

    // 5. Provide clear, detailed feedback to the user.
    let new_qualified_name =
        index_manager::build_qualified_name(project_to_move_uuid, state_guard.index())
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

/// Centralizes all pre-flight safety checks for the link operation.
///
/// This function performs initial, high-level validation before the more intensive
/// checks (like cycle detection) in `index_manager::link_project`.
///
/// # Arguments
/// * `index` - An immutable reference to the `GlobalIndex`.
/// * `project_to_move_uuid` - The UUID of the project being moved.
/// * `new_parent_uuid` - The UUID of the target parent.
/// * `old_qualified_name` - The current qualified name of the project being moved.
/// * `new_parent_qualified_name` - The qualified name of the target parent.
///
/// # Returns
/// - `Ok(Some(()))` if the operation is valid and should proceed.
/// - `Ok(None)` if the operation is a no-op (already a child) and should be silently aborted.
/// - `Err` for critical validation failures (e.g., linking to self).
fn validate_link_operation(
    index: &GlobalIndex,
    project_to_move_uuid: Uuid,
    new_parent_uuid: Uuid,
    old_qualified_name: &str,
    new_parent_qualified_name: &str,
) -> Result<Option<()>> {
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
        return Ok(None); // Signal a no-op.
    }

    Ok(Some(())) // Signal success.
}
