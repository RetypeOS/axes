// src/core/context_resolver.rs

use crate::models::{GlobalIndex, IndexEntry, LastUsedCache};
use dialoguer::{Error as DialoguerError, Select, theme::ColorfulTheme};
use std::{env, fs, path::Path};
use thiserror::Error;
use uuid::Uuid;

use crate::constants::AXES_DIR;
use crate::constants::LAST_USED_CACHE_FILENAME;
use crate::core::index_manager::{self, GLOBAL_PROJECT_UUID};

use bincode::error::DecodeError;

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
///
/// The resolution follows a strict priority order:
/// 1.  **Absolute Contexts:** If the context starts with an alias (e.g., `g!`) or the
///     name of the root project (e.g., `global`), it is resolved from the absolute
///     root of the index, completely ignoring any active session.
/// 2.  **Session-Relative Contexts:** If not an absolute context and a session is active
///     (via `AXES_PROJECT_UUID`), the context is resolved relative to the session's project.
/// 3.  **Filesystem/Global-Relative Contexts:** If neither of the above, resolution proceeds
///     relative to the filesystem (`.`, `..`) or the global project.
pub fn resolve_context(context: &str, index: &GlobalIndex) -> ContextResult<(Uuid, String)> {
    let parts: Vec<&str> = context.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Err(ContextError::EmptyContext);
    }

    let first_part = parts[0];
    let global_project_name = &index.projects.get(&GLOBAL_PROJECT_UUID).unwrap().name;

    // --- 1. Determine the starting point of the traversal ---
    let (mut current_uuid, mut current_parent_uuid) = if let Some(alias_name) = first_part.strip_suffix('!') {
        // --- PRIORITY 1A: Absolute resolution via Alias ---
        let uuid = index
            .aliases
            .get(alias_name)
            .ok_or_else(|| ContextError::AliasNotFound {
                name: alias_name.to_string(),
            })?;
        let entry = index.projects.get(uuid).unwrap();
        (*uuid, entry.parent)
    } else if first_part == *global_project_name {
        // --- PRIORITY 1B: Absolute resolution via Global Project Name ---
        (GLOBAL_PROJECT_UUID, None)
    } else {
        // --- PRIORITY 2 & 3: Relative resolution ---
        if let Some(session_uuid) = env::var("AXES_PROJECT_UUID")
            .ok()
            .and_then(|s| Uuid::parse_str(&s).ok())
        {
            // --- PRIORITY 2: Session-Relative Resolution ---
            let session_entry = index.projects.get(&session_uuid).unwrap();
            match first_part {
                "." | "_" => (session_uuid, session_entry.parent),
                ".." => {
                    let parent_uuid = session_entry.parent.ok_or(ContextError::AlreadyAtRoot)?;
                    let parent_entry = index.projects.get(&parent_uuid).unwrap();
                    (parent_uuid, parent_entry.parent)
                }
                "*" => {
                    let child_uuid = resolve_last_used_child(session_uuid, session_entry, index)?;
                    (child_uuid, Some(session_uuid))
                }
                name => {
                    let child_uuid = find_child_by_name(session_uuid, session_entry, name, index)?;
                    (child_uuid, Some(session_uuid))
                }
            }
        } else {
            // --- PRIORITY 3: Filesystem/Global-Relative Resolution (No Session) ---
            match first_part {
                "." => {
                    let uuid = find_project_from_path(&env::current_dir()?, true, index)?;
                    let entry = index.projects.get(&uuid).unwrap();
                    (uuid, entry.parent)
                }
                "_" => {
                    let uuid = find_project_from_path(&env::current_dir()?, false, index)?;
                    let entry = index.projects.get(&uuid).unwrap();
                    (uuid, entry.parent)
                }
                ".." => {
                    let cwd_uuid = find_project_from_path(&env::current_dir()?, true, index)?;
                    let cwd_entry = index.projects.get(&cwd_uuid).unwrap();
                    let parent_uuid = cwd_entry.parent.ok_or(ContextError::AlreadyAtRoot)?;
                    let parent_entry = index.projects.get(&parent_uuid).unwrap();
                    (parent_uuid, parent_entry.parent)
                }
                "**" => {
                    let uuid = index.last_used.ok_or(ContextError::NoLastUsedProject)?;
                    let entry = index.projects.get(&uuid).unwrap();
                    (uuid, entry.parent)
                }
                "*" => {
                    let global_entry = index.projects.get(&GLOBAL_PROJECT_UUID).unwrap();
                    let child_uuid = resolve_last_used_child(GLOBAL_PROJECT_UUID, global_entry, index)?;
                    (child_uuid, Some(GLOBAL_PROJECT_UUID))
                }
                name => {
                    let global_entry = index.projects.get(&GLOBAL_PROJECT_UUID).unwrap();
                    let child_uuid = find_child_by_name(GLOBAL_PROJECT_UUID, global_entry, name, index)?;
                    (child_uuid, Some(GLOBAL_PROJECT_UUID))
                }
            }
        }
    };

    // --- 2. Iterate over the remaining parts of the context path ---
    for part in &parts[1..] {
        let (next_uuid, next_parent_uuid) = match *part {
            "." => (current_uuid, current_parent_uuid),
            ".." => {
                let parent_uuid = current_parent_uuid.ok_or(ContextError::AlreadyAtRoot)?;
                let parent_entry = index.projects.get(&parent_uuid).unwrap();
                (parent_uuid, parent_entry.parent)
            }
            "*" => {
                let parent_entry = index.projects.get(&current_uuid).unwrap();
                let child_uuid = resolve_last_used_child(current_uuid, parent_entry, index)?;
                (child_uuid, Some(current_uuid))
            }
            "**" => return Err(ContextError::GlobalRecentNotAtStart),
            "_" => return Err(ContextError::StrictLocalPathNotAtStart),
            name => {
                let parent_entry = index.projects.get(&current_uuid).unwrap();
                let child_uuid = find_child_by_name(current_uuid, parent_entry, name, index)?;
                (child_uuid, Some(current_uuid))
            }
        };
        current_uuid = next_uuid;
        current_parent_uuid = next_parent_uuid;
    }

    // --- 3. Finalize and Return ---
    update_last_used_caches(current_uuid, index)?;
    let final_qualified_name = index_manager::build_qualified_name(current_uuid, index)
        .ok_or(ContextError::AliasResolutionError)?;

    Ok((current_uuid, final_qualified_name))
}

/// Resolves '*' for a child, with interactive fallback.
fn resolve_last_used_child(
    parent_uuid: Uuid,
    parent_entry: &IndexEntry,
    index: &GlobalIndex,
) -> ContextResult<Uuid> {
    let cache_path = parent_entry
        .path
        .join(AXES_DIR)
        .join(LAST_USED_CACHE_FILENAME);
    if let Ok(Some(cache)) = read_last_used_cache(&cache_path)
        && let Some(uuid) = cache.child_uuid
    {
        log::debug!(
            "Last used child '{}' found in cache for '{}'.",
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

/// Reads the "last used" cache for a parent project.
fn read_last_used_cache(path: &Path) -> ContextResult<Option<LastUsedCache>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;

    let decode_result: Result<(LastUsedCache, usize), _> =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard());

    match decode_result {
        Ok((cache, _)) => Ok(Some(cache)),
        Err(e) => {
            if !matches!(e, DecodeError::Io { .. }) {
                log::warn!(
                    "'last used' cache at '{}' is corrupt. It will be regenerated. (Error: {})",
                    path.display(),
                    e
                );
                let _ = fs::remove_file(path);
                Ok(None)
            } else {
                Err(ContextError::BincodeDecode(e))
            }
        }
    }
}

/// Writes the "last used" cache for a parent project.
fn write_last_used_cache(path: &Path, cache: &LastUsedCache) -> ContextResult<()> {
    let cache_dir = path.parent().unwrap(); // Ensures the directory exists
    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir)?;
    }
    let bytes = bincode::serde::encode_to_vec(cache, bincode::config::standard())?;
    fs::write(path, bytes)?;
    Ok(())
}

fn update_last_used_caches(final_uuid: Uuid, index: &GlobalIndex) -> ContextResult<()> {
    // 1. Update the global `last_used`.
    let mut global_index = index_manager::load_and_ensure_global_project()?;
    global_index.last_used = Some(final_uuid);
    index_manager::save_global_index(&global_index)?;

    // 2. Update child caches (`*`) by moving up the tree.
    let mut current_entry = index.projects.get(&final_uuid).unwrap();
    let mut child_uuid_to_save = final_uuid;

    // Climb the inheritance chain
    while let Some(parent_uuid) = current_entry.parent {
        if let Some(parent_entry) = index.projects.get(&parent_uuid) {
            log::debug!(
                "Updating 'last used' for parent '{}' to '{}'",
                parent_entry.name,
                child_uuid_to_save
            );
            let cache = LastUsedCache {
                child_uuid: Some(child_uuid_to_save),
            };
            let cache_path = parent_entry
                .path
                .join(AXES_DIR)
                .join(LAST_USED_CACHE_FILENAME);

            // Call the function that was not used before
            write_last_used_cache(&cache_path, &cache)?;

            // Prepare for the next iteration
            child_uuid_to_save = parent_uuid;
            current_entry = parent_entry;
        } else {
            // If the parent is not found in the index (broken link), we stop.
            break;
        }
    }

    Ok(())
}
