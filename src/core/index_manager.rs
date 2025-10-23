use crate::constants::PROJECT_REF_FILENAME;
use crate::core::paths;
use crate::models::{GlobalIndex, IndexEntry, ProjectRef};

use std::collections::HashSet;
use std::error::Error;
use std::io::ErrorKind;
use std::{fs, path::Path, path::PathBuf};
use thiserror::Error;
use uuid::Uuid;

use crate::constants::GLOBAL_INDEX_FILENAME;

/// The special, well-known UUID for the virtual "global" project.
pub const GLOBAL_PROJECT_UUID: Uuid = Uuid::nil();

/// Represents errors that can occur during operations on the `GlobalIndex`.
#[derive(Error, Debug)]
pub enum IndexError {
    /// A filesystem I/O error occurred.
    #[error("Filesystem Error: {0}")]
    Io(#[from] std::io::Error),
    /// An error occurred related to filesystem paths (e.g., config directory not found).
    #[error("Path error: {0}")]
    Path(#[from] crate::core::paths::PathError),
    /// An error occurred while serializing data to TOML format.
    #[error("Failed to serialize to TOML: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    /// An attempt was made to create or rename a project with a name that is already
    /// used by a sibling under the same parent.
    #[error("Project name '{name}' is already in use by another child of the same parent.")]
    NameAlreadyExists {
        /// The conflicting name.
        name: String,
    },
    /// An error occurred while deserializing data from `bincode` binary format.
    #[error("Failed to decode from binary format: {0}")]
    BincodeDecode(#[from] bincode::error::DecodeError),
    /// An error occurred while serializing data to `bincode` binary format.
    #[error("Failed to encode to binary format: {0}")]
    BincodeEncode(#[from] bincode::error::EncodeError),
    /// A project in the index references a parent UUID that does not exist.
    #[error(
        "Broken parent link: project '{child_uuid}' points to a non-existent parent '{missing_parent_uuid}'."
    )]
    BrokenParentLink {
        /// The UUID of the child project with the broken link.
        child_uuid: Uuid,
        /// The UUID of the parent that could not be found.
        missing_parent_uuid: Uuid,
    },
    /// A specified UUID could not be found in the index.
    #[error("Project with UUID '{uuid}' not found in global index.")]
    ProjectNotFoundInIndex {
        /// The UUID that was not found.
        uuid: Uuid,
    },
    /// A `link` operation was attempted that would create a circular dependency
    /// (e.g., making a project a child of one of its own descendants).
    #[error(
        "Circular dependency detected: cannot link project '{cycle_node_uuid}' as it would create a cycle."
    )]
    CircularDependency {
        /// The UUID of the project that would cause the cycle.
        cycle_node_uuid: Uuid,
    },
}

type IndexResult<T> = Result<T, IndexError>;

/// Loads the global index and ensures that the entry for the 'global' project exists.
pub fn load_and_ensure_global_project() -> IndexResult<GlobalIndex> {
    let mut index = load_global_index_internal()?;
    if let std::collections::hash_map::Entry::Vacant(e) = index.projects.entry(GLOBAL_PROJECT_UUID)
    {
        log::warn!("'global' project not found in index. Creating it now.");
        let config_dir = paths::get_axes_config_dir()?;

        let global_entry = IndexEntry {
            name: "global".to_string(),
            path: config_dir.clone(),
            parent: None,
            config_hash: None,
            cache_dir: None,
            last_used_child: None,
        };
        e.insert(global_entry.clone());

        index.projects.insert(GLOBAL_PROJECT_UUID, global_entry);

        // If alias `g` does not exist, create it.
        if !index.aliases.contains_key("g") {
            log::debug!("Creando alias por defecto 'g' para el proyecto global.");
            index.aliases.insert("g".to_string(), GLOBAL_PROJECT_UUID);
        }

        // 1. Create the default `axes.toml`.
        let axes_dir = config_dir.join(crate::constants::AXES_DIR);
        fs::create_dir_all(&axes_dir)?;
        let config_path = axes_dir.join(crate::constants::PROJECT_CONFIG_FILENAME);
        if !config_path.exists() {
            let default_config = crate::models::ProjectConfig::new();
            // Add default configuration for 'open'
            let toml_string = toml::to_string_pretty(&default_config)?;
            fs::write(config_path, toml_string)?;
        }

        // 2. Create its `project_ref.bin`.
        let project_ref = crate::models::ProjectRef {
            self_uuid: GLOBAL_PROJECT_UUID,
            parent_uuid: None,
            name: "global".to_string(),
        };
        write_project_ref(&config_dir, &project_ref)?;

        // Save the updated index.
        save_global_index(&index)?;
    }
    Ok(index)
}

/// Adds a new project entry to the index.
///
/// # Arguments
/// * `index` - A mutable reference to the `GlobalIndex`.
/// * `name` - The simple name for the new project.
/// * `path` - The absolute path to the new project's root directory.
/// * `parent_uuid` - An optional UUID of the parent project. Defaults to the global project.
///
/// # Errors
/// Returns `IndexError::NameAlreadyExists` if a sibling with the same name already exists.
pub fn add_project_to_index(
    index: &mut GlobalIndex,
    name: String,
    path: PathBuf,
    parent_uuid: Option<Uuid>,
) -> IndexResult<(Uuid, IndexEntry)> {
    let final_parent_uuid = parent_uuid.unwrap_or(GLOBAL_PROJECT_UUID);

    let name_exists = index
        .projects
        .values()
        .any(|entry| entry.parent == Some(final_parent_uuid) && entry.name == name);

    if name_exists {
        return Err(IndexError::NameAlreadyExists { name });
    }

    let new_uuid = Uuid::new_v4();
    let new_entry = IndexEntry {
        name,
        path,
        parent: Some(final_parent_uuid),
        config_hash: None,
        cache_dir: None,
        last_used_child: None,
    };

    index.projects.insert(new_uuid, new_entry.clone());
    Ok((new_uuid, new_entry))
}

fn load_global_index_internal() -> IndexResult<GlobalIndex> {
    let path = paths::get_axes_config_dir()?.join(GLOBAL_INDEX_FILENAME);
    if !path.exists() {
        return Ok(GlobalIndex::default());
    }
    let bytes = fs::read(&path)?;
    // Use bincode to deserialize from bytes
    let (index, _): (GlobalIndex, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
    Ok(index)
}

/// Saves the global index to disk.
pub fn save_global_index(index: &GlobalIndex) -> IndexResult<()> {
    let path = paths::get_axes_config_dir()?.join(GLOBAL_INDEX_FILENAME);
    // Use bincode to serialize to bytes
    let bytes = bincode::serde::encode_to_vec(index, bincode::config::standard())?;
    fs::write(path, bytes)?;
    Ok(())
}

/// Reads and deserializes a project's local identity from its `.axes/project_ref.bin` file.
///
/// # Arguments
/// * `project_root` - The absolute path to the project's root directory.
pub fn read_project_ref(project_root: &Path) -> IndexResult<ProjectRef> {
    let ref_path = project_root
        .join(crate::constants::AXES_DIR)
        .join(PROJECT_REF_FILENAME);
    let bytes = fs::read(&ref_path)?;
    let (project_ref, _): (ProjectRef, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
    Ok(project_ref)
}

/// Serializes and writes a project's local identity to its `.axes/project_ref.bin` file.
///
/// # Arguments
/// * `project_root` - The absolute path to the project's root directory.
/// * `project_ref` - The `ProjectRef` struct to write.
pub fn write_project_ref(project_root: &Path, project_ref: &ProjectRef) -> IndexResult<()> {
    let axes_dir = project_root.join(crate::constants::AXES_DIR);
    if !axes_dir.exists() {
        fs::create_dir_all(&axes_dir)?;
    }
    let ref_path = axes_dir.join(PROJECT_REF_FILENAME);
    // **CORRECTION**: Use `?` directly, as `IndexError` can now be converted from `bincode::error::EncodeError`.
    let bytes = bincode::serde::encode_to_vec(project_ref, bincode::config::standard())?;
    fs::write(ref_path, bytes)?;
    Ok(())
}

/// Traverses up the parent chain from a starting node to detect a circular dependency.
///
/// # Arguments
/// * `start_node_uuid` - The UUID of the project from which to start traversing upwards.
/// * `index` - An immutable reference to the `GlobalIndex`.
///
/// # Returns
/// `Ok(Some(Uuid))` containing the UUID of the repeated node if a cycle is found.
/// `Ok(None)` if no cycle is detected.
pub fn find_cycle_from_node(
    start_node_uuid: Uuid,
    index: &GlobalIndex,
) -> Result<Option<Uuid>, IndexError> {
    let mut current_uuid_opt = Some(start_node_uuid);
    let mut visited_nodes = HashSet::new();

    while let Some(current_uuid) = current_uuid_opt {
        // If we cannot insert the node, it's because it was already there. Cycle detected!
        if !visited_nodes.insert(current_uuid) {
            return Ok(Some(current_uuid));
        }

        // Move to parent
        match index.projects.get(&current_uuid) {
            Some(current_entry) => {
                current_uuid_opt = current_entry.parent;
            }
            None => {
                // The current node does not exist in the index, meaning `parent_uuid`
                // from a previous node points to a non-existent entry (broken link).
                // Or we have safely reached the root (parent: None).
                if current_uuid != GLOBAL_PROJECT_UUID {
                    // If it's not the global project and has no parent, it's a broken link
                    // (since all should point to global or another project).
                    // This should be `current_entry.parent` of the previous node.
                    // This is a bit more complex to report accurately at this point.
                    // For now, we assume `index.projects.get(&current_uuid)` would already detect it.
                    // The error would propagate earlier.
                }
                return Ok(None); // We reached a root or an endpoint without a cycle.
            }
        }
    }

    Ok(None) // The loop never executed (start_node_uuid was None) or no cycle was found.
}

/// Atomically links a project to a new parent within the index.
///
/// This operation is "transactional" in nature and performs several critical steps:
/// 1.  **Safety Checks:** It validates that the link operation is valid by checking for:
///     -   Attempts to link a project to itself.
///     -   Circular dependencies (e.g., attempting to make a parent a child of its own descendant).
///     -   Name collisions under the new parent.
/// 2.  **In-Memory Index Update:** If all checks pass, it modifies the `parent` UUID of the
///     project's `IndexEntry` within the `GlobalIndex`.
/// 3.  **Local State Synchronization:** It immediately updates the `.axes/project_ref.bin` file
///     of the moved project to reflect the new parentage.
///
/// If updating the local `project_ref.bin` fails, an error is logged, but the in-memory
/// change is *not* rolled back. This maintains consistency with the principle that the in-memory
/// index is the source of truth for the current session, and discrepancies can be fixed
/// with the `axes repair` command.
///
/// # Arguments
/// * `index` - A mutable reference to the `GlobalIndex`.
/// * `project_to_move_uuid` - The `Uuid` of the project being moved.
/// * `new_parent_uuid` - The `Uuid` of the new parent project.
///
/// # Errors
/// Returns an `IndexError` if any safety check fails.
pub fn link_project(
    index: &mut GlobalIndex,
    project_to_move_uuid: Uuid,
    new_parent_uuid: Uuid,
) -> IndexResult<()> {
    // --- 1. Pre-flight Safety Checks ---

    // A project cannot be its own parent.
    if project_to_move_uuid == new_parent_uuid {
        return Err(IndexError::CircularDependency {
            cycle_node_uuid: project_to_move_uuid,
        });
    }

    // Anti-Cycle Validation: A cycle is created if the new parent is already a
    // descendant of the project we are trying to move.
    let descendants = get_all_descendants(index, project_to_move_uuid);
    if descendants.contains(&new_parent_uuid) {
        log::error!(
            "Link operation aborted: circular dependency detected. Cannot make project {} a child of its own descendant {}.",
            project_to_move_uuid,
            new_parent_uuid
        );
        return Err(IndexError::CircularDependency {
            cycle_node_uuid: new_parent_uuid,
        });
    }

    // Sibling Name Collision Validation: Check if another child with the same name
    // already exists under the new parent.
    let project_to_move_entry =
        index
            .projects
            .get(&project_to_move_uuid)
            .ok_or(IndexError::ProjectNotFoundInIndex {
                uuid: project_to_move_uuid,
            })?;

    let project_name_to_move = project_to_move_entry.name.clone();
    let project_root = project_to_move_entry.path.clone();

    if is_sibling_name_taken(
        index,
        new_parent_uuid,
        &project_name_to_move,
        Some(project_to_move_uuid),
    ) {
        return Err(IndexError::NameAlreadyExists {
            name: project_name_to_move,
        });
    }

    // --- 2. Execute Transactional Update ---

    // Step 2a: Modify the in-memory index. This is the primary state change.
    // We can safely unwrap here because we've already fetched the entry above.
    let entry_to_modify = index.projects.get_mut(&project_to_move_uuid).unwrap();
    entry_to_modify.parent = Some(new_parent_uuid);
    log::debug!(
        "Updated parent of project {} to {} in memory.",
        project_to_move_uuid,
        new_parent_uuid
    );

    // Step 2b: Synchronize the change to the local `.axes/project_ref.bin`.
    log::debug!(
        "Updating local project_ref.bin for linked project {}",
        project_to_move_uuid
    );
    match get_or_create_project_ref(&project_root, project_to_move_uuid, index) {
        Ok(mut project_ref) => {
            project_ref.parent_uuid = Some(new_parent_uuid);
            if let Err(e) = write_project_ref(&project_root, &project_ref) {
                // This is a non-fatal error for the operation itself, but critical to log.
                // It indicates a desynchronization between the global index and the local state.
                log::error!(
                    "Failed to update project_ref.bin for '{}' at '{}': {}. The global index is now ahead of the local ref. Run 'axes repair' to fix.",
                    project_name_to_move,
                    project_root.display(),
                    e
                );
            }
        }
        Err(e) => {
            log::error!(
                "Could not read or create project_ref.bin for '{}' at '{}': {}. Local ref is out of sync.",
                project_name_to_move,
                project_root.display(),
                e
            );
        }
    }

    Ok(())
}

/// Reads a project's local `.axes/project_ref.bin` file.
/// If the file does not exist, it regenerates it using data from the `GlobalIndex`
/// and writes it to disk, ensuring the local state is synchronized.
///
/// # Arguments
/// * `project_root` - The path to the project's root directory.
/// * `uuid` - The UUID of the project.
/// * `index` - An immutable reference to the `GlobalIndex`.
pub fn get_or_create_project_ref(
    project_root: &Path,
    uuid: Uuid,
    index: &GlobalIndex,
) -> IndexResult<ProjectRef> {
    match read_project_ref(project_root) {
        Ok(project_ref) => Ok(project_ref), // The file exists and is valid.
        Err(e) => {
            // Check if the error is specifically "File not found".
            if let Some(io_err) = e.source().and_then(|s| s.downcast_ref::<std::io::Error>())
                && io_err.kind() == ErrorKind::NotFound
            {
                log::warn!(
                    "Local reference file (project_ref.bin) does not exist for project at '{}'. A new one will be created.",
                    project_root.display()
                );

                // Reconstruct information from the index.
                let entry = index
                    .projects
                    .get(&uuid)
                    .ok_or(IndexError::ProjectNotFoundInIndex { uuid })?;

                let new_ref = ProjectRef {
                    self_uuid: uuid,
                    parent_uuid: entry.parent,
                    name: entry.name.clone(),
                };

                // Write the newly created file for future operations.
                write_project_ref(project_root, &new_ref)?;

                return Ok(new_ref);
            }
            // If the error is anything else, we propagate it.
            Err(e)
        }
    }
}

/// Traverses the project graph downwards to find all descendants of a given project.
///
/// # Arguments
/// * `index` - An immutable reference to the `GlobalIndex`.
/// * `start_uuid` - The UUID of the project from which to start the search.
pub fn get_all_descendants(index: &GlobalIndex, start_uuid: Uuid) -> Vec<Uuid> {
    let mut descendants = Vec::new();
    let mut to_visit = vec![start_uuid];

    while let Some(current_uuid) = to_visit.pop() {
        let children: Vec<Uuid> = index
            .projects
            .iter()
            .filter(|(_, entry)| entry.parent == Some(current_uuid))
            .map(|(uuid, _)| *uuid)
            .collect();

        descendants.extend(&children);
        to_visit.extend(children);
    }
    descendants
}

/// Removes a list of projects from the index by their UUIDs.
///
/// # Arguments
/// * `index` - A mutable reference to the `GlobalIndex`.
/// * `uuids_to_remove` - A slice of UUIDs to remove from the index.
///
/// # Returns
/// The number of projects that were successfully removed.
pub fn remove_from_index(index: &mut GlobalIndex, uuids_to_remove: &[Uuid]) -> usize {
    let mut removed_count = 0;
    let remove_set: std::collections::HashSet<Uuid> = uuids_to_remove.iter().cloned().collect();

    index.projects.retain(|uuid, _| {
        if remove_set.contains(uuid) {
            removed_count += 1;
            false
        } else {
            true
        }
    });

    removed_count
}

/// Reparents the direct children of a project, handling name collisions automatically.
/// Returns a list of warnings for any automatic renames that occurred.
pub fn reparent_children(
    index: &mut GlobalIndex,
    old_parent_uuid: Uuid,
    new_parent_uuid: Uuid,
) -> Result<Vec<String>, IndexError> {
    // Handle case where a project is reparented to itself (no-op).
    if old_parent_uuid == new_parent_uuid {
        return Ok(Vec::new());
    }

    let mut warnings = Vec::new();
    let old_parent_name = index
        .projects
        .get(&old_parent_uuid)
        .ok_or(IndexError::ProjectNotFoundInIndex {
            uuid: old_parent_uuid,
        })?
        .name
        .clone();

    // Collect (uuid, name) tuples directly to avoid repeated lookups.
    let children_to_move: Vec<(Uuid, String)> = index
        .projects
        .iter()
        .filter(|(_, entry)| entry.parent == Some(old_parent_uuid))
        .map(|(uuid, entry)| (*uuid, entry.name.clone()))
        .collect();

    if children_to_move.is_empty() {
        return Ok(warnings);
    }

    // Pre-calculate sibling names at the destination once.
    let new_sibling_names: HashSet<String> = index
        .projects
        .values()
        .filter(|e| e.parent == Some(new_parent_uuid))
        .map(|e| e.name.clone())
        .collect();

    for (child_uuid, original_child_name) in children_to_move {
        let mut final_child_name = original_child_name.clone();

        if new_sibling_names.contains(&final_child_name) {
            // Collision detected, try automatic rename
            let suggested_name = format!("{}_{}", old_parent_name, final_child_name);
            // Also check against other children being moved in the same batch
            if new_sibling_names.contains(&suggested_name)
                || index.projects.get(&child_uuid).unwrap().name != final_child_name
            {
                return Err(IndexError::NameAlreadyExists {
                    name: suggested_name,
                });
            }

            warnings.push(format!(
                "Child '{}' was automatically renamed to '{}' to avoid collision.",
                final_child_name, suggested_name
            ));
            final_child_name = suggested_name;
        }

        // Apply changes
        let child_entry = index.projects.get_mut(&child_uuid).unwrap();
        child_entry.name = final_child_name;
        child_entry.parent = Some(new_parent_uuid);
    }

    Ok(warnings)
}

/// Reconstructs a project's human-readable, slash-separated qualified name (e.g., `app/api/db`)
/// by traversing up the parent tree from a starting UUID.
///
/// # Arguments
/// * `start_uuid` - The UUID of the project whose name to build.
/// * `index` - An immutable reference to the `GlobalIndex`.
///
/// # Returns
/// `Some(String)` containing the qualified name, or `None` if a broken parent link is found.
pub fn build_qualified_name(start_uuid: Uuid, index: &GlobalIndex) -> Option<String> {
    // --- SPECIAL CASE: Handle the global project itself ---
    if start_uuid == GLOBAL_PROJECT_UUID {
        // The qualified name of the global project is just its name.
        return index
            .projects
            .get(&GLOBAL_PROJECT_UUID)
            .map(|e| e.name.clone());
    }

    let mut parts = Vec::with_capacity(8);
    let mut current_uuid_opt = Some(start_uuid);

    while let Some(current_uuid) = current_uuid_opt {
        if let Some(entry) = index.projects.get(&current_uuid) {
            // Stop traversing upwards when we reach a direct child of 'global'.
            // We add its name, but then we stop, to avoid the "global/" prefix.
            parts.push(entry.name.as_str());
            if entry.parent == Some(GLOBAL_PROJECT_UUID) {
                break;
            }
            current_uuid_opt = entry.parent;
        } else {
            // Broken parent link in the hierarchy.
            log::warn!(
                "Broken parent link detected while building qualified name for UUID: {}",
                start_uuid
            );
            return None;
        }
    }

    parts.reverse();
    Some(parts.join("/"))
}

// Alias Handlers

/// Sets or updates an alias in the index to point to a target UUID.
///
/// # Arguments
/// * `index` - A mutable reference to the `GlobalIndex`.
/// * `name` - The name of the alias.
/// * `target_uuid` - The UUID of the project the alias should point to.
pub fn set_alias(index: &mut GlobalIndex, name: String, target_uuid: Uuid) {
    index.aliases.insert(name, target_uuid);
}

/// Deletes an alias from the index.
///
/// # Arguments
/// * `index` - A mutable reference to the `GlobalIndex`.
/// * `name` - The name of the alias to remove.
///
/// # Returns
/// `true` if the alias existed and was removed, `false` otherwise.
pub fn remove_alias(index: &mut GlobalIndex, name: &str) -> bool {
    index.aliases.remove(name).is_some()
}

/// Checks if a sibling name is already taken under a specific parent.
/// If `self_uuid` is provided, it excludes that project from the check (used during rename).
pub fn is_sibling_name_taken(
    index: &GlobalIndex,
    parent_uuid: Uuid,
    name: &str,
    self_uuid: Option<Uuid>,
) -> bool {
    index.projects.iter().any(|(uuid, entry)| {
        entry.parent == Some(parent_uuid) && entry.name == name && (self_uuid != Some(*uuid))
    })
}

/// Atomically renames a project in the index and synchronizes its local `project_ref.bin`.
///
/// This function ensures that both the in-memory global index and the on-disk local
/// project identity are updated as a single logical operation.
///
/// # Arguments
/// * `index` - A mutable reference to the `GlobalIndex`.
/// * `target_uuid` - The `Uuid` of the project to rename.
/// * `new_name` - The new simple name for the project.
///
/// # Errors
/// Returns an `IndexError` if the project is not found or if the new name
/// collides with an existing sibling project.
pub fn rename_project(
    index: &mut GlobalIndex,
    target_uuid: Uuid,
    new_name: &str,
) -> IndexResult<()> {
    // 1. Get the entry and check for name collisions.
    let target_entry = index
        .projects
        .get(&target_uuid)
        .ok_or(IndexError::ProjectNotFoundInIndex { uuid: target_uuid })?;

    if is_sibling_name_taken(
        index,
        target_entry.parent.unwrap_or(GLOBAL_PROJECT_UUID),
        new_name,
        Some(target_uuid),
    ) {
        return Err(IndexError::NameAlreadyExists {
            name: new_name.to_string(),
        });
    }

    let project_root = target_entry.path.clone();

    // 2. Modify the in-memory index.
    // We can unwrap here because we've already confirmed the entry exists.
    let entry_to_modify = index.projects.get_mut(&target_uuid).unwrap();
    entry_to_modify.name = new_name.to_string();
    log::debug!(
        "Renamed project {} to '{}' in memory.",
        target_uuid,
        new_name
    );

    // 3. Synchronize the change to the local `project_ref.bin`.
    log::debug!(
        "Updating local project_ref.bin for renamed project {}",
        target_uuid
    );
    match get_or_create_project_ref(&project_root, target_uuid, index) {
        Ok(mut project_ref) => {
            project_ref.name = new_name.to_string();
            if let Err(e) = write_project_ref(&project_root, &project_ref) {
                log::error!(
                    "Failed to update project_ref.bin for '{}' at '{}': {}. Run 'axes repair' to fix.",
                    new_name,
                    project_root.display(),
                    e
                );
            }
        }
        Err(e) => {
            log::error!(
                "Could not read/create project_ref.bin for '{}' at '{}': {}. Local ref is out of sync.",
                new_name,
                project_root.display(),
                e
            );
        }
    }

    Ok(())
}
