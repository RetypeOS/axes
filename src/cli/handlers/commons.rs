// This module contains shared functions used by multiple handlers.

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
};

use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};

use colored::Colorize;

pub fn resolve_config_for_context(
    context_str: Option<String>,
    index: &mut GlobalIndex,
) -> Result<ResolvedConfig> {
    let final_context_str = context_str.unwrap_or_else(|| ".".to_string());

    if final_context_str == "_" {
        // Ephemeral context: load from current directory without relying on the index for the project itself.
        let mut loader = crate::core::config_loader::ConfigLoader::new(index);
        let cwd = std::env::current_dir()?;
        return loader
            .resolve_ephemeral(&cwd)
            .with_context(|| "Failed to resolve ephemeral project in the current directory");
    }

    // Original logic for registered projects
    let (uuid, _qualified_name) = context_resolver::resolve_context(&final_context_str, index)?;
    let mut loader = crate::core::config_loader::ConfigLoader::new(index);
    loader.resolve(uuid)
}

/// [NEW UNIFIED STRUCT] Represents a calculated plan for an operation like delete or unregister.
#[derive(Debug, Default)]
pub struct OperationPlan {
    pub uuids_to_remove: Vec<Uuid>,
    pub paths_to_purge: Vec<PathBuf>, // Empty for unregister, populated for delete.
    pub reparent_warnings: Vec<String>,
    pub summary_lines: Vec<String>,
}

/// [NEW UNIFIED FUNCTION] Prepares a comprehensive plan for an operation.
/// This is a "dry run" that calculates effects without modifying the index.
pub fn prepare_operation_plan(
    index: &mut GlobalIndex,
    config: &ResolvedConfig,
    recursive: bool,
    reparent_to: Option<String>,
    is_destructive: bool, // `true` for delete, `false` for unregister
) -> Result<OperationPlan> {
    let mut plan = OperationPlan::default();

    let new_parent_uuid = reparent_to
        .as_ref()
        .map(|ctx| context_resolver::resolve_context(ctx, index))
        .transpose()?
        .map(|(uuid, _)| uuid);

    if recursive {
        if reparent_to.is_some() {
            return Err(anyhow!(t!("plan.error.recursive_and_reparent")));
        }
        plan.uuids_to_remove.push(config.uuid);
        plan.uuids_to_remove
            .extend(index_manager::get_all_descendants(index, config.uuid));

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
        let final_parent_entry = index.projects.get(&final_parent_uuid).unwrap(); // Safe

        plan.summary_lines.push(format!(
            t!("plan.summary.reparent_to"),
            name = final_parent_entry.name
        ));

        let (warnings, conflicts) =
            check_reparent_collisions(index, config.uuid, final_parent_uuid)?;
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
            .filter_map(|uuid| index.projects.get(uuid).map(|e| e.path.clone()))
            .collect();
    }

    Ok(plan)
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

/// This is a shared utility for handlers like `open`, `run`, and `start`.
/// It now traverses the platform-agnostic AST to find all possible parameter definitions.
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
