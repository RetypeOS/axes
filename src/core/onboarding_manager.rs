// src/core/onboarding_manager.rs

use crate::CancellationToken;
use crate::core::index_manager::{self, GLOBAL_PROJECT_UUID};
use crate::models::{GlobalIndex, IndexEntry, ProjectRef};
use dialoguer::{
    Confirm, Error as DialoguerError, Input, MultiSelect, Select, theme::ColorfulTheme,
};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum OnboardingError {
    #[error("Filesystem Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Index Error: {0}")]
    Index(#[from] crate::core::index_manager::IndexError),
    #[error("User Interface Error: {0}")]
    Dialoguer(#[from] DialoguerError),
    #[error("El directorio '{0}' no parece ser un proyecto de `axes` (falta '.axes/axes.toml').")]
    NotAnAxesProject(String),
    #[error("Project path is already registered with a different UUID.")]
    PathAlreadyRegistered,
    #[error("UUID conflict: project already registered at another path: '{0}'.")]
    UuidConflict(String),
    #[error("Project path does not have a valid name: '{0}'.")]
    InvalidProjectRootName(String),
    #[error("Operation cancelled by user.")]
    Cancelled,
}
type OnboardingResult<T> = Result<T, OnboardingError>;

pub struct OnboardingOptions {
    pub autosolve: bool,
    pub suggested_parent_uuid: Option<Uuid>,
}

/// The main function of the onboarding state machine.
pub fn register_project(
    path: &Path,
    index: &mut GlobalIndex,
    options: &OnboardingOptions,
) -> OnboardingResult<()> {
    let project_root = dunce::canonicalize(path)?;
    println!(
        "\n--- Analizando proyecto en: {} ---",
        project_root.display()
    );

    if !project_root.join(".axes/axes.toml").exists() {
        return Err(OnboardingError::NotAnAxesProject(
            project_root.display().to_string(),
        ));
    }

    // Check if the PATH is already registered. If so, skip to child scan.
    if let Some((uuid, _)) = index.projects.iter().find(|(_, e)| e.path == project_root) {
        println!("This project is already registered. Moving to child scan...");
        scan_and_register_children(&project_root, *uuid, index, options, cancellation_token)?;
        return Ok(());
    }

    // Try to read local identity
    match index_manager::read_project_ref(&project_root) {
        Ok(pref) => {
            // Case 1: `project_ref.bin` exists.
            handle_registration_with_ref(
                project_root.clone(),
                pref,
                index,
                options,
                cancellation_token,
            )?;
        }
        Err(_) => {
            // Case 2: `project_ref.bin` does not exist.
            handle_registration_without_ref(
                project_root.clone(),
                index,
                options,
                cancellation_token,
            )?;
        }
    };

    // Get the newly registered UUID for child scanning.
    if let Some((uuid, _)) = index.projects.iter().find(|(_, e)| e.path == project_root) {
        scan_and_register_children(&project_root, *uuid, index, options, cancellation_token)?;
    }

    Ok(())
}

fn handle_registration_with_ref(
    project_root: PathBuf,
    mut pref: ProjectRef, // Make it mutable to be able to correct it
    index: &mut GlobalIndex,
    options: &OnboardingOptions,
) -> OnboardingResult<()> {
    println!("Local reference (`project_ref.bin`) found. Validating...");

    // 1. Validate UUID and Path
    if let Some(existing_entry) = index.projects.get(&pref.self_uuid)
        && existing_entry.path != project_root
    {
        // UUID Conflict
        if options.autosolve {
            return Err(OnboardingError::UuidConflict(
                existing_entry.path.display().to_string(),
            ));
        }
        let prompt = format!(
            "This project's UUID is already registered at another path ({}). Update path to current location?",
            existing_entry.path.display()
        );
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(true)
            .interact()?
        {
            return Err(OnboardingError::Cancelled);
        }
        // The user accepted, the path update will be done at the end.
    }

    // 2. Validate Parent
    if let Some(parent_uuid) = pref.parent_uuid
        && !index.projects.contains_key(&parent_uuid)
    {
        if options.autosolve {
            return Err(OnboardingError::Index(
                index_manager::IndexError::ProjectNotFoundInIndex { uuid: parent_uuid },
            ));
        }
        println!(
            "Warning: The parent of this project (UUID: {}) is not registered.",
            parent_uuid
        );
        pref.parent_uuid = Some(choose_parent(index, None, cancellation_token)?); // Ask for new parent
    }

    // 3. Validate Name
    loop {
        let name_conflict = index
            .projects
            .values()
            .any(|entry| entry.parent == pref.parent_uuid && entry.name == pref.name);
        if !name_conflict {
            break; // The name is valid, exit the loop
        }

        if options.autosolve {
            return Err(OnboardingError::Index(
                index_manager::IndexError::NameAlreadyExists { name: pref.name },
            ));
        }

        println!(
            "Conflicto de nombre: El padre seleccionado ya tiene un hijo llamado '{}'.",
            pref.name
        );
        pref.name = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Por favor, introduce un nuevo nombre para este proyecto")
            .interact_text()?;
    }

    // 4. Register/Update in the index
    let final_entry = IndexEntry {
        name: pref.name.clone(),
        path: project_root.clone(),
        parent: pref.parent_uuid,
    };
    index.projects.insert(pref.self_uuid, final_entry);

    // 5. Update the local `project_ref.bin` to be consistent
    index_manager::write_project_ref(&project_root, &pref)?;

    println!("Project '{}' successfully registered/updated.", pref.name);
    Ok(())
}

fn handle_registration_without_ref(
    project_root: PathBuf,
    index: &mut GlobalIndex,
    options: &OnboardingOptions,
) -> OnboardingResult<()> {
    if options.autosolve {
        if let Some(parent_uuid) = options.suggested_parent_uuid {
            let name = project_root
                .file_name()
                .ok_or_else(|| {
                    OnboardingError::InvalidProjectRootName(project_root.display().to_string())
                })?
                .to_string_lossy()
                .into_owned();
            println!(
                "Modo --autosolve: registrando '{}' como hijo de proyecto sugerido.",
                name
            );
            let (new_uuid, _) = index_manager::add_project_to_index(
                index,
                name.clone(),
                project_root.clone(),
                Some(parent_uuid),
            )?;
            let new_ref = ProjectRef {
                self_uuid: new_uuid,
                parent_uuid: Some(parent_uuid),
                name,
            };
            index_manager::write_project_ref(&project_root, &new_ref)?;
        } else {
            return Err(OnboardingError::Cancelled);
        }
    } else {
        println!("No local reference found. Details will be requested.");

        let name_default = project_root
            .file_name()
            .ok_or_else(|| {
                OnboardingError::InvalidProjectRootName(project_root.display().to_string())
            })?
            .to_string_lossy()
            .into_owned();

        let name: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Nombre para este proyecto:")
            .default(name_default)
            .interact_text()?;

        let parent_uuid = choose_parent(index, options.suggested_parent_uuid, cancellation_token)?;
        let (new_uuid, _) = index_manager::add_project_to_index(
            index,
            name.clone(),
            project_root.clone(),
            Some(parent_uuid),
        )?;
        let new_ref = ProjectRef {
            self_uuid: new_uuid,
            parent_uuid: Some(parent_uuid),
            name,
        };
        index_manager::write_project_ref(&project_root, &new_ref)?;
        println!(
            "Project '{}' successfully registered and linked.",
            new_ref.name
        );
    }
    Ok(())
}

fn scan_and_register_children(
    project_root: &Path,
    parent_uuid: Uuid,
    index: &mut GlobalIndex,
    options: &OnboardingOptions,
) -> OnboardingResult<()> {
    if !options.autosolve
        && !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Scan subdirectories for unregistered children?")
            .default(true)
            .interact()?
    {
        return Ok(());
    }

    println!("Escaneando hijos...");
    let mut unregistered_children = Vec::new();
    for entry in fs::read_dir(project_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join(".axes/axes.toml").exists() {
            // It's an axes project. Is it already registered?
            if !index.projects.values().any(|e| e.path == path) {
                unregistered_children.push(path);
            }
        }
    }

    if unregistered_children.is_empty() {
        println!("No se encontraron hijos no registrados.");
        return Ok(());
    }

    let children_to_register = if options.autosolve {
        unregistered_children
    } else {
        let child_names: Vec<_> = unregistered_children
            .iter()
            .filter_map(|p| p.file_name().map(|f| f.to_string_lossy()))
            .collect();
        let selections = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("The following unregistered children were found. Select which ones to register (space to toggle, enter to continue):")
            .items(&child_names)
            .interact()?;

        selections
            .iter()
            .map(|i| unregistered_children[*i].clone())
            .collect()
    };

    for child_path in children_to_register {
        let child_options = OnboardingOptions {
            autosolve: options.autosolve,
            suggested_parent_uuid: Some(parent_uuid),
        };
        // RECURSIVE CALL
        register_project(&child_path, index, &child_options, cancellation_token)?;
    }

    Ok(())
}

fn choose_parent(
    index: &GlobalIndex,
    suggested_parent: Option<Uuid>,
) -> OnboardingResult<Uuid> {
    let mut parents: Vec<(Uuid, String)> = index
        .projects
        .iter()
        .map(|(uuid, entry)| (*uuid, entry.name.clone()))
        .collect();
    parents.sort_by_key(|(_, name)| name.clone());

    let parent_names: Vec<String> = parents.iter().map(|(_, name)| name.clone()).collect();

    let default_selection = suggested_parent
        .and_then(|s_uuid| parents.iter().position(|(uuid, _)| *uuid == s_uuid))
        .unwrap_or_else(|| {
            parents
                .iter()
                .position(|(uuid, _)| *uuid == GLOBAL_PROJECT_UUID)
                .unwrap()
        });

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Selecciona el proyecto padre:")
        .items(&parent_names)
        .default(default_selection)
        .interact()?;

    Ok(parents[selection].0)
}
