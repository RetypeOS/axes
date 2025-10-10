// src/core/context_resolver.rs

use crate::models::{GlobalIndex, IndexEntry};
use dialoguer::{Error as DialoguerError, Select, theme::ColorfulTheme};
use std::{env, path::Path};
use thiserror::Error;
use uuid::Uuid;

use crate::core::index_manager::{self, GLOBAL_PROJECT_UUID};

#[derive(Error, Debug)]
pub enum ContextError {
    #[error("Filesystem Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Index Error: {0}")]
    Index(#[from] crate::core::index_manager::IndexError),
    #[error("Error decoding cache: {0}")]
    BincodeDecode(#[from] bincode::error::DecodeError),
    #[error("Error encoding cache: {0}")]
    BincodeEncode(#[from] bincode::error::EncodeError),
    #[error("User Interface Error: {0}")]
    Dialoguer(#[from] DialoguerError),
    #[error("Empty context not provided.")]
    EmptyContext,
    #[error("Context '**' can only be used at the beginning of the path.")]
    GlobalRecentNotAtStart,
    #[error("Context '_' can only be used at the beginning of a path when outside a session.")]
    StrictLocalPathNotAtStart,
    #[error("Cannot go further up the hierarchy. Already at a root project.")]
    AlreadyAtRoot,
    #[error("No projects have been used recently. Cannot resolve '**'.")]
    NoLastUsedProject,
    #[error(
        "Parent project '{parent_name}' has not used any children recently. Cannot resolve '*'."
    )]
    NoLastUsedChild { parent_name: String },
    #[error("No axes project found in current directory or any parent directories.")]
    ProjectNotFoundFromPath,
    #[error("No axes project found in current directory.")]
    ProjectNotFoundInCwd,
    #[error("Root project with name '{name}' not found.")]
    RootProjectNotFound { name: String },
    #[error("Child project '{child_name}' not found for parent '{parent_name}'.")]
    ChildProjectNotFound {
        child_name: String,
        parent_name: String,
    },
    #[error("Alias '{name}!' not found.")]
    AliasNotFound { name: String },
    #[error("Could not resolve project name for alias (possible broken parent link).")]
    AliasResolutionError,
    #[error("Operation cancelled by user.")]
    Cancelled,
}

type ContextResult<T> = Result<T, ContextError>;

/// Resolves a project context string to its canonical UUID and fully qualified name.
/// The resolution follows a strict, multi-layered priority order to ensure
/// predictable behavior both inside and outside of project sessions.
pub fn resolve_context(context: &str, index: &mut GlobalIndex) -> ContextResult<(Uuid, String)> {
    let context = if context.trim().is_empty() { "." } else { context.trim() };

    let parts: Vec<&str> = context.split('/').collect();

    let first_part = parts[0];
    let global_project_entry = index.projects.get(&GLOBAL_PROJECT_UUID).unwrap();

    // --- 1. DETERMINE THE STARTING POINT AND TRAVERSAL PARTS ---
    // This logic implements the full precedence hierarchy.
    let session_uuid_opt = env::var("AXES_PROJECT_UUID")
        .ok()
        .and_then(|s| Uuid::parse_str(&s).ok());

    let (mut current_uuid, traversal_parts) =
        {
            match first_part {
                // --- PRIORITY 1: ABSOLUTE OVERRIDES (SESSION-IGNORANT) ---
                "." => {
                    // Relative to CWD
                    let uuid = find_project_from_path(&env::current_dir()?, true, index)?;
                    (uuid, &parts[1..])
                }
                "_" => {
                    // Strictly relative to CWD
                    let uuid = find_project_from_path(&env::current_dir()?, false, index)?;
                    (uuid, &parts[1..])
                }
                "**" => {
                    // Global last used
                    let uuid = index.last_used.ok_or(ContextError::NoLastUsedProject)?;
                    (uuid, &parts[1..])
                }
                _ if first_part.ends_with('!') => {
                    // Aliases
                    let alias_name = first_part.strip_suffix('!').unwrap();
                    let uuid = *index.aliases.get(alias_name).ok_or_else(|| {
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
                        find_project_from_path(&env::current_dir()?, true, index)?
                    };
                    let focus_entry = index.projects.get(&focus_uuid).unwrap();
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
        let current_entry = index.projects.get(&current_uuid).unwrap();

        let next_uuid = match *part {
            "." | "_" | "**" => return Err(ContextError::GlobalRecentNotAtStart),
            ".." => current_entry.parent.ok_or(ContextError::AlreadyAtRoot)?,
            "*" => resolve_last_used_child(current_uuid, current_entry, index)?,
            name => find_child_by_name(current_uuid, current_entry, name, index)?,
        };
        current_uuid = next_uuid;
    }

    // --- 3. Finalize and Return (Unchanged) ---
    update_last_used_caches(current_uuid, index)?;
    let final_qualified_name = index_manager::build_qualified_name(current_uuid, index)
        .ok_or(ContextError::AliasResolutionError)?;

    Ok((current_uuid, final_qualified_name))
}

/// Resolves '*' for a child, with interactive fallback.
fn resolve_last_used_child(
    parent_uuid: Uuid, // No longer needed directly, but kept for context
    parent_entry: &IndexEntry,
    index: &GlobalIndex,
) -> ContextResult<Uuid> {
    // Read directly from the parent's in-memory index entry.
    if let Some(uuid) = parent_entry.last_used_child {
        log::debug!(
            "Last used child '{}' found in index for parent '{}'.",
            uuid,
            parent_entry.name
        );
        return Ok(uuid);
    }

    // Fallback: no cache or empty. Ask the user.
    log::warn!(
        "No last used child cache found for '{}'. Initiating interactive fallback.",
        parent_entry.name
    );
    let children: Vec<_> = index
        .projects
        .values()
        .filter(|e| e.parent == Some(parent_uuid))
        .collect();

    if children.is_empty() {
        return Err(ContextError::NoLastUsedChild {
            parent_name: parent_entry.name.clone(),
        });
    }

    let child_names: Vec<_> = children.iter().map(|e| e.name.as_str()).collect();
    println!(
        "Project '{}' has no recently used child.",
        parent_entry.name
    );
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Please select a child to continue:")
        .items(&child_names)
        .default(0)
        .interact_opt()?
        .ok_or(ContextError::Cancelled)?;

    let selected_name = child_names[selection];
    find_child_by_name(parent_uuid, parent_entry, selected_name, index)
}

/// Finds a project's UUID by searching from a file system path.
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

/// Updates `last_used` information directly in the mutable GlobalIndex.
pub fn update_last_used_caches(final_uuid: Uuid, index: &mut GlobalIndex) -> ContextResult<()> {
    // 1. Update the global `last_used` for the `**` token.
    index.last_used = Some(final_uuid);

    // 2. Update the parent's `last_used_child` for the `*` token by traversing up.
    let mut current_entry = match index.projects.get(&final_uuid) {
        Some(entry) => entry.clone(), // Clone to avoid borrow checker issues
        None => return Ok(()),
    };
    let mut child_uuid_to_save = final_uuid;

    while let Some(parent_uuid) = current_entry.parent {
        if let Some(parent_entry) = index.projects.get_mut(&parent_uuid) {
            log::debug!(
                "Updating 'last_used_child' for parent '{}' to '{}'",
                parent_entry.name,
                child_uuid_to_save
            );
            // Mutate the parent entry in the index directly.
            parent_entry.last_used_child = Some(child_uuid_to_save);

            // Prepare for the next iteration up the tree.
            child_uuid_to_save = parent_uuid;
            current_entry = parent_entry.clone();
        } else {
            break; // Broken parent link, stop traversal.
        }
    }

    Ok(())
}
