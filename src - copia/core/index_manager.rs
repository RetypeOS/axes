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

/// Carga el índice global y asegura que la entrada para el proyecto 'global' exista.
pub fn load_and_ensure_global_project() -> IndexResult<GlobalIndex> {
    let mut index = load_global_index_internal()?;
    if let std::collections::hash_map::Entry::Vacant(e) = index.projects.entry(GLOBAL_PROJECT_UUID)
    {
        log::warn!("'global' project not found in index. Creating it now.");
        let config_dir = paths::get_axes_config_dir()?;

        let global_entry = IndexEntry {
            name: "global".to_string(),
            path: config_dir.clone(), // Clonar para usarla después
            parent: None,
        };
        e.insert(global_entry.clone());

        index.projects.insert(GLOBAL_PROJECT_UUID, global_entry);

        // Si el alias `g` no existe, crearlo.
        if !index.aliases.contains_key("g") {
            log::debug!("Creando alias por defecto 'g' para el proyecto global.");
            index.aliases.insert("g".to_string(), GLOBAL_PROJECT_UUID);
        }

        // 1. Crear el `axes.toml` por defecto.
        let axes_dir = config_dir.join(crate::constants::AXES_DIR);
        fs::create_dir_all(&axes_dir)?;
        let config_path = axes_dir.join(crate::constants::PROJECT_CONFIG_FILENAME);
        if !config_path.exists() {
            let default_config = crate::models::ProjectConfig::new();
            // Añadir configuración por defecto para 'open'
            // NOTA: Esto requerirá que los modelos se actualicen primero. Lo haremos después.
            let toml_string = toml::to_string_pretty(&default_config)?;
            fs::write(config_path, toml_string)?;
        }

        // 2. Crear su `project_ref.bin`.
        let project_ref = crate::models::ProjectRef {
            self_uuid: GLOBAL_PROJECT_UUID,
            parent_uuid: None,
            name: "global".to_string(),
        };
        write_project_ref(&config_dir, &project_ref)?;

        // Guardar el índice actualizado.
        save_global_index(&index)?;
    }
    Ok(index)
}

/// Añade una nueva entrada de proyecto al índice.
pub fn add_project_to_index(
    index: &mut GlobalIndex,
    name: String,
    path: PathBuf,
    parent_uuid: Option<Uuid>,
) -> IndexResult<(Uuid, IndexEntry)> {
    let final_parent_uuid = parent_uuid.unwrap_or(GLOBAL_PROJECT_UUID);

    let name_exists = index.projects.values().any(|entry| {
        if name == "global" {
            false
        } else {
            entry.parent == Some(final_parent_uuid) && entry.name == name
        }
    });

    if name_exists {
        return Err(IndexError::NameAlreadyExists { name });
    }

    let new_uuid = Uuid::new_v4();
    let new_entry = IndexEntry {
        name,
        path,
        parent: Some(final_parent_uuid),
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
    // Usar bincode para deserializar desde los bytes
    let (index, _): (GlobalIndex, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())?;
    Ok(index)
}

/// Guarda el índice global en el disco.
pub fn save_global_index(index: &GlobalIndex) -> IndexResult<()> {
    let path = paths::get_axes_config_dir()?.join(GLOBAL_INDEX_FILENAME);
    // Usar bincode para serializar a bytes
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
    // **CORRECCIÓN**: Usar `?` directamente, ya que `IndexError` ahora puede convertirse desde `bincode::error::EncodeError`.
    let bytes = bincode::serde::encode_to_vec(project_ref, bincode::config::standard())?;
    fs::write(ref_path, bytes)?;
    Ok(())
}

pub fn rename_project(
    index: &mut GlobalIndex,
    target_uuid: Uuid,
    new_name: &str,
) -> IndexResult<()> {
    // 1. Obtener la entrada. Robusto. `ok_or_else` previene un panic si el UUID es inválido.
    let target_entry = index
        .projects
        .get(&target_uuid)
        .ok_or(IndexError::ProjectNotFoundInIndex { uuid: target_uuid })?;

    let parent_uuid = target_entry.parent;

    // 2. Validación de colisión. Robusto. La lógica con `.any()` es correcta y eficiente.
    // El `*uuid != target_uuid` asegura que no nos comparemos con nosotros mismos.
    let sibling_name_exists = index.projects.iter().any(|(uuid, entry)| {
        *uuid != target_uuid && entry.parent == parent_uuid && entry.name == new_name
    });

    if sibling_name_exists {
        return Err(IndexError::NameAlreadyExists {
            name: new_name.to_string(),
        });
    }

    // 3. Modificación. Robusto. `get_mut` es la forma correcta de modificar un valor en un HashMap.
    // El `else` con el `Err` es una capa extra de seguridad, aunque teóricamente inalcanzable.
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
        // Si no podemos insertar el nodo, es porque ya estaba. ¡Ciclo detectado!
        if !visited_nodes.insert(current_uuid) {
            return Ok(Some(current_uuid));
        }

        // Moverse al padre
        match index.projects.get(&current_uuid) {
            Some(current_entry) => {
                current_uuid_opt = current_entry.parent;
            }
            None => {
                // El nodo actual no existe en el índice, significa que el `parent_uuid`
                // de un nodo anterior apunta a una entrada inexistente (enlace roto).
                // O hemos llegado a la raíz (parent: None) de forma segura.
                if current_uuid != GLOBAL_PROJECT_UUID {
                    // Si no es el proyecto global y no tiene padre, es un enlace roto
                    // (ya que todos deberían apuntar a global o a otro proyecto).
                    // Esto debería ser `current_entry.parent` del anterior nodo.
                    // Esto es un poco más complejo de reportar con precisión en este punto.
                    // Por ahora, asumimos que `index.projects.get(&current_uuid)` ya lo detectaría.
                    // El error se propagaría antes.
                }
                return Ok(None); // Llegamos a una raíz o a un punto final sin ciclo.
            }
        }
    }

    Ok(None) // El bucle nunca se ejecutó (start_node_uuid era None) o no se encontró ciclo.
}

pub fn link_project(
    index: &mut GlobalIndex,
    project_to_move_uuid: Uuid,
    new_parent_uuid: Uuid,
) -> IndexResult<()> {
    // 1. No se puede mover un proyecto a sí mismo o a `global` de forma arbitraria si ya es hijo de `global`.
    if project_to_move_uuid == new_parent_uuid {
        return Err(IndexError::CircularDependency {
            cycle_node_uuid: project_to_move_uuid,
        });
    }
    // Un proyecto no puede ser padre de sí mismo.

    // 2. Validación de Anti-Ciclos
    // Creamos una copia temporal del índice con el cambio propuesto para testear el ciclo.
    let mut temp_index_for_cycle_check = index.clone(); // Necesita `Clone` para GlobalIndex
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

    // 3. Validación de Colisión de Nombres de Hermano
    let project_to_move_entry = index.projects.get(&project_to_move_uuid).ok_or({
        IndexError::ProjectNotFoundInIndex {
            uuid: project_to_move_uuid,
        }
    })?;

    let sibling_name_exists = index.projects.iter().any(|(uuid, entry)| {
        *uuid != project_to_move_uuid && // No es el proyecto que estamos moviendo
        entry.parent == Some(new_parent_uuid) && // Es hijo del nuevo padre
        entry.name == project_to_move_entry.name // Y tiene el mismo nombre
    });

    if sibling_name_exists {
        return Err(IndexError::NameAlreadyExists {
            name: project_to_move_entry.name.clone(),
        });
    }

    // 4. Si todas las validaciones pasan, realizar el cambio en el índice real.
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

/// Lee el `project_ref.bin` de un proyecto. Si no existe, lo crea a partir del índice global.
pub fn get_or_create_project_ref(
    project_root: &Path,
    uuid: Uuid,
    index: &GlobalIndex,
) -> IndexResult<ProjectRef> {
    match read_project_ref(project_root) {
        Ok(project_ref) => Ok(project_ref), // El archivo existe y es válido.
        Err(e) => {
            // Comprobar si el error es específicamente "Archivo no encontrado".
            if let Some(io_err) = e.source().and_then(|s| s.downcast_ref::<std::io::Error>())
                && io_err.kind() == ErrorKind::NotFound
            {
                log::warn!(
                    "Local reference file (project_ref.bin) does not exist for project at '{}'. A new one will be created.",
                    project_root.display()
                );

                // Reconstruir la información desde el índice.
                let entry = index
                    .projects
                    .get(&uuid)
                    .ok_or(IndexError::ProjectNotFoundInIndex { uuid })?;

                let new_ref = ProjectRef {
                    self_uuid: uuid,
                    parent_uuid: entry.parent,
                    name: entry.name.clone(),
                };

                // Escribir el archivo recién creado para futuras operaciones.
                write_project_ref(project_root, &new_ref)?;

                return Ok(new_ref);
            }
            // Si el error es cualquier otra cosa, lo propagamos.
            Err(e)
        }
    }
}

/// Elimina un proyecto del índice. Re-parenta a sus hijos directos a 'global'.
pub fn delete_project_entry(index: &mut GlobalIndex, target_uuid: Uuid) -> Option<IndexEntry> {
    if target_uuid == GLOBAL_PROJECT_UUID {
        log::error!("No se puede eliminar el proyecto 'global'.");
        return None; // No se puede eliminar el proyecto global
    }

    // Encontrar todos los hijos directos del nodo a eliminar
    let children_to_reparent: Vec<Uuid> = index
        .projects
        .iter()
        .filter(|(_, entry)| entry.parent == Some(target_uuid))
        .map(|(uuid, _)| *uuid)
        .collect();

    // Re-parentarlos a `global`
    for child_uuid in children_to_reparent {
        if let Some(child_entry) = index.projects.get_mut(&child_uuid) {
            child_entry.parent = Some(GLOBAL_PROJECT_UUID);
        }
    }

    // Finalmente, eliminar la entrada del proyecto
    index.projects.remove(&target_uuid)
}

/// Recolecta todos los UUIDs descendientes de un nodo inicial.
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

pub fn remove_from_index(
    index: &mut GlobalIndex,
    uuids_to_remove: &[Uuid],
    should_reparent_orphans: bool,
) -> usize {
    let mut removed_count = 0;
    let remove_set: std::collections::HashSet<Uuid> = uuids_to_remove.iter().cloned().collect();

    if should_reparent_orphans {
        let children_to_reparent: Vec<Uuid> = index
            .projects
            .iter()
            .filter(|(_, entry)| {
                entry
                    .parent
                    .is_some_and(|p_uuid| remove_set.contains(&p_uuid))
            })
            .map(|(uuid, _)| *uuid)
            .collect();

        for child_uuid in children_to_reparent {
            if let Some(child_entry) = index.projects.get_mut(&child_uuid) {
                child_entry.parent = Some(GLOBAL_PROJECT_UUID);
            }
        }
    }

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

/// Reconstruye el nombre cualificado de un proyecto subiendo por el árbol de padres.
pub fn build_qualified_name(start_uuid: Uuid, index: &GlobalIndex) -> Option<String> {
    let mut parts = Vec::new();
    let mut current_uuid = Some(start_uuid);

    while let Some(uuid) = current_uuid {
        if let Some(entry) = index.projects.get(&uuid) {
            parts.push(entry.name.clone());
            current_uuid = entry.parent;
            // Si el padre es `None`, hemos llegado a la raíz del árbol de `axes`.
            if entry.parent.is_none() {
                break;
            }
        } else {
            // Enlace roto, no se puede construir el nombre completo.
            return None;
        }
    }

    parts.reverse();
    Some(parts.join("/"))
}

// Alias Handlers

/// Establece o actualiza un alias en el índice.
pub fn set_alias(index: &mut GlobalIndex, name: String, target_uuid: Uuid) {
    index.aliases.insert(name, target_uuid);
}

/// Elimina un alias del índice. Devuelve `true` si el alias existía.
pub fn remove_alias(index: &mut GlobalIndex, name: &str) -> bool {
    // Proteger el alias 'g'
    if name.to_lowercase() == "g" {
        log::warn!("No se puede eliminar el alias protegido 'g'.");
        return false;
    }
    index.aliases.remove(name).is_some()
}
