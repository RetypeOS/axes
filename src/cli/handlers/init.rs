//! # Handler for the `init` command
//!
//! This module provides the logic for the `axes init` (or `axes new`) command, which creates
//! a new `axes` project in the current directory. It handles both interactive setup and
//! non-interactive (automated) project creation via command-line flags.
//!
//! ## Core Logic
//!
//! 1.  **Argument Parsing**: Parses a rich set of arguments (`--parent`, `--version`, `--env`, etc.)
//!     to allow for detailed project configuration directly from the command line.
//! 2.  **Pre-flight Checks**: Before creating any files, it verifies that the target directory
//!     is not already an `axes` project.
//! 3.  **Detail Gathering**: The `gather_project_details` orchestrator collects all necessary
//!     information, either from the parsed arguments (in `--autosolve` mode) or by prompting
//!     the user interactively for things like the project name and parent.
//! 4.  **Collision Detection**: It performs a crucial check to ensure the chosen project name
//!     does not conflict with an existing sibling under the chosen parent.
//! 5.  **File & Index Creation**: If all checks pass, it performs the necessary mutations:
//!     a. Adds the new project to the `GlobalIndex`.
//!     b. Creates the `.axes` directory.
//!     c. Generates and writes a default `axes.toml` file.
//!     d. Generates and writes the local `project_ref.bin` identity file.
//! 6.  **User Feedback**: It concludes by printing a clear summary of the newly created project.

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
    models::{ProjectConfig, ProjectRef},
    state::AppStateGuard,
};

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Initializes a new axes project in the current directory."
)]
struct InitArgs {
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

/// A container for all details needed to create a new project, gathered from arguments
/// or interactive prompts.
struct ProjectDetails {
    /// The validated name of the new project.
    name: String,
    /// The UUID of the chosen parent project.
    parent_uuid: Uuid,
    /// The project's version string.
    version: String,
    /// A short description of the project.
    description: String,
    /// A map of environment variables to include in `axes.toml`.
    env: HashMap<String, String>,
    /// A map of interpolation variables to include in `axes.toml`.
    vars: HashMap<String, String>,
}

// --- Main Handler ---

/// The main handler for the `init` command.
///
/// It orchestrates the entire project creation workflow, from gathering details to
/// modifying the filesystem and updating the global index.
///
/// # Arguments
/// * `_context` - The context from the dispatcher, which is ignored by this command.
/// * `args` - The command-specific arguments provided by the user.
/// * `state_guard` - A mutable guard to the application state.
pub fn handle(
    _context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard<'_>,
) -> Result<()> {
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
    let details = gather_project_details(init_args, &target_dir, state_guard)?;

    // 3. Create the configuration object.
    let mut project_config =
        ProjectConfig::new_for_init(&details.name, &details.version, &details.description);
    project_config.env.extend(details.env);
    for (key, value) in details.vars {
        project_config
            .vars
            .insert(key, crate::models::TomlVar::Simple(value));
    }

    // 4. Perform filesystem and index operations.
    let (new_uuid, _) = index_manager::add_project_to_index(
        state_guard.index_mut(),
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
        index_manager::build_qualified_name(details.parent_uuid, state_guard.index())
            .unwrap_or_default()
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
    state_guard: &mut AppStateGuard<'_>,
) -> Result<ProjectDetails> {
    let is_interactive = !args.autosolve;

    // --- Resolve name and parent first for early validation ---
    let name = resolve_project_name(&args.name, target_dir, is_interactive)?;
    let parent_uuid = resolve_parent_project(&args.parent, is_interactive, state_guard)?;

    // --- Early collision check ---
    if index_manager::is_sibling_name_taken(state_guard.index(), parent_uuid, &name, None) {
        return Err(anyhow!(t!("init.error.name_collision"), name = name));
    }

    // --- Proceed with other details only after validation passes ---
    let version = resolve_string_value(
        &args.version,
        t!("init.prompt.version"),
        "0.1.0".to_string(),
        is_interactive,
    )?;
    let description = resolve_string_value(
        &args.description,
        t!("init.prompt.description"),
        format!("A new project named '{}', managed by `axes`.", name),
        is_interactive,
    )?;
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
    state_guard: &mut AppStateGuard<'_>,
) -> Result<Uuid> {
    if let Some(parent_context) = parent_arg {
        let (uuid, qualified_name) =
            context_resolver::resolve_context(parent_context, state_guard)?;
        println!("  {} {}", "Parent Project:".dimmed(), qualified_name);
        return Ok(uuid);
    }

    if is_interactive {
        return commons::choose_parent_interactive(state_guard);
    }

    println!(
        "  {} global {}",
        "Parent Project:".dimmed(),
        t!("common.label.default").dimmed()
    );
    Ok(index_manager::GLOBAL_PROJECT_UUID)
}

/// A helper function to parse key-value pairs from the command line (e.g., "KEY=VALUE").
/// Used for parsing `--env` and `--var` arguments.
fn parse_key_value_pairs(pairs: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::with_capacity(pairs.len());
    for pair in pairs {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| anyhow!(t!("init.error.invalid_kv_pair"), pair = pair))?;

        // OPTIMIZATION: Use `to_owned()` which can be cheaper than `to_string()` for `&str`.
        // Also makes the ownership explicit.
        map.insert(key.trim().to_owned(), value.trim().to_owned());
    }
    Ok(map)
}

/// A generic helper to resolve a string value, either from a command-line argument
/// or by interactively prompting the user.
///
/// # Arguments
/// * `arg_val` - The value from the parsed `clap` arguments.
/// * `prompt` - The text to display to the user if interactive input is needed.
/// * `default_val` - The default value to use in both interactive and non-interactive modes.
/// * `is_interactive` - A boolean indicating whether to prompt the user.
fn resolve_string_value(
    arg_val: &Option<String>,
    prompt: &str,
    default_val: String,
    is_interactive: bool,
) -> Result<String> {
    if let Some(val) = arg_val {
        // We can make the output cleaner by specifying what we are setting.
        // Let's assume the prompt contains the "name" of the value.
        println!("  {} {}", format!("{}:", prompt).dimmed(), val);
        return Ok(val.clone());
    }

    if is_interactive {
        Input::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(default_val)
            .interact_text()
            .map_err(|e| anyhow!(e))
    } else {
        println!(
            "  {} {} {}",
            format!("{}:", prompt).dimmed(),
            default_val,
            t!("common.label.default").dimmed()
        );
        Ok(default_val)
    }
}
