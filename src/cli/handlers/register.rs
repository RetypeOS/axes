use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;
use std::{env, path::PathBuf};

use crate::{
    core::onboarding_manager::{self, OnboardingOptions},
    state::AppStateGuard,
};

// --- Command Argument Parsing ---
#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Registers an existing axes project (and its children) into the global index."
)]
pub struct RegisterArgs {
    /// The path to the project to register. Defaults to the current directory.
    pub path: Option<String>,
    /// The context of the project that should become the parent of the new project.
    #[arg(long)]
    pub parent: Option<String>,
    /// Do not ask for user input; fail on any conflict.
    #[arg(long)]
    pub autosolve: bool,
}

// --- Main Handler ---

pub fn handle(
    context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard,
) -> Result<()> {
    if env::var("AXES_PROJECT_UUID").is_ok() {
        return Err(anyhow!(t!("register.error.in_session")));
    }

    let register_args = RegisterArgs::try_parse_from(&args)?;

    // Determine the initial path to scan from.
    let path_arg = register_args.path.or(context);
    let initial_path_unresolved = match path_arg {
        Some(p) => PathBuf::from(p),
        None => env::current_dir()?,
    };

    // This prevents ambiguity with relative paths like `.` or `../`.
    let initial_path = dunce::canonicalize(&initial_path_unresolved).with_context(|| {
        format!(
            "Failed to resolve path: {}",
            initial_path_unresolved.display()
        )
    })?;

    let suggested_parent_uuid = if let Some(parent_context) = &register_args.parent {
        let (uuid, name) =
            crate::core::context_resolver::resolve_context(parent_context, state_guard)?;
        println!("Using '{}' as the suggested parent.", name.cyan());
        Some(uuid)
    } else {
        None
    };

    let options = OnboardingOptions {
        autosolve: register_args.autosolve,
        suggested_parent_uuid,
    };

    onboarding_manager::register_project(&initial_path, state_guard, &options)
        .with_context(|| format!(t!("register.error.failed"), path = initial_path.display()))?;

    Ok(())
}
