// src/core/index_manager.rs

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

pub const GLOBAL_PROJECT_UUID: Uuid = Uuid::nil();

#[derive(Error, Debug)]
pub enum IndexError {
    #[error("Filesystem Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Error de rutas: {0}")]
    Path(#[from] crate::core::paths::PathError),
    #[error("Error al serializar a formato TOML: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("Project name '{name}' is already in use by another child of the same parent.")]
    NameAlreadyExists { name: String },
    #[error("Error al decodificar desde formato binario: {0}")]
    BincodeDecode(#[from] bincode::error::DecodeError),
    #[error("Error al codificar a formato binario: {0}")]
    BincodeEncode(#[from] bincode::error::EncodeError),
    #[error(
        "Enlace de padre roto: el proyecto '{child_uuid}' apunta a un padre inexistente '{missing_parent_uuid}'."
    )]
    BrokenParentLink {
        child_uuid: Uuid,
        missing_parent_uuid: Uuid,
    },
    #[error("Project with UUID '{uuid}' not found in global index.")]
    ProjectNotFoundInIndex { uuid: Uuid },
    #[error(
        "Dependencia circular detectada: el proyecto '{cycle_node_uuid}' ya es un ancestro de la ruta del nuevo padre. No se puede establecer este enlace."
    )]
    CircularDependency { cycle_node_uuid: Uuid },
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

pub fn read_project_ref(project_root: &Path) -> IndexResult<ProjectRef> {
    let ref_path = project_root
        .join(crate::constants::AXES_DIR)
        .join(PROJECT_REF_FILENAME);
    let bytes = fs::read(&ref_path)?;
    let (project_ref, _): (ProjectRef, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
    Ok(project_ref)
}

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

pub fn rename_project(
    index: &mut GlobalIndex,
    target_uuid: Uuid,
    new_name: &str,
) -> IndexResult<()> {
    // 1. Get the entry. Robust. `ok_or_else` prevents a panic if the UUID is invalid.
    let target_entry = index
        .projects
        .get(&target_uuid)
        .ok_or(IndexError::ProjectNotFoundInIndex { uuid: target_uuid })?;

    let parent_uuid = target_entry.parent;

    // 2. Collision validation. Robust. The logic with `.any()` is correct and efficient.
    // The `*uuid != target_uuid` ensures we don't compare ourselves to ourselves.
    let sibling_name_exists = index.projects.iter().any(|(uuid, entry)| {
        *uuid != target_uuid && entry.parent == parent_uuid && entry.name == new_name
    });

    if sibling_name_exists {
        return Err(IndexError::NameAlreadyExists {
            name: new_name.to_string(),
        });
    }

    // 3. Modification. Robust. `get_mut` is the correct way to modify a value in a HashMap.
    // The `else` with `Err` is an extra layer of security, although theoretically unreachable.
    if let Some(entry_to_modify) = index.projects.get_mut(&target_uuid) {
        entry_to_modify.name = new_name.to_string();
    } else {
        return Err(IndexError::ProjectNotFoundInIndex { uuid: target_uuid });
    }

    Ok(())
}

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

pub fn link_project(
    index: &mut GlobalIndex,
    project_to_move_uuid: Uuid,
    new_parent_uuid: Uuid,
) -> IndexResult<()> {
    // 1. A project cannot be moved to itself or to `global` arbitrarily if it is already a child of `global`.
    if project_to_move_uuid == new_parent_uuid {
        return Err(IndexError::CircularDependency {
            cycle_node_uuid: project_to_move_uuid,
        });
    }
    // A project cannot be its own parent.

    // 2. Anti-Cycle Validation
    // We create a temporary copy of the index with the proposed change to test the cycle.
    let mut temp_index_for_cycle_check = index.clone(); // Needs `Clone` for GlobalIndex
    if let Some(entry_to_modify) = temp_index_for_cycle_check
        .projects
        .get_mut(&project_to_move_uuid)
    {
        entry_to_modify.parent = Some(new_parent_uuid);
    } else {
        return Err(IndexError::ProjectNotFoundInIndex {
            uuid: project_to_move_uuid,
        });
    }

    if let Some(cycle_node_uuid) =
        find_cycle_from_node(project_to_move_uuid, &temp_index_for_cycle_check)?
    {
        return Err(IndexError::CircularDependency { cycle_node_uuid });
    }

    // 3. Sibling Name Collision Validation
    let project_to_move_entry = index.projects.get(&project_to_move_uuid).ok_or({
        IndexError::ProjectNotFoundInIndex {
            uuid: project_to_move_uuid,
        }
    })?;

    let sibling_name_exists = index.projects.iter().any(|(uuid, entry)| {
        *uuid != project_to_move_uuid && // It is not the project we are moving
        entry.parent == Some(new_parent_uuid) && // It is a child of the new parent
        entry.name == project_to_move_entry.name // And has the same name
    });

    if sibling_name_exists {
        return Err(IndexError::NameAlreadyExists {
            name: project_to_move_entry.name.clone(),
        });
    }

    // 4. If all validations pass, make the change in the actual index.
    if let Some(entry_to_modify) = index.projects.get_mut(&project_to_move_uuid) {
        entry_to_modify.parent = Some(new_parent_uuid);
    } else {
        return Err(IndexError::ProjectNotFoundInIndex {
            uuid: project_to_move_uuid,
        });
    }

    Ok(())
}

//Utils

/// Reads the `project_ref.bin` of a project. If it doesn't exist, it creates it from the global index.
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

/// Deletes a project from the index. Re-parents its direct children to 'global'.
pub fn delete_project_entry(index: &mut GlobalIndex, target_uuid: Uuid) -> Option<IndexEntry> {
    if target_uuid == GLOBAL_PROJECT_UUID {
        log::error!("No se puede eliminar el proyecto 'global'.");
        return None; // Cannot delete the global project
    }

    // Find all direct children of the node to be deleted
    let children_to_reparent: Vec<Uuid> = index
        .projects
        .iter()
        .filter(|(_, entry)| entry.parent == Some(target_uuid))
        .map(|(uuid, _)| *uuid)
        .collect();

    // Re-parent them to `global`
    for child_uuid in children_to_reparent {
        if let Some(child_entry) = index.projects.get_mut(&child_uuid) {
            child_entry.parent = Some(GLOBAL_PROJECT_UUID);
        }
    }

    // Finally, delete the project entry
    index.projects.remove(&target_uuid)
}

/// Collects all descendant UUIDs of an initial node.
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
    let mut warnings = Vec::new();
    let old_parent_name = index.projects.get(&old_parent_uuid).unwrap().name.clone();

    // Collect children to avoid borrowing issues
    let children_uuids: Vec<Uuid> = index
        .projects
        .values()
        .filter(|e| e.parent == Some(old_parent_uuid))
        .map(|e| index.projects.iter().find(|(_, val)| *val == e).unwrap().0) // Find UUID for entry
        .cloned()
        .collect();

    for child_uuid in children_uuids {
        let mut child_name = index.projects.get(&child_uuid).unwrap().name.clone();

        // Check for initial collision
        let sibling_names: HashSet<String> = index
            .projects
            .values()
            .filter(|e| e.parent == Some(new_parent_uuid))
            .map(|e| e.name.clone())
            .collect();

        if sibling_names.contains(&child_name) {
            // Collision detected, try automatic rename
            let new_child_name = format!("{}_{}", old_parent_name, child_name);
            if sibling_names.contains(&new_child_name) {
                // Automatic rename also fails, abort entire operation
                return Err(IndexError::NameAlreadyExists {
                    name: new_child_name,
                });
            }

            // Automatic rename is safe, update name and add a warning
            warnings.push(format!(
                "Child '{}' was automatically renamed to '{}' to avoid collision.",
                child_name, new_child_name
            ));
            child_name = new_child_name;
        }

        // Apply changes
        let child_entry = index.projects.get_mut(&child_uuid).unwrap();
        child_entry.name = child_name;
        child_entry.parent = Some(new_parent_uuid);
    }

    Ok(warnings)
}

/// Reconstructs a project's qualified name by traversing up the parent tree.
pub fn build_qualified_name(start_uuid: Uuid, index: &GlobalIndex) -> Option<String> {
    let mut parts = Vec::new();
    let mut current_uuid = Some(start_uuid);

    while let Some(uuid) = current_uuid {
        if let Some(entry) = index.projects.get(&uuid) {
            parts.push(entry.name.clone());
            current_uuid = entry.parent;
            // If parent is `None`, we have reached the root of the `axes` tree.
            if entry.parent.is_none() {
                break;
            }
        } else {
            // Broken link, unable to build full name.
            return None;
        }
    }

    parts.reverse();
    Some(parts.join("/"))
}

// Alias Handlers

/// Sets or updates an alias in the index.
pub fn set_alias(index: &mut GlobalIndex, name: String, target_uuid: Uuid) {
    index.aliases.insert(name, target_uuid);
}

/// Deletes an alias from the index. Returns `true` if the alias existed.
pub fn remove_alias(index: &mut GlobalIndex, name: &str) -> bool {
    index.aliases.remove(name).is_some()
}
