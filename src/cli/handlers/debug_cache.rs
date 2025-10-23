//! # Handler for the `cache` command
//!
//! This module provides the logic for the internal `axes cache` command. It is primarily
//! a debugging tool intended for developers of `axes` to inspect and manage the binary
//! configuration cache for projects.
//!
//! ## Features
//!
//! - **Inspect**: Deserializes a project's cached configuration (`.bin` file) and prints
//!   it to the console as human-readable JSON. This is useful for verifying the output
//!   of the configuration compiler.
//! - **Clear**: Deletes a project's cached configuration file from the filesystem and also
//!   removes the `config_hash` and `cache_dir` metadata from the project's entry in the
//!   global index. This forces a full recompilation of the project's configuration on the
//!   next run.

use crate::{
    models::{CachedProjectConfig, GlobalIndex},
    state::AppStateGuard,
};
use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use colored::*;
use std::fs;

// --- Command Argument Parsing ---

/// (Internal) Inspect or clear the configuration cache for a project.
#[derive(Parser, Debug)]
#[command(no_binary_name = true, hide = true)]
struct CacheArgs {
    /// The project context to inspect the cache for.
    context: Option<String>,
    #[command(subcommand)]
    command: CacheSubcommand,
}

#[derive(Subcommand, Debug)]
enum CacheSubcommand {
    /// Deserializes and prints the content of a project's config cache.
    Inspect,
    /// Deletes the cache file for a project and clears its hash from the index,
    /// forcing a full regeneration on the next run.
    Clear,
}

// --- Main Handler ---

/// The main handler for the `cache` command.
///
/// It parses the subcommand (`inspect` or `clear`) and the target project context,
/// then dispatches to the appropriate logic function.
///
/// # Arguments
/// * `context` - The project context, if provided by the dispatcher's universal grammar.
/// * `args` - The command-specific arguments (e.g., `<context> inspect`).
/// * `state_guard` - A mutable guard to the application state.
pub fn handle(
    context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard<'_>,
) -> Result<()> {
    let cache_args = CacheArgs::try_parse_from(&args)?;

    let final_context = cache_args
        .context
        .or(context)
        .ok_or_else(|| anyhow!("The '_cache' command requires an explicit project context."))?;
    let (uuid, qualified_name) =
        crate::core::context_resolver::resolve_context(&final_context, state_guard)?;
    match cache_args.command {
        CacheSubcommand::Inspect => inspect_cache(uuid, &qualified_name, state_guard.index()),
        CacheSubcommand::Clear => clear_cache(uuid, &qualified_name, state_guard.index_mut()),
    }
}

// --- Subcommand Logic ---

/// Handles the logic for inspecting a project's cache file.
///
/// It resolves the path to the cache file using the metadata from the `GlobalIndex`,
/// reads the binary data, decompresses it, deserializes it using `bincode`, and then
/// pretty-prints it as JSON.
///
/// # Arguments
/// * `uuid` - The UUID of the project whose cache is to be inspected.
/// * `name` - The qualified name of the project, for display purposes.
/// * `index` - An immutable reference to the `GlobalIndex`.
fn inspect_cache(uuid: uuid::Uuid, name: &str, index: &GlobalIndex) -> Result<()> {
    println!(
        "\nInspecting cache for project '{}' ({})",
        name.cyan(),
        uuid
    );

    let project = index.projects.get(&uuid).unwrap();

    // Use a single `if let` to check for both required fields.
    if let (Some(cache_dir), Some(cache_hash)) = (&project.cache_dir, &project.config_hash) {
        let cache_path = cache_dir.join(cache_hash);
        println!(
            "  {:<15} {}",
            "Index Cache Dir:".blue(),
            cache_dir.display()
        );
        println!("  {:<15} {}", "Index Hash:".blue(), cache_hash);
        println!("  {:<15} {}", "Resolved Path:".blue(), cache_path.display());

        match fs::read(&cache_path) {
            Ok(bytes) => {
                if bytes.is_empty() {
                    println!("{}", "\nStatus: Cache file is empty.".yellow());
                    return Ok(());
                }

                // CLARITY: Wrap deserialization in a context block.
                let cache_data: CachedProjectConfig = bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                    .map(|(data, _)| data) // Discard the size
                    .context("Failed to deserialize cache file. It might be corrupt or from an incompatible version.")?;

                let json_output = serde_json::to_string_pretty(&cache_data)
                    .context("Failed to serialize cache data to JSON for display.")?;

                println!("\n--- {} ---", "Cache Content (as JSON)".green());
                println!("{}", json_output);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!(
                    "{}",
                    "\nStatus: Cache file does not exist at the resolved path.".red()
                );
            }
            Err(e) => {
                // Handle other potential I/O errors (e.g., permissions).
                return Err(e).with_context(|| {
                    format!("Failed to read cache file at {}", cache_path.display())
                });
            }
        }
    } else {
        // ROBUSTNESS: Provide more detailed information about what's missing.
        println!(
            "{}",
            "\nStatus: Project has incomplete cache information in the index.".yellow()
        );
        if project.cache_dir.is_none() {
            println!("  - Missing: `cache_dir`");
        }
        if project.config_hash.is_none() {
            println!("  - Missing: `config_hash` (project has likely never been cached)");
        }
    }

    Ok(())
}

/// Handles the logic for clearing a project's cache.
///
/// This is a mutable operation that performs two actions:
/// 1.  It attempts to delete the binary cache file from the filesystem.
/// 2.  It removes the `config_hash` and `cache_dir` fields from the project's
///     `IndexEntry` in the `GlobalIndex`.
///
/// # Arguments
/// * `uuid` - The UUID of the project whose cache is to be cleared.
/// * `name` - The qualified name of the project, for display purposes.
/// * `index` - A mutable reference to the `GlobalIndex`.
fn clear_cache(uuid: uuid::Uuid, name: &str, index: &mut GlobalIndex) -> Result<()> {
    println!("\nClearing cache for project '{}' ({})", name.cyan(), uuid);

    let project = index.projects.get_mut(&uuid).unwrap();

    let mut actions_performed = false;

    // --- File System Cleanup ---
    if let (Some(cache_dir), Some(cache_hash)) = (&project.cache_dir, &project.config_hash) {
        let cache_path = cache_dir.join(cache_hash);
        log::debug!(
            "Attempting to delete cache file at: {}",
            cache_path.display()
        );

        match fs::remove_file(&cache_path) {
            Ok(()) => {
                println!(
                    "  {:<15} {}",
                    "File System:".blue(),
                    "Cache file deleted.".green()
                );
                actions_performed = true;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!(
                    "  {:<15} {}",
                    "File System:".blue(),
                    "Cache file not found, nothing to delete.".yellow()
                );
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!("Failed to delete cache file at {}", cache_path.display())
                });
            }
        }
    } else {
        println!(
            "  {:<15} {}",
            "File System:".blue(),
            "No cache path in index, skipping file deletion.".yellow()
        );
    }

    // --- Index State Cleanup ---
    // Use `take()` for a more idiomatic way to consume and clear Option fields.
    if project.config_hash.take().is_some() | project.cache_dir.take().is_some() {
        println!(
            "  {:<15} {}",
            "Index State:".blue(),
            "Cache hash and directory cleared.".green()
        );
        actions_performed = true;
    }

    if !actions_performed {
        println!(
            "\n{}",
            "No cache information was found for this project. Nothing to do.".yellow()
        );
    } else {
        println!(
            "\n{}",
            "Successfully cleared cache. It will be regenerated on the next run.".bold()
        );
    }

    Ok(())
}
