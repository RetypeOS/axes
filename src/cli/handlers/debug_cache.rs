// EN: src/cli/handlers/debug_cache.rs

use crate::{
    constants::{AXES_DIR, CONFIG_CACHE_FILENAME},
    core::{context_resolver, index_manager},
    models::SerializableConfigCache,
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
pub fn handle(context: Option<String>, args: Vec<String>) -> Result<()> {
    // 1. The handler requires an explicit context.
    let context_str = context
        .ok_or_else(|| anyhow!("The '_cache' command requires an explicit project context."))?;

    // 2. Parse the specific subcommands for `_cache`.
    let cache_args = CacheArgs::try_parse_from(&args)?;

    // 3. Resolve the project entry from the context.
    let index = index_manager::load_and_ensure_global_project()?;
    let (uuid, _) = context_resolver::resolve_context(&context_str, &index)?;
    let project = index.projects.get(&uuid).unwrap();
    let cache_path = project.path.join(AXES_DIR).join(CONFIG_CACHE_FILENAME);

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

            let (cache_data, _): (SerializableConfigCache, usize) =
                bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                    .context("Failed to deserialize cache file. It might be corrupt.")?;

            let json_output = serde_json::to_string_pretty(&cache_data)
                .context("Failed to serialize cache data to JSON.")?;

            println!("\n--- Cache Content (as JSON) ---");
            println!("{}", json_output);
        }
        CacheSubcommand::Clear => {
            if cache_path.exists() {
                fs::remove_file(&cache_path)?;
                println!(
                    "✅ Successfully cleared cache for project '{}'.",
                    context_str
                );
            } else {
                println!(
                    "- Cache for project '{}' did not exist. Nothing to do.",
                    context_str
                );
            }
        }
    }

    Ok(())
}
