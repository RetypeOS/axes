//! # Context Resolver
//!
//! This module provides the `resolve_context` function, which is responsible for resolving a project
//! context string to its canonical UUID and fully qualified name. The resolution follows a strict,
//! multi-layered priority order to ensure predictable behavior both inside and outside of project
//! sessions.

use crate::{
    core::index_manager::{self, GLOBAL_PROJECT_UUID},
    models::{GlobalIndex, IndexEntry},
    state::AppStateGuard,
};
use dialoguer::{theme::ColorfulTheme, Error as DialoguerError, Select};
use std::{env, path::Path};
use thiserror::Error;
use uuid::Uuid;

/// Represents errors that can occur during the resolution of a context string (e.g., `my-app/backend`).
#[derive(Error, Debug)]
pub enum ContextError {
    // --- Wrapped Errors ---
    /// A filesystem error occurred during path resolution.
    #[error("Filesystem Error: {0}")]
    Io(#[from] std::io::Error),
    /// An error occurred while accessing the global index.
    #[error("Index Error: {0}")]
    Index(#[from] crate::core::index_manager::IndexError),
    /// An error occurred while decoding a binary file (e.g., `project_ref.bin`).
    #[error("Error decoding cache: {0}")]
    BincodeDecode(#[from] bincode::error::DecodeError),
    /// An error occurred while encoding a binary file.
    #[error("Error encoding cache: {0}")]
    BincodeEncode(#[from] bincode::error::EncodeError),
    /// An error occurred during an interactive prompt (e.g., user pressed Ctrl+C).
    #[error("User Interface Error: {0}")]
    Dialoguer(#[from] DialoguerError),

    // --- Semantic Errors ---
    /// An empty context string was provided where one was required.
    #[error("Empty context not provided.")]
    EmptyContext,
    /// The `**` token was used in a position other than the start of the context string.
    #[error("Context '**' can only be used at the beginning of the path.")]
    GlobalRecentNotAtStart,
    /// The `_` token was used in a position other than the start of the context string.
    #[error("Context '_' can only be used at the beginning of a path when outside a session.")]
    StrictLocalPathNotAtStart,
    /// A `..` token was used on a project that has no parent.
    #[error("Cannot go further up the hierarchy. Already at a root project.")]
    AlreadyAtRoot,
    /// The `**` token was used but no projects have been accessed recently.
    #[error("No projects have been used recently. Cannot resolve '**'.")]
    NoLastUsedProject,
    /// The `*` token was used on a parent that has no last-used child.
    #[error("Parent project '{parent_name}' has not used any children recently. Cannot resolve '*'.")]
    NoLastUsedChild {
        /// The name of the parent project.
        parent_name: String,
    },
    /// A project context was inferred from the current path (`.`), but no project was found.
    #[error("No axes project found in current directory or any parent directories.")]
    ProjectNotFoundFromPath,
    /// A project context was specified as the current directory (`_`), but no project is registered there.
    #[error("No axes project found in current directory.")]
    ProjectNotFoundInCwd,
    /// The first part of a context path did not match any known root project.
    #[error("Root project with name '{name}' not found.")]
    RootProjectNotFound {
        /// The name of the root project that was not found.
        name: String,
    },
    /// A part of a context path did not match any child of the preceding project.
    #[error("Child project '{child_name}' not found for parent '{parent_name}'.")]
    ChildProjectNotFound {
        /// The name of the child that was not found.
        child_name: String,
        /// The name of the parent that was being searched.
        parent_name: String,
    },
    /// An alias (e.g., `my-alias!`) was used that is not defined.
    #[error("Alias '{name}!' not found.")]
    AliasNotFound {
        /// The name of the alias that was not found.
        name: String,
    },
    /// The qualified name for a project could not be constructed, likely due to a broken parent link.
    #[error("Could not resolve project name for alias (possible broken parent link).")]
    AliasResolutionError,
    /// The user cancelled an interactive operation.
    #[error("Operation cancelled by user.")]
    Cancelled,
}

type ContextResult<T> = Result<T, ContextError>;

/// Resolves a project context string to its canonical UUID and fully qualified name.
/// The resolution follows a strict, multi-layered priority order to ensure
/// predictable behavior both inside and outside of project sessions.
///
/// # Arguments
///
/// * `context` - The context string to resolve.
/// * `state_guard` - A mutable reference to the `AppStateGuard`.
///
/// # Returns
///
/// A `Result` containing a tuple of the resolved UUID and the fully qualified name on success,
/// or a `ContextError` if resolution fails.
pub fn resolve_context(
    context: &str,
    state_guard: &mut AppStateGuard<'_>,
) -> ContextResult<(Uuid, String)> {
    let context = context.trim();

    let parts: Vec<&str> = context.split('/').collect();

    let first_part = if parts[0].trim().is_empty() {
        "."
    } else {
        parts[0]
    };
    let global_project_entry = state_guard
        .index()
        .projects
        .get(&GLOBAL_PROJECT_UUID)
        .unwrap();

    // --- 1. DETERMINE THE STARTING POINT AND TRAVERSAL PARTS ---
    // This logic implements the full precedence hierarchy.
    let session_uuid_opt = env::var("AXES_PROJECT_UUID")
        .ok()
        .and_then(|s| Uuid::parse_str(&s).ok());

    let (mut current_uuid, traversal_parts) = {
        match first_part {
            // --- PRIORITY 1: ABSOLUTE OVERRIDES (SESSION-IGNORANT) ---
            "." => {
                // Relative to CWD
                let uuid = find_project_from_path(&env::current_dir()?, true, state_guard.index())?;
                (uuid, &parts[1..])
            }
            "_" => {
                // Strictly relative to CWD
                let uuid =
                    find_project_from_path(&env::current_dir()?, false, state_guard.index())?;
                (uuid, &parts[1..])
            }
            "**" => {
                // Global last used
                let uuid = state_guard
                    .index()
                    .last_used
                    .ok_or(ContextError::NoLastUsedProject)?;
                (uuid, &parts[1..])
            }
            _ if first_part.ends_with('!') => {
                // Aliases
                let alias_name = first_part.strip_suffix('!').unwrap();
                let uuid = *state_guard.index().aliases.get(alias_name).ok_or_else(|| {
                    ContextError::AliasNotFound {
                        name: alias_name.to_string(),
                    }
                })?;
                (uuid, &parts[1..])
            }
            _ if first_part == global_project_entry.name => {
                // Global project name
                (GLOBAL_PROJECT_UUID, &parts[1..])
            }

            // --- PRIORITY 2: `axes` FOCUS-RELATIVE NAVIGATION (SESSION-AWARE `..`) ---
            ".." => {
                let focus_uuid = if let Some(session_uuid) = session_uuid_opt {
                    // In a session, `..` refers to the session project's parent.
                    session_uuid
                } else {
                    // Outside a session, `..` refers to the CWD project's parent.
                    find_project_from_path(&env::current_dir()?, true, state_guard.index())?
                };
                let focus_entry = state_guard.index().projects.get(&focus_uuid).unwrap();
                let parent_uuid = focus_entry.parent.ok_or(ContextError::AlreadyAtRoot)?;
                (parent_uuid, &parts[1..])
            }

            // --- PRIORITY 3: `axes` FOCUS-RELATIVE CHILD LOOKUP (SESSION-AWARE) ---
            _ => {
                // `first_part` is a simple name like "backend"
                let start_node = if let Some(session_uuid) = session_uuid_opt {
                    // In a session, resolve relative to the session project.
                    session_uuid
                } else {
                    // Outside a session, resolve relative to the global project.
                    GLOBAL_PROJECT_UUID
                };
                (start_node, &parts[..]) // Do not consume the part, it's the first child to find.
            }
        }
    };

    // --- 2. TRAVERSE THE PATH ---
    for part in traversal_parts {
        let current_entry = state_guard.index().projects.get(&current_uuid).unwrap();

        let next_uuid = match *part {
            "." | "_" | "**" => return Err(ContextError::GlobalRecentNotAtStart),
            ".." => current_entry.parent.ok_or(ContextError::AlreadyAtRoot)?,
            "*" => resolve_last_used_child(current_uuid, current_entry, state_guard.index())?,
            name => find_child_by_name(current_uuid, current_entry, name, state_guard.index())?,
        };
        current_uuid = next_uuid;
    }

    // --- 3. Finalize and Return ---
    state_guard.update_last_used_caches(current_uuid);
    let final_qualified_name =
        index_manager::build_qualified_name(current_uuid, state_guard.index())
            .ok_or(ContextError::AliasResolutionError)?;

    Ok((current_uuid, final_qualified_name))
}

/// Resolves '*' for a child, with interactive fallback.
///
/// # Arguments
///
/// * `parent_uuid` - The UUID of the parent project.
/// * `parent_entry` - A reference to the `IndexEntry` of the parent project.
/// * `index` - A reference to the `GlobalIndex`.
///
/// # Returns
///
/// A `Result` containing the UUID of the resolved child on success, or a `ContextError` if
/// resolution fails.
fn resolve_last_used_child(
    parent_uuid: Uuid,
    parent_entry: &IndexEntry,
    index: &GlobalIndex,
) -> ContextResult<Uuid> {
    // 1. Check cached `last_used_child` and VALIDATE that it still exists and is a child.
    if let Some(uuid) = parent_entry.last_used_child {
        if let Some(child_entry) = index.projects.get(&uuid)
            && child_entry.parent == Some(parent_uuid)
        {
            log::debug!(
                "Validated last used child '{}' for parent '{}'.",
                uuid,
                parent_entry.name
            );
            return Ok(uuid);
        }

        // If the check fails, the cache is stale. Proceed to fallback.
        log::warn!(
            "Stale 'last_used_child' cache for parent '{}'. Re-evaluating.",
            parent_entry.name
        );
    }

    // 2. Fallback: No valid cache. Find children and ask user if interactive.
    let mut children: Vec<(Uuid, &str)> = index
        .projects
        .iter()
        .filter(|(_, e)| e.parent == Some(parent_uuid))
        .map(|(uuid, e)| (*uuid, e.name.as_str()))
        .collect();

    if children.is_empty() {
        return Err(ContextError::NoLastUsedChild {
            parent_name: parent_entry.name.clone(),
        });
    }

    // Sort for deterministic selection in tests and UI.
    children.sort_by_key(|(_, name)| *name);

    println!(
        "Project '{}' has no recently used child.",
        parent_entry.name
    );
    let child_names: Vec<_> = children.iter().map(|(_, name)| *name).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Please select a child to continue:")
        .items(&child_names)
        .default(0)
        .interact_opt()?
        .ok_or(ContextError::Cancelled)?;

    Ok(children[selection].0) // Return the UUID directly from the collected tuple.
}

/// Finds a project's UUID by searching from a file system path.
///
/// # Arguments
///
/// * `path` - The path to search from.
/// * `search_up` - Whether to search up the directory tree.
/// * `index` - A reference to the `GlobalIndex`.
///
/// # Returns
///
/// A `Result` containing the UUID of the found project on success, or a `ContextError` if
/// no project is found.
fn find_project_from_path(
    path: &Path,
    search_up: bool,
    index: &GlobalIndex,
) -> ContextResult<Uuid> {
    let current_path = dunce::canonicalize(path)?;

    if search_up {
        // Mode '.' (ascending search)
        let mut candidates: Vec<(Uuid, &IndexEntry)> = index
            .projects
            .iter()
            .filter(|(_, entry)| current_path.starts_with(&entry.path))
            .map(|(uuid, entry)| (*uuid, entry))
            .collect();

        if candidates.is_empty() {
            return Err(ContextError::ProjectNotFoundFromPath);
        }

        // Sort by path length, from longest to shortest.
        candidates.sort_by_key(|(_, entry)| std::cmp::Reverse(entry.path.as_os_str().len()));

        // The first candidate is the most specific (the closest "ancestor").
        Ok(candidates[0].0)
    } else {
        // Mode '_' (strict search in current directory)
        index
            .projects
            .iter()
            .find(|(_, entry)| entry.path == current_path)
            .map(|(uuid, _)| *uuid)
            .ok_or(ContextError::ProjectNotFoundInCwd)
    }
}

/// Finds the UUID of a child by its name (logic moved from config_resolver).
///
/// # Arguments
///
/// * `parent_uuid` - The UUID of the parent project.
/// * `parent_entry` - A reference to the `IndexEntry` of the parent project.
/// * `child_name` - The name of the child to find.
/// * `index` - A reference to the `GlobalIndex`.
///
/// # Returns
///
/// A `Result` containing the UUID of the found child on success, or a `ContextError` if
/// the child is not found.
fn find_child_by_name(
    parent_uuid: Uuid,
    parent_entry: &IndexEntry,
    child_name: &str,
    index: &GlobalIndex,
) -> ContextResult<Uuid> {
    index
        .projects
        .iter()
        .find(|(_, e)| e.parent == Some(parent_uuid) && e.name == child_name)
        .map(|(uuid, _)| *uuid)
        .ok_or_else(|| ContextError::ChildProjectNotFound {
            child_name: child_name.to_string(),
            parent_name: parent_entry.name.clone(),
        })
}
