// EN: src/cli/handlers/init.rs (CORRECTED AND UPDATED)

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use dialoguer::{self, Error as DialoguerError, Input, theme::ColorfulTheme};
use std::{collections::HashMap, env, fs, io, path::Path};
use uuid::Uuid;

use super::commons;
use crate::{
    constants::{AXES_DIR, PROJECT_CONFIG_FILENAME, PROJECT_REF_FILENAME},
    core::{context_resolver, index_manager},
    models::{GlobalIndex, ProjectConfig, ProjectRef},
};

use colored::Colorize;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
pub struct InitArgs {
    /// The name for the new project. If not provided, will be asked interactively.
    pub name: Option<String>,
    /// The context of the parent project. Defaults to 'global'.
    #[arg(long)]
    pub parent: Option<String>,
    /// The version of the project.
    #[arg(long)]
    pub version: Option<String>,
    /// A short description of the project.
    #[arg(long)]
    pub description: Option<String>,
    /// Do not ask for user input, use defaults for unspecified values.
    #[arg(long)]
    pub autosolve: bool,
    /// Set environment variables for the project (e.g., "KEY=VALUE").
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    pub env: Vec<String>,
    /// Set interpolation variables for the project (e.g., "KEY=VALUE").
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    pub var: Vec<String>,
}

/// The main handler for the `init` command.
/// Allows creating and registering new projects to axes.
pub fn handle(_context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
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

    // 2. Resolve configuration details, passing the mutable index down.
    let project_name = resolve_project_name(&init_args, &target_dir, is_interactive)?;

    let parent_uuid = resolve_parent_project(&init_args, is_interactive, index)?;

    let version = resolve_project_version(&init_args, is_interactive)?;

    let description = resolve_project_description(&init_args, &project_name, is_interactive)?;

    // 3. Build the project configuration object
    let mut project_config = ProjectConfig::new_for_init(&project_name, &version, &description);

    let env_vars = parse_key_value_pairs(&init_args.env)?;
    project_config.env.extend(env_vars);

    let vars = parse_key_value_pairs(&init_args.var)?;
    project_config.vars.extend(vars);

    // 4. Perform filesystem and index operations
    // FIX: Use the passed `index` reference, do not reload it.
    let (new_uuid, _) = index_manager::add_project_to_index(
        index,
        project_name.clone(),
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

    // 4.3. The updated global index will be saved by `main` at the end of execution.
    // No need to save it here explicitly.

    println!("\n{}", t!("common.success"));
    println!(
        "  Project '{}' created with UUID: {}",
        project_name, new_uuid
    );
    println!("  Configuration created at: {}", config_path.display());
    println!("  Successfully registered in global index.");

    Ok(())
}

// --- Auxiliary Functions ---

fn resolve_project_name(
    args: &InitArgs,
    target_dir: &Path,
    is_interactive: bool,
) -> Result<String> {
    if let Some(name) = &args.name {
        return commons::validate_project_name(name);
    }

    let default_name = target_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    if is_interactive {
        loop {
            let input = match Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Project name")
                .default(default_name.clone())
                .interact_text()
            {
                Ok(value) => value,
                Err(DialoguerError::IO(io_err)) if io_err.kind() == io::ErrorKind::Interrupted => {
                    return Err(anyhow!(t!("common.error.operation_cancelled")));
                }
                Err(e) => return Err(e.into()),
            };

            match commons::validate_project_name(&input) {
                Ok(name) => return Ok(name),
                Err(e) => {
                    println!("{}", format!("Error: {}", e).red());
                    continue;
                }
            }
        }
    } else {
        commons::validate_project_name(&default_name)
    }
}

// FIX: This function now takes and passes `&mut GlobalIndex`.
fn resolve_parent_project(
    args: &InitArgs,
    is_interactive: bool,
    index: &mut GlobalIndex,
) -> Result<Uuid> {
    if let Some(parent_context) = &args.parent {
        println!("Resolving parent '{}'...", parent_context);
        // `resolve_context` needs a mutable index to update `last_used` caches.
        let (uuid, qualified_name) = context_resolver::resolve_context(parent_context, index)?;
        println!(
            "Parent project '{}' found (UUID: {}).",
            qualified_name, uuid
        );
        return Ok(uuid);
    }

    if is_interactive {
        // `choose_parent_interactive` also needs the index. Let's make it immutable for read-only ops.
        commons::choose_parent_interactive(index)
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
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Version")
            .default("0.1.0".to_string())
            .interact_text()
            .map_err(|e| anyhow!(e))
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
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Description")
            .default(default_desc)
            .interact_text()
            .map_err(|e| anyhow!(e))
    } else {
        Ok(default_desc)
    }
}

fn parse_key_value_pairs(pairs: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for pair in pairs {
        if let Some((key, value)) = pair.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        } else {
            return Err(anyhow!(
                "Invalid format for key-value pair: '{}'. Expected 'KEY=VALUE'.",
                pair
            ));
        }
    }
    Ok(map)
}
