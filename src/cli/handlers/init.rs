// EN: src/cli/handlers/init.rs

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use dialoguer::{self, theme::ColorfulTheme, Input};
use std::{collections::HashMap, env, fs, path::Path, sync::atomic::Ordering};
use uuid::Uuid;

use super::commons::{self, check_for_cancellation};
use crate::{
    cli::args::InitArgs,
    constants::{AXES_DIR, PROJECT_CONFIG_FILENAME, PROJECT_REF_FILENAME},
    core::{context_resolver, index_manager},
    models::{ProjectConfig, ProjectRef}, CancellationToken,
};

use colored::Colorize;

/// The main handler for the `init` command.
/// Allows creating and registering new projects to axes.
pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    // 1. Parse arguments
    let init_args = InitArgs::try_parse_from(&args)?;

    let target_dir = env::current_dir()?;
    println!("Initializing project in: {}", target_dir.display());

    // 1.5 Validate target directory
    let axes_dir = target_dir.join(AXES_DIR);
    if axes_dir.exists() {
        return Err(anyhow!(
            "An '{}' directory already exists at this location.",
            AXES_DIR
        ));
    }

    let is_interactive = !init_args.autosolve;

    // 2. Resolve configuration details
    let project_name = resolve_project_name(&init_args, &target_dir, is_interactive, cancellation_token)?;
    check_for_cancellation(cancellation_token)?;
    let parent_uuid = resolve_parent_project(&init_args, is_interactive, cancellation_token)?;
    check_for_cancellation(cancellation_token)?;

    // --- Interactive step for version and description ---
    let version = resolve_project_version(&init_args, is_interactive)?;
    check_for_cancellation(cancellation_token)?;
    let description = resolve_project_description(&init_args, &project_name, is_interactive)?;
    check_for_cancellation(cancellation_token)?;

    // 3. Build the project configuration object
    let mut project_config = ProjectConfig::new_for_init(&project_name, &version, &description);

    // Apply overrides from flags (`--env`, `--var`) if they exist.
    let env_vars = parse_key_value_pairs(&init_args.env)?;
    project_config.env.extend(env_vars);

    let vars = parse_key_value_pairs(&init_args.var)?;
    project_config.vars.extend(vars);

    // 4. Perform filesystem and index operations
    let mut index = index_manager::load_and_ensure_global_project()?;

    let (new_uuid, _) = index_manager::add_project_to_index(
        &mut index,
        project_name.clone(), // `project_name` is the validated name
        target_dir.clone(),
        Some(parent_uuid),
    )
    .with_context(
        || "Could not add project to global index. A sibling with the same name might exist.",
    )?;

    // 4.2. Write local files (`.axes/axes.toml`, `.axes/project_ref.bin`)
    fs::create_dir_all(&axes_dir)?;

    let config_path = axes_dir.join(PROJECT_CONFIG_FILENAME);
    let toml_string = toml::to_string_pretty(&project_config)?;
    fs::write(&config_path, toml_string)?;

    let project_ref = ProjectRef {
        self_uuid: new_uuid,
        parent_uuid: Some(parent_uuid),
        name: project_name.clone(),
    };
    index_manager::write_project_ref(&target_dir, &project_ref).with_context(|| {
        format!(
            "Could not write project reference file ('{}').",
            PROJECT_REF_FILENAME
        )
    })?;

    // 4.3. Save updated global index
    index_manager::save_global_index(&index).with_context(|| t!("error.saving_global_index"))?;

    println!("\n{}", t!("common.success"));
    println!(
        "  Project '{}' created with UUID: {}",
        project_name, new_uuid
    );
    println!("  Configuration created at: {}", config_path.display());
    println!("  Successfully registered in global index.");

    Ok(())
}

// --- Funciones Auxiliares ---

/// Resuelve el nombre del proyecto, ya sea desde flags, el directorio, o interactivamente.
fn resolve_project_name(
    args: &InitArgs,
    target_dir: &Path,
    is_interactive: bool,
    cancellation_token: &CancellationToken,
) -> Result<String> {
    if let Some(name) = &args.name {
        // If name is provided via flag, validate it. Failure is a hard error.
        return commons::validate_project_name(name);
    }

    let default_name = target_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    if is_interactive {
        // In interactive mode, loop until a valid name is provided.
        loop {
            let input = match Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Project name")
                .default(default_name.clone())
                .interact_text() {
                    Ok(i) => i,
                    //Err(dialoguer::Error::Interrupt) => {
                    //    return Err(anyhow!(t!("common.error.operation_cancelled")));
                    //},
                    Err(e) => return Err(e.into()),
                };
            
            check_for_cancellation(cancellation_token)?;

            match commons::validate_project_name(&input) {
                Ok(name) => return Ok(name),
                Err(e) => {
                    // Print the validation error and prompt the user to try again.
                    println!("{}", format!("Error: {}", e).red());
                    continue;
                }
            }

        }
    } else {
        // In non-interactive mode, use the default name and validate it.
        commons::validate_project_name(&default_name)
    }
}

/// Resuelve el UUID del padre, desde flags, interactivamente, o usando 'global' como default.
fn resolve_parent_project(args: &InitArgs, is_interactive: bool, cancellation_token: &CancellationToken) -> Result<Uuid> {
    let index = index_manager::load_and_ensure_global_project()?;

    if let Some(parent_context) = &args.parent {
        println!("Resolving parent '{}'...", parent_context);
        let (uuid, qualified_name) = context_resolver::resolve_context(parent_context, &index, cancellation_token)?;
        println!(
            "Parent project '{}' found (UUID: {}).",
            qualified_name, uuid
        );
        return Ok(uuid);
    }

    if is_interactive {
        // NOTE: Uses the new interactive tree selector.
        commons::choose_parent_interactive(&index, cancellation_token)
    } else {
        println!("No parent specified. Linking to 'global' project.");
        Ok(index_manager::GLOBAL_PROJECT_UUID)
    }
}

fn resolve_project_version(args: &InitArgs, is_interactive: bool) -> Result<String> {
    if let Some(version) = &args.version {
        return Ok(version.clone());
    }
    if is_interactive {
        match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Version")
            .default("0.1.0".to_string())
            .interact_text()
            .map_err(|e| anyhow!(e)) {
                    Ok(i) => Ok(i),
                    Err(e) => return Err(e.into()),
                }
        
    } else {
        Ok("0.1.0".to_string())
    }
}

fn resolve_project_description(
    args: &InitArgs,
    name: &str,
    is_interactive: bool,
) -> Result<String> {
    if let Some(desc) = &args.description {
        return Ok(desc.clone());
    }
    let default_desc = format!("A new project named '{}', managed by `axes`.", name);
    if is_interactive {
        match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Description")
            .default(default_desc)
            .interact_text()
            .map_err(|e| anyhow!(e)) {
                    Ok(i) => Ok(i),
                    Err(e) => return Err(e.into()),
                }
    } else {
        Ok(default_desc)
    }
}

/// Parses a vector of "KEY=VALUE" strings into a HashMap.
fn parse_key_value_pairs(pairs: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for pair in pairs {
        match pair.split_once('=') {
            Some((key, value)) => {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
            None => {
                return Err(anyhow!(
                    "Invalid format for key-value pair: '{}'. Expected 'KEY=VALUE'.",
                    pair
                ));
            }
        }
    }
    Ok(map)
}
