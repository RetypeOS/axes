// EN: src/cli/handlers/init.rs (REBUILT FOR CLARITY AND UX)

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::{Error as DialoguerError, Input, theme::ColorfulTheme};
use std::{collections::HashMap, env, fs, io, path::Path};
use uuid::Uuid;

use super::commons;
use crate::{
    constants::{AXES_DIR, PROJECT_CONFIG_FILENAME, PROJECT_REF_FILENAME},
    core::{context_resolver, index_manager},
    models::{GlobalIndex, ProjectConfig, ProjectRef},
};

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Initializes a new axes project in the current directory."
)]
pub struct InitArgs {
    /// The name for the new project. If not provided, will be asked interactively.
    pub name: Option<String>,
    /// The context of the parent project. Defaults to 'global'.
    #[arg(long)]
    pub parent: Option<String>,
    /// The version of the project.
    #[arg(long, short)]
    pub version: Option<String>,
    /// A short description of the project.
    #[arg(long, short)]
    pub description: Option<String>,
    /// Do not ask for user input, use defaults for unspecified values.
    #[arg(long)]
    pub autosolve: bool,
    /// Set environment variables (e.g., --env KEY1=VAL1,KEY2=VAL2).
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    pub env: Vec<String>,
    /// Set interpolation variables (e.g., --var KEY1=VAL1,KEY2=VAL2).
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    pub var: Vec<String>,
}

/// A container for all details needed to create a new project.
struct ProjectDetails {
    name: String,
    parent_uuid: Uuid,
    version: String,
    description: String,
    env: HashMap<String, String>,
    vars: HashMap<String, String>,
}

// --- Main Handler ---

pub fn handle(_context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    let init_args = InitArgs::try_parse_from(&args)?;
    let target_dir = env::current_dir()?;

    println!(
        "\n{}",
        format!("Initializing axes project in {}", target_dir.display()).bold()
    );

    // 1. Pre-flight check: ensure the directory is not already an axes project.
    let axes_dir = target_dir.join(AXES_DIR);
    if axes_dir.exists() {
        return Err(anyhow!(t!("init.error.already_exists"), dir = AXES_DIR));
    }

    // 2. Gather all project details, interactively or from args.
    let details = gather_project_details(init_args, &target_dir, index)?;

    // 3. Create the configuration object.
    let mut project_config =
        ProjectConfig::new_for_init(&details.name, &details.version, &details.description);
    project_config.env.extend(details.env);
    project_config.vars.extend(details.vars);

    // 4. Perform filesystem and index operations.
    let (new_uuid, _) = index_manager::add_project_to_index(
        index,
        details.name.clone(),
        target_dir.clone(),
        Some(details.parent_uuid),
    )
    .with_context(|| t!("init.error.add_to_index"))?;

    fs::create_dir_all(&axes_dir)?;

    let config_path = axes_dir.join(PROJECT_CONFIG_FILENAME);
    let toml_string = toml::to_string_pretty(&project_config)?;
    fs::write(&config_path, toml_string)?;

    let project_ref = ProjectRef {
        self_uuid: new_uuid,
        parent_uuid: Some(details.parent_uuid),
        name: details.name.clone(),
    };
    index_manager::write_project_ref(&target_dir, &project_ref)
        .with_context(|| format!(t!("init.error.write_ref"), file = PROJECT_REF_FILENAME))?;

    // `main` will handle saving the updated index.

    // 5. Final summary feedback.
    println!("\n{}", t!("common.success").green().bold());
    println!("  {:<15} {}", "Project Name:".blue(), details.name);
    println!("  {:<15} {}", "UUID:".blue(), new_uuid);
    println!(
        "  {:<15} {}",
        "Parent:".blue(),
        index_manager::build_qualified_name(details.parent_uuid, index).unwrap_or_default()
    );
    println!("  {:<15} {}", "Config file:".blue(), config_path.display());
    println!("\n{}", t!("init.success.next_steps").dimmed());

    Ok(())
}

// --- Helper Functions ---

/// Orchestrates the collection of all project details, handling interactive and non-interactive modes.
fn gather_project_details(
    args: InitArgs,
    target_dir: &Path,
    index: &mut GlobalIndex,
) -> Result<ProjectDetails> {
    let is_interactive = !args.autosolve;

    let name = resolve_project_name(&args.name, target_dir, is_interactive)?;
    let parent_uuid = resolve_parent_project(&args.parent, is_interactive, index)?;
    let version = resolve_project_version(&args.version, is_interactive)?;
    let description = resolve_project_description(&args.description, &name, is_interactive)?;
    let env = parse_key_value_pairs(&args.env)?;
    let vars = parse_key_value_pairs(&args.var)?;

    Ok(ProjectDetails {
        name,
        parent_uuid,
        version,
        description,
        env,
        vars,
    })
}

fn resolve_project_name(
    name_arg: &Option<String>,
    target_dir: &Path,
    is_interactive: bool,
) -> Result<String> {
    if let Some(name) = name_arg {
        println!("  {} {}", "Project Name:".dimmed(), name);
        return commons::validate_project_name(name);
    }

    let default_name = target_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    if is_interactive {
        loop {
            let prompt = t!("init.prompt.name");
            let input = match Input::with_theme(&ColorfulTheme::default())
                .with_prompt(prompt)
                .default(default_name.clone())
                .interact_text()
            {
                Ok(value) => value,
                Err(DialoguerError::IO(e)) if e.kind() == io::ErrorKind::Interrupted => {
                    return Err(anyhow!(t!("common.error.operation_cancelled")));
                }
                Err(e) => return Err(e.into()),
            };
            match commons::validate_project_name(&input) {
                Ok(name) => return Ok(name),
                Err(e) => println!("{}", format!("  Error: {}", e).red()),
            }
        }
    } else {
        println!(
            "  {} {} {}",
            "Project Name:".dimmed(),
            default_name,
            t!("common.label.default").dimmed()
        );
        commons::validate_project_name(&default_name)
    }
}

fn resolve_parent_project(
    parent_arg: &Option<String>,
    is_interactive: bool,
    index: &mut GlobalIndex,
) -> Result<Uuid> {
    if let Some(parent_context) = parent_arg {
        let (uuid, qualified_name) = context_resolver::resolve_context(parent_context, index)?;
        println!("  {} {}", "Parent Project:".dimmed(), qualified_name);
        return Ok(uuid);
    }

    if is_interactive {
        return commons::choose_parent_interactive(index);
    }

    println!(
        "  {} global {}",
        "Parent Project:".dimmed(),
        t!("common.label.default").dimmed()
    );
    Ok(index_manager::GLOBAL_PROJECT_UUID)
}

fn resolve_project_version(version_arg: &Option<String>, is_interactive: bool) -> Result<String> {
    if let Some(version) = version_arg {
        println!("  {} {}", "Version:".dimmed(), version);
        return Ok(version.clone());
    }
    if is_interactive {
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt(t!("init.prompt.version"))
            .default("0.1.0".to_string())
            .interact_text()
            .map_err(|e| anyhow!(e))
    } else {
        println!(
            "  {} 0.1.0 {}",
            "Version:".dimmed(),
            t!("common.label.default").dimmed()
        );
        Ok("0.1.0".to_string())
    }
}

fn resolve_project_description(
    desc_arg: &Option<String>,
    name: &str,
    is_interactive: bool,
) -> Result<String> {
    if let Some(desc) = desc_arg {
        println!("  {} {}", "Description:".dimmed(), desc);
        return Ok(desc.clone());
    }
    let default_desc = format!("A new project named '{}', managed by `axes`.", name);
    if is_interactive {
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt(t!("init.prompt.description"))
            .default(default_desc)
            .interact_text()
            .map_err(|e| anyhow!(e))
    } else {
        println!("  {} {}", "Description:".dimmed(), default_desc);
        Ok(default_desc)
    }
}

fn parse_key_value_pairs(pairs: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for pair in pairs {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| anyhow!(t!("init.error.invalid_kv_pair"), pair = pair))?;
        map.insert(key.trim().to_string(), value.trim().to_string());
    }
    Ok(map)
}
