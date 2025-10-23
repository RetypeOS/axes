//! # Handler for the `register` command
//!
//! This module provides the logic for the `axes register` command, which discovers existing
//! `axes` projects on the filesystem (those with an `axes.toml` file) and adds them to the
//! global index.
//!
//! ## Core Logic
//!
//! This handler acts as a high-level orchestrator for the more complex logic contained
//! within the `onboarding_manager` module. Its primary responsibilities are:
//!
//! 1.  **Argument Parsing**: It parses arguments like the target `path`, a suggested `--parent`,
//!     and the `--autosolve` flag for non-interactive mode.
//! 2.  **Path Resolution**: It determines the absolute, canonical path to begin the discovery scan,
//!     defaulting to the current working directory if no path is provided.
//! 3.  **Options Assembly**: It constructs an `OnboardingOptions` struct based on the parsed
//!     arguments to configure the behavior of the onboarding process.
//! 4.  **Delegation**: It delegates the entire complex workflow of discovery, conflict resolution,
//!     user interaction, and final registration to the `onboarding_manager::register_project` function.

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
struct RegisterArgs {
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

/// The main handler for the `register` command.
///
/// It sets up the necessary options and initial path, then delegates the complex
/// registration logic to the `onboarding_manager`.
///
/// # Arguments
/// * `context` - The project context, which can be used as the path to register.
/// * `args` - Command-specific arguments (e.g., `--parent <parent_context>`).
/// * `state_guard` - A mutable guard to the application state, which will be modified
///   by the registration process.
pub fn handle(
    context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard<'_>,
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
