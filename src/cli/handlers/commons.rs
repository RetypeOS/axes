//! # Common Handler Utilities
//!
//! This module provides a collection of shared functions and data structures that are used
//! across multiple command handlers. Centralizing this logic promotes code reuse (DRY) and
//! ensures consistent behavior for common operations like configuration resolution,
//! operational planning, and interactive user prompts.
//!
//! ## Key Components
//!
//! - **`resolve_config_for_context`**: The main entry point for handlers to obtain a fully
//!   resolved, lazy-loaded `ResolvedConfig` for a given project context.
//! - **`OperationPlan`**: A unified struct for "dry-running" destructive or state-changing
//!   operations like `delete` and `unregister`, allowing the plan to be presented to the
//!   user for confirmation before execution.
//! - **Interactive Helpers**: Functions like `choose_parent_interactive` provide consistent,
//!   multi-modal UI for common tasks like selecting a project from the index.
//! - **Validation**: Utilities such as `validate_project_name` enforce consistent naming
//!   rules throughout the application.
//! - **Parameter Parsing**: Helpers like `build_resolver_for_task` abstract the logic
//!   of preparing command-line arguments for script execution.

use anyhow::{Context, Result, anyhow};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use uuid::Uuid;

use crate::{
    core::{
        context_resolver,
        index_manager::{self},
        parameters::ArgResolver,
    },
    models::{
        CommandAction, GlobalIndex, IndexEntry, ParameterDef, ResolvedConfig, Task,
        TemplateComponent,
    },
    state::AppStateGuard,
};

use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};

use colored::Colorize;

/// Lazily loads and resolves the full, inherited configuration for a given context string.
///
/// This is the primary entry point for most handlers to get project configuration. It handles
/// special contexts like `.` (current directory) and `_` (ephemeral project), and delegates
/// to the `ConfigLoader` for the heavy lifting of parallel loading, caching, and merging.
///
/// # Arguments
/// * `context_str` - An optional string representing the project context (e.g., "my-app/api", ".").
///   Defaults to "." if `None`.
/// * `state_guard` - A mutable guard to the application state, required by the `ConfigLoader`
///   to update cache metadata if necessary.
pub fn resolve_config_for_context(
    context_str: Option<String>,
    state_guard: &mut AppStateGuard<'_>,
) -> Result<ResolvedConfig> {
    let final_context_str = context_str.unwrap_or_else(|| ".".to_string());

    if final_context_str == "_" {
        // Ephemeral context: load from current directory without relying on the index for the project itself.
        let mut loader = crate::core::config_loader::ConfigLoader::new(state_guard.index_mut());
        let cwd = std::env::current_dir()?;
        return loader
            .resolve_ephemeral(&cwd)
            .with_context(|| "Failed to resolve ephemeral project in the current directory");
    }

    // Original logic for registered projects
    let (uuid, _qualified_name) =
        context_resolver::resolve_context(&final_context_str, state_guard)?;
    let mut loader = crate::core::config_loader::ConfigLoader::new(state_guard.index_mut());
    loader.resolve(uuid)
}

/// Represents a calculated plan for a state-changing operation like `delete` or `unregister`.
///
/// This struct is the result of a "dry run" and contains all the information needed to
/// both inform the user of the impending changes and to execute them after confirmation.
#[derive(Debug, Default)]
pub struct OperationPlan {
    /// The list of project UUIDs that will be removed from the index.
    pub uuids_to_remove: Vec<Uuid>,
    /// For destructive operations (`delete`), the list of project root paths whose `.axes`
    /// directory will be purged. This is empty for non-destructive operations.
    pub paths_to_purge: Vec<PathBuf>,
    /// A list of informational warnings about actions that will be taken automatically,
    /// such as renaming a child project to avoid a name collision during reparenting.
    pub reparent_warnings: Vec<String>,
    /// A list of human-readable strings summarizing the operation, to be presented to the user.
    pub summary_lines: Vec<String>,
}

/// Prepares a comprehensive plan for a `delete` or `unregister` operation.
///
/// This function performs a "dry run," calculating all the effects of the operation
/// without modifying the application state. It handles recursive logic, reparenting,
/// and collision detection, producing a detailed `OperationPlan`.
///
/// # Arguments
/// * `state_guard` - A mutable guard to the application state, used for context resolution.
/// * `config` - The `ResolvedConfig` of the target project for the operation.
/// * `recursive` - If `true`, the plan will include all descendants of the target project.
/// * `reparent_to` - An optional context string for a new parent for the target's children.
/// * `is_destructive` - If `true`, the plan will include filesystem paths to purge (for `delete`).
pub fn prepare_operation_plan(
    state_guard: &mut AppStateGuard<'_>,
    config: &ResolvedConfig,
    recursive: bool,
    reparent_to: Option<String>,
    is_destructive: bool, // `true` for delete, `false` for unregister
) -> Result<OperationPlan> {
    let mut plan = OperationPlan::default();

    let new_parent_uuid = reparent_to
        .as_ref()
        .map(|ctx| context_resolver::resolve_context(ctx, state_guard))
        .transpose()?
        .map(|(uuid, _)| uuid);

    if recursive {
        if reparent_to.is_some() {
            return Err(anyhow!(t!("plan.error.recursive_and_reparent")));
        }
        plan.uuids_to_remove.push(config.uuid);
        plan.uuids_to_remove
            .extend(index_manager::get_all_descendants(
                state_guard.index(),
                config.uuid,
            ));

        let summary_line = if is_destructive {
            format!(
                t!("plan.summary.delete_recursive"),
                name = config.qualified_name
            )
        } else {
            format!(
                t!("plan.summary.unregister_recursive"),
                name = config.qualified_name
            )
        };
        plan.summary_lines.push(summary_line);
    } else {
        plan.uuids_to_remove.push(config.uuid);
        plan.summary_lines.push(format!(
            t!("plan.summary.unregister_single"),
            name = config.qualified_name
        ));

        let final_parent_uuid = new_parent_uuid.unwrap_or(index_manager::GLOBAL_PROJECT_UUID);
        let final_parent_entry = state_guard.index().projects.get(&final_parent_uuid).expect(
            "Parent UUID is guaranteed to exist as it's either resolved or the global UUID",
        );

        plan.summary_lines.push(format!(
            t!("plan.summary.reparent_to"),
            name = final_parent_entry.name
        ));

        let (warnings, conflicts) =
            check_reparent_collisions(state_guard.index(), config.uuid, final_parent_uuid);
        if !conflicts.is_empty() {
            return Err(anyhow!(
                t!("plan.error.reparent_collision"),
                conflicts = conflicts.join("', '")
            ));
        }
        plan.reparent_warnings = warnings;
    }

    // This is the only part that differs between delete and unregister.
    if is_destructive {
        plan.paths_to_purge = plan
            .uuids_to_remove
            .iter()
            .filter_map(|uuid| {
                state_guard
                    .index()
                    .projects
                    .get(uuid)
                    .map(|e| e.path.clone())
            })
            .collect();
    }

    Ok(plan)
}

/// Checks for potential name collisions when reparenting children.
/// Returns a tuple of (`warnings_for_automatic_renames`, `hard_conflicts`).
fn check_reparent_collisions(
    index: &GlobalIndex,
    old_parent_uuid: Uuid,
    new_parent_uuid: Uuid,
) -> (Vec<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut conflicts = Vec::new();

    let old_parent_name = &index
        .projects
        .get(&old_parent_uuid)
        .expect("Old parent UUID must exist in the index for this function to be called")
        .name;
    let children_to_move: Vec<_> = index
        .projects
        .values()
        .filter(|e| e.parent == Some(old_parent_uuid))
        .collect();

    if children_to_move.is_empty() {
        return (warnings, conflicts);
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

    (warnings, conflicts)
}

/// Presents an interactive, multi-modal UI for selecting a parent project.
///
/// This function offers the user multiple ways to choose a project:
/// 1.  Entering a context path directly.
/// 2.  Visually browsing the project tree.
/// 3.  Choosing the "global" project as the parent.
///     It ensures a consistent and user-friendly experience for any command that needs to
///     ask the user for a project context (e.g., `init`, `register`).
///
/// # Arguments
/// * `state_guard` - A mutable guard to the application state, needed for context resolution
///   and browsing.
pub fn choose_parent_interactive(state_guard: &mut AppStateGuard<'_>) -> Result<Uuid> {
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
                if let Some(uuid) = select_parent_by_context(state_guard)? {
                    return Ok(uuid);
                }
                // If it returns None, the user cancelled, so we loop again.
            }
            1 => {
                // Browse projects visually
                return select_parent_by_browsing(state_guard.index());
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
fn select_parent_by_context(state_guard: &mut AppStateGuard<'_>) -> Result<Option<Uuid>> {
    loop {
        let input: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter context path (leave empty to go back)")
            .interact_text()?;

        if input.is_empty() {
            return Ok(None); // User wants to go back to the main menu
        }

        match context_resolver::resolve_context(&input, state_guard) {
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
    // Pre-build a map of parent -> children to avoid iterating the whole projects map in every loop.
    let mut children_map: HashMap<Option<Uuid>, Vec<(Uuid, &IndexEntry)>> = HashMap::new();
    for (uuid, entry) in &index.projects {
        children_map
            .entry(entry.parent)
            .or_default()
            .push((*uuid, entry));
    }
    // Sort all child lists once for a consistent UI.
    for children in children_map.values_mut() {
        children.sort_by_key(|(_, entry)| &entry.name);
    }

    let mut current_uuid = index_manager::GLOBAL_PROJECT_UUID;

    loop {
        let current_entry = index.projects.get(&current_uuid).ok_or_else(|| {
            anyhow!(
                "Browser state invalid: project with UUID {} not found.",
                current_uuid
            )
        })?;

        let children = children_map
            .get(&Some(current_uuid))
            .map_or(&[][..], |v| &v[..]);

        let mut items = Vec::new();
        items.push(format!("✅ [ Select '{}' as parent ]", current_entry.name));
        if current_uuid != index_manager::GLOBAL_PROJECT_UUID {
            items.push("⬆️  [ Go up to parent project ]".to_string());
        }

        enum BrowserAction {
            Select,
            GoUp,
            GoToChild(Uuid),
        }
        let mut action_map = vec![BrowserAction::Select];
        if current_uuid != index_manager::GLOBAL_PROJECT_UUID {
            action_map.push(BrowserAction::GoUp);
        }

        for (child_uuid, child_entry) in children {
            items.push(format!("  └─ {}", child_entry.name));
            action_map.push(BrowserAction::GoToChild(*child_uuid));
        }

        let prompt = format!("Browsing children of '{}'", current_entry.name);
        let selection_idx = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(&prompt)
            .items(&items)
            .default(0)
            .interact()?;

        match action_map.get(selection_idx) {
            Some(BrowserAction::Select) => return Ok(current_uuid),
            Some(BrowserAction::GoUp) => {
                current_uuid = current_entry
                    .parent
                    .unwrap_or(index_manager::GLOBAL_PROJECT_UUID);
            }
            Some(BrowserAction::GoToChild(child_uuid)) => {
                current_uuid = *child_uuid;
            }
            None => { /* Should not happen */ }
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
    let first_char = name
        .chars()
        .next()
        .expect("String is guaranteed not to be empty here");
    let last_char = name
        .chars()
        .last()
        .expect("String is guaranteed not to be empty here");

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

/// Collects all unique `ParameterDef`s from every platform-specific command in a `Task`.
///
/// This utility traverses the entire AST of a task to build a complete contract of all
/// possible parameters it accepts, which is essential for the `ArgResolver`.
///
/// # Arguments
/// * `task` - An `Arc<Task>` from which to collect parameter definitions.
pub fn collect_parameter_defs_from_task(task: &Arc<Task>) -> Vec<ParameterDef> {
    task.commands
        .iter()
        // Iterate over each PlatformExecution block
        .flat_map(|plat_exec| {
            // Create an array of the Option<T>s themselves, then call iter().flatten().
            // This turns an iterator of Option<T> into an iterator of T.
            [
                plat_exec.default.as_ref(),
                plat_exec.windows.as_ref(),
                plat_exec.linux.as_ref(),
                plat_exec.macos.as_ref(),
            ]
            .into_iter()
            .flatten()
        })
        .flat_map(|cmd_exec| match &cmd_exec.action {
            CommandAction::Execute(t) | CommandAction::Print(t) => t.clone(),
        })
        .filter_map(|component| match component {
            TemplateComponent::Parameter(def) => Some(def),
            _ => None,
        })
        // Use a fold with a HashSet to collect only unique definitions.
        .fold(
            (Vec::new(), std::collections::HashSet::new()),
            |(mut acc_vec, mut acc_set), def| {
                if acc_set.insert(def.clone()) {
                    acc_vec.push(def);
                }
                (acc_vec, acc_set)
            },
        )
        .0 // Return only the vector of unique definitions.
}

/// Builds an argument resolver for a given task and CLI parameters.
/// This encapsulates the logic for collecting definitions and checking for the generic `<params>` token.
pub fn build_resolver_for_task<'a>(
    task: &Arc<Task>,
    params: &'a [String],
) -> Result<ArgResolver<'a>> {
    let all_definitions = collect_parameter_defs_from_task(task);

    let has_generic_params = task.commands.iter().any(|plat_exec| {
        [
            plat_exec.default.as_ref(),
            plat_exec.windows.as_ref(),
            plat_exec.linux.as_ref(),
            plat_exec.macos.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|cmd_exec| {
            let template = match &cmd_exec.action {
                CommandAction::Execute(t) | CommandAction::Print(t) => t,
            };
            template
                .iter()
                .any(|c| matches!(c, TemplateComponent::GenericParams { .. }))
        })
    });

    ArgResolver::new(&all_definitions, params, has_generic_params)
}
