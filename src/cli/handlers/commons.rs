// src/cli/handlers/commons.rs

// This module contains shared functions used by multiple handlers.

use anyhow::{Result, anyhow};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{
    core::{
        config_resolver::{self, ConfigResolutionResult}, context_resolver,
        index_manager::{self},
    },
    models::{GlobalIndex, IndexEntry, ResolvedConfig},
};

use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};

use colored::Colorize;

/// Represents a calculated plan for an unregister or delete operation.
/// It contains all the necessary information to present to the user and execute.
#[derive(Debug, Default)]
pub struct UnregisterPlan {
    pub uuids_to_remove: Vec<Uuid>,
    pub reparent_warnings: Vec<String>,
    pub summary_lines: Vec<String>,
}

/// Prepares a plan for unregistering projects. This function is a "dry run"
/// and does not modify the index; it only calculates the effects.
pub fn prepare_unregister_plan(
    index: &mut GlobalIndex,
    config: &ResolvedConfig,
    recursive: bool,
    reparent_to: Option<String>,
) -> Result<UnregisterPlan> {
    let mut uuids_to_remove = vec![config.uuid];
    let mut reparent_warnings = Vec::new();
    let mut summary_lines = Vec::new();

    let new_parent_uuid = if let Some(ctx) = &reparent_to {
        let (uuid, _) = context_resolver::resolve_context(ctx, index)?;
        Some(uuid)
    } else {
        None
    };

    if recursive {
        if reparent_to.is_some() {
            return Err(anyhow!(t!("plan.error.recursive_and_reparent")));
        }
        uuids_to_remove.extend(index_manager::get_all_descendants(index, config.uuid));
        summary_lines.push(t!("plan.summary.unregister_recursive").to_string());
    } else {
        summary_lines.push(
            format!(
                t!("plan.summary.unregister_single"),
                name = config.qualified_name
            )
            .to_string(),
        );

        let final_parent_uuid = new_parent_uuid.unwrap_or(index_manager::GLOBAL_PROJECT_UUID);
        let final_parent_entry = index.projects.get(&final_parent_uuid).unwrap();

        summary_lines.push(
            format!(
                t!("plan.summary.reparent_to"),
                name = final_parent_entry.name
            )
            .to_string(),
        );

        let (warnings, conflicts) =
            check_reparent_collisions(index, config.uuid, final_parent_uuid)?;
        if !conflicts.is_empty() {
            let conflict_str = conflicts.join("', '");
            return Err(anyhow!(
                t!("plan.error.reparent_collision"),
                conflicts = conflict_str
            ));
        }
        reparent_warnings = warnings;
    }

    Ok(UnregisterPlan {
        uuids_to_remove,
        reparent_warnings,
        summary_lines,
    })
}

/// Checks for potential name collisions when reparenting children.
/// Returns a tuple of (warnings_for_automatic_renames, hard_conflicts).
fn check_reparent_collisions(
    index: &GlobalIndex,
    old_parent_uuid: Uuid,
    new_parent_uuid: Uuid,
) -> Result<(Vec<String>, Vec<String>)> {
    let mut warnings = Vec::new();
    let mut conflicts = Vec::new();

    let old_parent_name = &index.projects.get(&old_parent_uuid).unwrap().name;
    let children_to_move: Vec<_> = index
        .projects
        .values()
        .filter(|e| e.parent == Some(old_parent_uuid))
        .collect();

    if children_to_move.is_empty() {
        return Ok((warnings, conflicts));
    }

    let new_sibling_names: HashSet<_> = index
        .projects
        .values()
        .filter(|e| e.parent == Some(new_parent_uuid))
        .map(|e| e.name.clone())
        .collect();

    for child in children_to_move {
        if new_sibling_names.contains(&child.name) {
            // Initial collision detected
            let suggested_name = format!("{}_{}", old_parent_name, child.name);
            if new_sibling_names.contains(&suggested_name) {
                // Automatic rename also conflicts. This is a hard conflict.
                conflicts.push(format!(
                    "'{}' (also conflicts as '{}')",
                    child.name, suggested_name
                ));
            } else {
                // Automatic rename is possible. This is a warning.
                warnings.push(format!(
                    "Child '{}' will be renamed to '{}' to avoid collision.",
                    child.name, suggested_name
                ));
            }
        }
    }

    Ok((warnings, conflicts))
}

/// Helper function to resolve a project's configuration.
/// It normalizes the context input before passing it to the main context_resolver.
//pub fn resolve_config_from_context_or_session(
//    context_str: Option<String>,
//    index: &GlobalIndex,
//) -> Result<ResolvedConfig> {
//    // This function now acts as a clean bridge to the powerful context_resolver.
//
//    // If no context is provided (e.g., from `axes build`), we default to `.`
//    // which means "find project in current dir or parents". The session-awareness
//    // logic is now correctly handled inside `context_resolver`.
//    let final_context_str = context_str.unwrap_or_else(|| ".".to_string());
//
//    // Delegate the complex resolution logic to the expert module.
//    let (uuid, qualified_name) = context_resolver::resolve_context(&final_context_str, index)?;
//
//    // Once we have the canonical UUID, we can resolve its config.
//    config_resolver::resolve_config_for_uuid(uuid, qualified_name, index).with_context(|| {
//        format!(
//            "Failed to resolve configuration for context '{}'",
//            final_context_str
//        )
//    })
//}

/// The new main helper to resolve configuration for a project.
/// It handles the entire flow, including updating the global index if the
/// project's cache path has changed.
pub fn resolve_config_and_update_index_if_needed(
    context_str: Option<String>,
    index: &mut GlobalIndex,
) -> Result<ResolvedConfig> {
    // 1. Resolve context to a canonical UUID.
    // Note: The `resolve_context` call itself doesn't need a mutable index.
    let final_context_str = context_str.unwrap_or_else(|| ".".to_string());
    let (uuid, qualified_name) = context_resolver::resolve_context(&final_context_str, index)?;

    // 2. After successful resolution, we update the `last_used` caches.
    // This is the "write" part of the operation.
    context_resolver::update_last_used_caches(uuid, index)?;

    // 3. Call the config resolver to get config and potential new cache path.
    let ConfigResolutionResult { config, new_cache_path } =
        config_resolver::resolve_config_for_uuid(uuid, qualified_name, index)?;

    // 4. If the cache path changed, update the index entry.
    let index_dirty = if let Some(path) = new_cache_path {
        if let Some(entry) = index.projects.get_mut(&uuid) {
            entry.cache_path = Some(path);
            true
        } else {
            false
        }
    } else {
        false
    };
    
    // We set index_dirty also if last_used caches were updated.
    // For simplicity, we can just check if any last_used_child field is not None,
    // but a more robust check would compare before/after states. For now, we assume
    // any context resolution might dirty the index.
    //index_dirty = true; // Assume dirty for now to ensure saves.

    // 5. Save the index to disk if any part of it was modified.
    if index_dirty {
        index_manager::save_global_index(index)?;
    }

    Ok(config)
}

/// Interactive, multi-modal parent selector.
pub fn choose_parent_interactive(index: &mut GlobalIndex) -> Result<Uuid> {
    loop {
        let items = &[
            "Enter a context path (e.g., 'my-app/api', 'g!', '*')",
            "Browse projects visually",
            "Use 'global' as the parent (default)",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("How would you like to select the parent project?")
            .items(items)
            .default(2) // Default to 'global'
            .interact()?;

        match selection {
            0 => {
                // Enter a context path
                if let Some(uuid) = select_parent_by_context(index)? {
                    return Ok(uuid);
                }
                // If it returns None, the user cancelled, so we loop again.
            }
            1 => {
                // Browse projects visually
                return select_parent_by_browsing(index);
            }
            2 => {
                // Use 'global'
                return Ok(index_manager::GLOBAL_PROJECT_UUID);
            }
            _ => unreachable!(),
        }
    }
}

/// Handles the "Enter context" workflow. Returns `Ok(Some(Uuid))` on success,
/// `Ok(None)` if the user cancels, and `Err` on I/O failure.
fn select_parent_by_context(index: &mut GlobalIndex) -> Result<Option<Uuid>> {
    loop {
        let input: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter context path (leave empty to go back)")
            .interact_text()?;

        if input.is_empty() {
            return Ok(None); // User wants to go back to the main menu
        }

        match context_resolver::resolve_context(&input, index) {
            Ok((uuid, qualified_name)) => {
                let prompt = format!("Resolved to '{}'. Use this as the parent?", qualified_name);
                if Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(prompt)
                    .default(true)
                    .interact()?
                {
                    return Ok(Some(uuid));
                };

                // If user says no, the loop continues to ask for another context.
            }
            Err(e) => {
                // Inform the user of the error and let them try again.
                println!("Error: {}", e);
            }
        }
    }
}

/// Handles the visual browsing workflow.
fn select_parent_by_browsing(index: &GlobalIndex) -> Result<Uuid> {
    let mut current_uuid_opt = None; // Start at the root view

    loop {
        let (current_name, current_uuid, children) = match current_uuid_opt {
            Some(uuid) => {
                let entry = index.projects.get(&uuid).unwrap();
                let children_vec: Vec<&IndexEntry> = index
                    .projects
                    .values()
                    .filter(|e| e.parent == Some(uuid))
                    .collect();
                (entry.name.clone(), uuid, children_vec)
            }
            None => {
                let root_entry = index
                    .projects
                    .get(&index_manager::GLOBAL_PROJECT_UUID)
                    .expect("Fatal: Root project not found during browsing.");

                let children_vec: Vec<&IndexEntry> = index
                    .projects
                    .values()
                    .filter(|e| e.parent == Some(index_manager::GLOBAL_PROJECT_UUID))
                    .collect();
                (
                    root_entry.name.clone(),
                    index_manager::GLOBAL_PROJECT_UUID,
                    children_vec,
                )
            }
        };

        let mut items = Vec::new();
        items.push(format!("✅ [ Select '{}' as parent ]", current_name));

        // The option to go back is only available if we are not at the root view.
        if current_uuid_opt.is_some() {
            items.push("⬆️  [ Go up to parent project ]".to_string());
        }

        let mut child_map = HashMap::new();
        for child in children.iter() {
            // Store the mapping from the display name to the actual entry.
            let display_name = format!("  └─ {}", child.name);
            items.push(display_name.clone());
            child_map.insert(display_name, *child);
        }

        let prompt = format!("Browsing children of '{}'", current_name);
        let selection_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(&prompt)
            .items(&items)
            .default(0)
            .interact()?;

        let selection_str = &items[selection_idx];

        if selection_str.starts_with("✅") {
            return Ok(current_uuid);
        } else if selection_str.starts_with("⬆️") {
            let current_entry = index.projects.get(&current_uuid).unwrap();
            current_uuid_opt = if current_entry.parent == Some(index_manager::GLOBAL_PROJECT_UUID) {
                None // Go back to the root view
            } else {
                current_entry.parent
            };
        } else {
            // A child was selected
            if let Some(selected_child) = child_map.get(selection_str) {
                // Find the UUID of the selected child
                let child_uuid = index
                    .projects
                    .iter()
                    .find(|(_, entry)| entry.path == selected_child.path) // Path is a reliable unique identifier
                    .map(|(uuid, _)| *uuid);

                if let Some(uuid) = child_uuid {
                    current_uuid_opt = Some(uuid);
                }
            }
        }
    }
}

/// Validates a project name against axes' naming rules.
/// Returns a sanitized `String` on success.
/// Prints non-blocking warnings for stylistic issues.
/// Returns a blocking `Err` for critical issues.
pub fn validate_project_name(raw_name: &str) -> Result<String> {
    let name = raw_name.trim();

    // --- Strict, Blocking Errors ---
    if name.is_empty() {
        return Err(anyhow!(t!("validation.error.empty_name")));
    }
    if name.contains(char::is_whitespace) {
        return Err(anyhow!(t!("validation.error.contains_whitespace")));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(anyhow!(t!("validation.error.invalid_chars")));
    }
    let reserved_nav_names = ["..", "*", "**", "_"];
    if reserved_nav_names.contains(&name.to_lowercase().as_str()) {
        return Err(anyhow!(t!("validation.error.reserved_name"), name = name));
    }

    // --- Soft, Non-Blocking Warnings ---
    let first_char = name.chars().next().unwrap(); // Safe due to is_empty check
    let last_char = name.chars().last().unwrap();

    if !first_char.is_alphanumeric() {
        println!(
            "{}",
            format!(
                "Warning: The name '{}' starts with a non-alphanumeric character. This is allowed but may cause confusion.",
                name
            )
            .yellow()
        );
    }
    if !last_char.is_alphanumeric() && last_char != '_' {
        println!(
            "{}",
            format!(
                "Warning: The name '{}' ends with a special character. This is allowed but not recommended.",
                name
            )
            .yellow()
        );
    }

    Ok(name.to_string())
}
