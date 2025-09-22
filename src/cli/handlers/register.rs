// EN: src/cli/handlers/register.rs

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::{env, path::PathBuf};

use crate::{
    cli::args::RegisterArgs,
    core::{
        graph_display::{self, DisplayOptions},
        index_manager,
        onboarding_manager::{self, OnboardingOptions},
    }, CancellationToken,
};

pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
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

    // Robustness #2: Canonicalize and clean the path from the start.
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

    let mut index = index_manager::load_and_ensure_global_project()?;

    // Robustness #1: Keep a copy of the index *before* modification to compare.
    let index_before = index.clone();

    let options = OnboardingOptions {
        autosolve: register_args.autosolve,
        suggested_parent_uuid: None,
    };

    onboarding_manager::register_project(&path_to_register, &mut index, &options, cancellation_token).with_context(
        || {
            anyhow!(
                t!("register.error.failed"),
                path = path_to_register.display()
            )
        },
    )?;

    // Save changes to disk
    index_manager::save_global_index(&index)?;

    // Robustness #4: Provide a meaningful summary of the operation.
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

            // NOTE: CORRECTION. Provide a default DisplayOptions.
            // We don't need to show full details here, so default options are fine.
            let display_options = DisplayOptions {
                show_paths: false,
                show_uuids: false,
            };
            graph_display::display_project_tree(&index, Some(*main_uuid), &display_options);
        }
    } else {
        println!("\nNo new projects were registered. The project may have already been indexed.");
    }

    Ok(())
}
