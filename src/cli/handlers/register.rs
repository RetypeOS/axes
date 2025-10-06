// EN: src/cli/handlers/register.rs (CORRECTED AND UPDATED)

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::{env, path::PathBuf};

use crate::{
    core::{
        graph_display::{self, DisplayOptions},
        onboarding_manager::{self, OnboardingOptions},
    },
    models::GlobalIndex,
};

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
pub struct RegisterArgs {
    /// The path to the project to register. Defaults to the current directory.
    pub path: Option<String>,
    /// Do not ask for user input, fail on any conflict.
    #[arg(long)]
    pub autosolve: bool,
}

pub fn handle(_context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    if env::var("AXES_PROJECT_UUID").is_ok() {
        return Err(anyhow!(
            "'register' command is not available inside a project session."
        ));
    }

    let register_args = RegisterArgs::try_parse_from(&args)?;

    let initial_path = match register_args.path {
        Some(p) => PathBuf::from(p),
        None => env::current_dir()?,
    };

    let path_to_register = dunce::canonicalize(&initial_path).with_context(|| {
        format!(
            "Could not resolve the absolute path for '{}'",
            initial_path.display()
        )
    })?;

    if !path_to_register.exists() {
        return Err(anyhow!(
            "Specified path does not exist: {}",
            path_to_register.display()
        ));
    }

    // FIX: Use the passed `index` reference, do not reload it.
    // We can still clone it if we need to compare before/after states within this handler.
    let index_before = index.clone();

    let options = OnboardingOptions {
        autosolve: register_args.autosolve,
        suggested_parent_uuid: None,
    };

    // `register_project` will mutate the `index` passed to it.
    onboarding_manager::register_project(&path_to_register, index, &options).with_context(|| {
        anyhow!(
            t!("register.error.failed"),
            path = path_to_register.display()
        )
    })?;

    // FIX: The saving of the index is now handled by `main`. We remove the explicit save.
    // index_manager::save_global_index(&index)?;

    // The comparison logic to provide user feedback remains valid.
    let projects_registered_count = index.projects.len() - index_before.projects.len();
    if projects_registered_count > 0 {
        println!(
            "\n✔ {} project(s) successfully registered.",
            projects_registered_count
        );

        if let Some((main_uuid, _)) = index
            .projects
            .iter()
            .find(|(_, entry)| entry.path == path_to_register)
        {
            println!("\nProject structure registered:");
            
            let display_options = DisplayOptions {
                show_paths: false,
                show_uuids: false,
            };
            // `display_project_tree` only needs an immutable reference, which is fine.
            graph_display::display_project_tree(index, Some(*main_uuid), &display_options);
        }
    } else {
        println!("\nNo new projects were registered. The project may have already been indexed.");
    }

    Ok(())
}