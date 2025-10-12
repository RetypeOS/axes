// src/cli/handlers/repair.rs

use anyhow::{Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::{env, path::PathBuf};
use walkdir::WalkDir;

use crate::{core::index_manager, models::GlobalIndex};

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Scans the filesystem to find and fix inconsistencies in the axes index."
)]
pub struct RepairArgs {
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

struct PathMismatch {
    uuid: uuid::Uuid,
    name: String,
    old_path: PathBuf,
    new_path: PathBuf,
}

// --- Main Handler ---

pub fn handle(_context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
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
    let mismatches = scan_for_path_mismatches(&start_path, &repair_args, index)?;

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
            if let Some(entry) = index.projects.get_mut(&mismatch.uuid) {
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
