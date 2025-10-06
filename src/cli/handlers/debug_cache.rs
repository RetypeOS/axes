// EN: src/cli/handlers/debug_cache.rs

use crate::{
    constants::{AXES_DIR},
    core::{context_resolver, index_manager},
    models::{CachedProjectConfig, GlobalIndex},
};
use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use std::fs;

/// Defines the subcommands for the `_cache` action.
#[derive(Subcommand, Debug)]
enum CacheSubcommand {
    /// Deserializes and prints the content of a project's config cache.
    Inspect,
    /// Deletes the cache for a specific project, forcing regeneration.
    Clear,
}

/// Defines the arguments for the `_cache` handler.
#[derive(Parser, Debug)]
#[command(no_binary_name = true, hide = true)]
struct CacheArgs {
    #[command(subcommand)]
    command: CacheSubcommand,
}

/// The main handler for the `_cache` command.
/// It now expects the context from the dispatcher and parses only its own subcommands.
pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. The handler requires an explicit context.
    let context_str = context
        .ok_or_else(|| anyhow!("The '_cache' command requires an explicit project context."))?;

    // 2. Parse the specific subcommands for `_cache`.
    let cache_args = CacheArgs::try_parse_from(&args)?;

    let project = index.projects.get(&uuid).unwrap();
    let cache_dir = project.cache_dir.as_ref()
        .ok_or_else(|| anyhow!("Project '{}' has no resolved cache directory.", context_str))?;
    let cache_hash = project.config_hash.as_ref()
        .ok_or_else(|| anyhow!("Project '{}' has no resolved config hash.", context_str))?;
    let cache_path = cache_dir.join(cache_hash);

    // 4. Execute logic based on the subcommand.
    match cache_args.command {
        CacheSubcommand::Inspect => {
            println!("Inspecting cache for project '{}'...", context_str);
            println!("Cache file path: {}", cache_path.display());

            if !cache_path.exists() {
                println!("\nCache file does not exist.");
                return Ok(());
            }

            let bytes = fs::read(&cache_path).with_context(|| {
                format!("Failed to read cache file at {}", cache_path.display())
            })?;

            if bytes.is_empty() {
                println!("\nCache file is empty.");
                return Ok(());
            }

            let (cache_data, _): (CachedProjectConfig, usize) =
                bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                    .context("Failed to deserialize cache file. It might be corrupt.")?;

            let json_output = serde_json::to_string_pretty(&cache_data)
                .context("Failed to serialize cache data to JSON.")?;

            println!("\n--- Cache Content (as JSON) ---");
            println!("{}", json_output);
        }
        CacheSubcommand::Clear => {
            // This now clears the object file. We can also add a command to clear the whole dir.
            if cache_path.exists() {
                fs::remove_file(&cache_path)?;
                // Also clear the hash from the index so it gets regenerated.
                if let Some(entry) = index.projects.get_mut(&uuid) {
                    entry.config_hash = None;
                    entry.cache_dir = None; // Force re-resolution of path too
                }
                index_manager::save_global_index(index)?;
                println!(
                    "✅ Successfully cleared cache for project '{}'.",
                    context_str
                );
            }
        }
    }

    Ok(())
}
