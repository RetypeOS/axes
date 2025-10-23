//! # Handler for the `repair` command
//!
//! This module provides the logic for the `axes repair` command, a diagnostic and fixing tool
//! designed to resolve inconsistencies between the central `GlobalIndex` and the state of
//! projects on the filesystem.
//!
//! ## Core Logic
//!
//! The command operates in distinct phases:
//!
//! 1.  **Scan & Detect**: The `scan_for_path_mismatches` function traverses a specified
//!     directory structure (recursively or not) looking for `.axes/project_ref.bin` files.
//!     For each one found, it compares the project's UUID and path with the information
//!     stored in the `GlobalIndex`. Currently, it detects:
//!     -   **Path Mismatches**: A project registered in the index with `path A`, but its
//!         `project_ref.bin` is found at `path B` (indicating the project directory was moved).
//!
//! 2.  **Report**: If any inconsistencies are found, they are presented to the user in a
//!     clear, color-coded format, showing the "registered" state vs. the "found" state.
//!
//! 3.  **Fix (Optional)**: If the user runs the command with the `--fix` flag and confirms
//!     the interactive prompt, the handler mutates the `GlobalIndex` to correct the detected
//!     discrepancies. For example, it will update the `path` field of an `IndexEntry` to
//!     point to the project's new location.

use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::{env, path::PathBuf};
use walkdir::WalkDir;

use crate::{core::index_manager, models::GlobalIndex, state::AppStateGuard};

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Scans the filesystem to find and fix inconsistencies in the axes index."
)]
struct RepairArgs {
    /// The starting path for the scan. Defaults to the current directory.
    path: Option<String>,

    /// Recursively scan subdirectories.
    #[arg(long, short)]
    recursive: bool,

    /// Maximum depth for recursive scanning. Requires --recursive.
    #[arg(long, short, requires = "recursive")]
    depth: Option<usize>,

    /// Apply the found fixes to the global index after confirmation.
    #[arg(long)]
    fix: bool,
}

// --- Data Structures for Reporting ---

/// A local struct used to report a discrepancy between the index and the filesystem.
struct PathMismatch {
    /// The UUID of the project.
    uuid: uuid::Uuid,
    /// The name of the project.
    name: String,
    /// The path currently registered in the `GlobalIndex`.
    old_path: PathBuf,
    /// The new path where the project was found on the filesystem.
    new_path: PathBuf,
}

// --- Main Handler ---

/// The main handler for the `repair` command.
///
/// It orchestrates the scan, report, and optional fix phases of the repair process.
///
/// # Arguments
/// * `_context` - The context from the dispatcher, which is ignored by this global command.
/// * `args` - Command-specific arguments (e.g., `--recursive`, `--fix`).
/// * `state_guard` - A mutable guard to the application state.
pub fn handle(
    _context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard<'_>,
) -> Result<()> {
    if env::var("AXES_PROJECT_UUID").is_ok() {
        return Err(anyhow!(t!("repair.error.in_session")));
    }

    let repair_args = RepairArgs::try_parse_from(&args)?;
    let start_path = match repair_args.path {
        Some(ref p) => PathBuf::from(p),
        None => env::current_dir()?,
    };

    println!(
        "\n{}",
        format!(t!("repair.info.starting_scan"), path = start_path.display()).bold()
    );

    // --- Phase 1: Scan and Detect ---
    let mismatches = scan_for_path_mismatches(&start_path, &repair_args, state_guard.index())?;

    // --- Phase 2: Report and (Optionally) Fix ---
    if mismatches.is_empty() {
        println!("{}", t!("repair.success.no_issues").green());
        // TODO: Add other repair checks here in the future (e.g., TOML validation).
        return Ok(());
    }

    println!(
        "\n{}",
        format!(t!("repair.warning.found_issues"), count = mismatches.len()).yellow()
    );
    for mismatch in &mismatches {
        println!(
            t!("repair.info.mismatch_header"),
            name = mismatch.name.cyan(),
            uuid = mismatch.uuid
        );
        println!(
            "  - {}: {}",
            t!("repair.label.registered_path"),
            mismatch.old_path.display().to_string().red()
        );
        println!(
            "  - {}: {}",
            t!("repair.label.found_at"),
            mismatch.new_path.display().to_string().green()
        );
    }

    if repair_args.fix {
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(t!("repair.prompt.apply_fixes"))
            .default(true)
            .interact()?
        {
            println!("\n{}", t!("common.info.operation_cancelled"));
            return Ok(());
        }

        // --- Phase 3: Apply Fixes ---
        let mut fixed_count = 0;
        for mismatch in mismatches {
            if let Some(entry) = state_guard.index_mut().projects.get_mut(&mismatch.uuid) {
                entry.path = mismatch.new_path;
                fixed_count += 1;
            }
        }
        println!(
            "\n{}",
            format!(t!("repair.success.fixed"), count = fixed_count).green()
        );
    } else {
        println!("\n{}", t!("repair.info.how_to_fix").dimmed());
    }

    Ok(())
}

// --- Helper Functions ---

/// Scans the filesystem starting from a given path to find projects and identify
/// path mismatches against the global index.
///
/// It uses `walkdir` to efficiently traverse the directory tree, reading `project_ref.bin`
/// files to identify projects.
///
/// # Arguments
/// * `start_path` - The root directory for the scan.
/// * `args` - The parsed command arguments, used to control recursion and depth.
/// * `index` - An immutable reference to the `GlobalIndex` to compare against.
fn scan_for_path_mismatches(
    start_path: &PathBuf,
    args: &RepairArgs,
    index: &GlobalIndex,
) -> Result<Vec<PathMismatch>> {
    let mut mismatches = Vec::new();
    let mut walker = WalkDir::new(start_path);

    if args.recursive {
        if let Some(d) = args.depth {
            walker = walker.max_depth(d);
        }
    } else {
        // If not recursive, only scan the start_path itself.
        walker = walker.max_depth(1);
    }

    for entry in walker.into_iter().filter_map(Result::ok) {
        let path = entry.path();
        let ref_path = path.join(".axes").join("project_ref.bin");

        if ref_path.is_file() {
            match index_manager::read_project_ref(path) {
                Ok(proj_ref) => {
                    if let Some(index_entry) = index.projects.get(&proj_ref.self_uuid) {
                        // Project found in index, check if the path is correct.
                        if index_entry.path != path {
                            mismatches.push(PathMismatch {
                                uuid: proj_ref.self_uuid,
                                name: proj_ref.name,
                                old_path: index_entry.path.clone(),
                                new_path: path.to_path_buf(),
                            });
                        }
                    }
                    // TODO: Handle case where project ref exists but UUID is NOT in index (orphaned project).
                }
                Err(e) => {
                    // Log or report corrupted project_ref.bin files.
                    println!(
                        "{}",
                        format!(
                            t!("repair.warning.corrupt_ref"),
                            path = ref_path.display(),
                            error = e
                        )
                        .yellow()
                    );
                }
            }
            // TODO: Add TOML validation logic here.
        }
    }
    Ok(mismatches)
}
