// EN: src/cli/handlers/debug_cache.rs

use crate::models::{CachedProjectConfig, GlobalIndex};
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

/// The main handler for the `_cache` command.
/// Provides tools to debug the single-layer configuration caching system.
pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    let cache_args = CacheArgs::try_parse_from(&args)?;

    let final_context = cache_args
        .context
        .or(context)
        .ok_or_else(|| anyhow!("The '_cache' command requires an explicit project context."))?;

    let (uuid, qualified_name) =
        crate::core::context_resolver::resolve_context(&final_context, index)?;

    match cache_args.command {
        CacheSubcommand::Inspect => inspect_cache(uuid, &qualified_name, index),
        CacheSubcommand::Clear => clear_cache(uuid, &qualified_name, index),
    }
}

// --- Subcommand Logic ---

/// Handles the logic for inspecting a project's cache file.
fn inspect_cache(uuid: uuid::Uuid, name: &str, index: &GlobalIndex) -> Result<()> {
    println!(
        "\nInspecting cache for project '{}' ({})",
        name.cyan(),
        uuid
    );

    // Get the project entry from the index.
    let project = index.projects.get(&uuid).unwrap(); // Safe, resolve_context guarantees it exists.

    let cache_dir = match &project.cache_dir {
        Some(dir) => dir,
        None => {
            println!(
                "{}",
                "Status: Project has no resolved cache directory in the index.".yellow()
            );
            return Ok(());
        }
    };

    let cache_hash = match &project.config_hash {
        Some(hash) => hash,
        None => {
            println!(
                "{}",
                "Status: Project has no configuration hash in the index (never cached).".yellow()
            );
            return Ok(());
        }
    };

    let cache_path = cache_dir.join(cache_hash);
    println!("  {:<15} {}", "Cache Path:".blue(), cache_path.display());

    if !cache_path.exists() {
        println!(
            "{}",
            "\nStatus: Cache file does not exist at the specified path.".red()
        );
        return Ok(());
    }

    let bytes = fs::read(&cache_path)
        .with_context(|| format!("Failed to read cache file at {}", cache_path.display()))?;

    if bytes.is_empty() {
        println!("{}", "\nStatus: Cache file is empty.".yellow());
        return Ok(());
    }

    let (cache_data, _): (CachedProjectConfig, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
            .context("Failed to deserialize cache file. It might be corrupt.")?;

    let json_output = serde_json::to_string_pretty(&cache_data)
        .context("Failed to serialize cache data to JSON.")?;

    println!("\n--- {} ---", "Cache Content (as JSON)".green());
    println!("{}", json_output);
    Ok(())
}

/// Handles the logic for clearing a project's cache.
fn clear_cache(uuid: uuid::Uuid, name: &str, index: &mut GlobalIndex) -> Result<()> {
    println!("\nClearing cache for project '{}' ({})", name.cyan(), uuid);

    // We get a mutable reference to the project entry to modify it.
    let project = index.projects.get_mut(&uuid).unwrap();

    let mut file_deleted = false;

    // If a cache path can be constructed, try to delete the file.
    if let (Some(cache_dir), Some(cache_hash)) = (&project.cache_dir, &project.config_hash) {
        let cache_path = cache_dir.join(cache_hash);
        println!("  {:<15} {}", "Target Path:".blue(), cache_path.display());

        if cache_path.exists() {
            fs::remove_file(&cache_path)?;
            file_deleted = true;
            println!(
                "  {:<15} {}",
                "File System:".blue(),
                "Cache file deleted.".green()
            );
        } else {
            println!(
                "  {:<15} {}",
                "File System:".blue(),
                "Cache file not found, nothing to delete.".yellow()
            );
        }
    } else {
        println!(
            "  {:<15} {}",
            "File System:".blue(),
            "No cache path in index, skipping file deletion.".yellow()
        );
    }

    // Always clear the index entries to force regeneration.
    let hash_cleared = project.config_hash.is_some();
    project.config_hash = None;
    // Clearing the cache_dir is important to force re-resolution of the path itself.
    let dir_cleared = project.cache_dir.is_some();
    project.cache_dir = None;

    if hash_cleared || dir_cleared {
        println!(
            "  {:<15} {}",
            "Index State:".blue(),
            "Cache hash and directory cleared.".green()
        );
    }

    // CRITICAL: Do NOT save the index. `main` will persist the changes.

    if !file_deleted && !hash_cleared && !dir_cleared {
        println!(
            "\n{}",
            "No cache information was found for this project. Nothing to do.".yellow()
        );
    } else {
        println!(
            "\n{}",
            "✅ Successfully cleared cache. It will be regenerated on the next run.".bold()
        );
    }

    Ok(())
}
