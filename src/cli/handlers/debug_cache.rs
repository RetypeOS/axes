// EN: src/cli/handlers/debug_cache.rs

use crate::{
    constants::{AXES_DIR, CONFIG_CACHE_FILENAME},
    models::SerializableConfigCache,
};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;

#[derive(Parser, Debug)]
#[command(no_binary_name = true, hide = true)] // `hide = true` lo oculta de la ayuda
struct CacheArgs {
    #[command(subcommand)]
    command: CacheCommand,
}

#[derive(Subcommand, Debug)]
enum CacheCommand {
    /// Deserializes and prints the content of a project's config cache.
    Inspect {
        /// The project context whose cache you want to inspect.
        context: String,
    },
    /// Deletes the cache for a specific project, forcing regeneration.
    Clear {
        /// The project context whose cache you want to clear.
        context: String,
    },
}

pub fn handle(args: Vec<String>) -> Result<()> {
    let cache_args = CacheArgs::try_parse_from(&args)?;

    match cache_args.command {
        CacheCommand::Inspect { context } => {
            let index = crate::core::index_manager::load_and_ensure_global_project()?;
            let (uuid, _) = crate::core::context_resolver::resolve_context(&context, &index)?;
            let project = index.projects.get(&uuid).unwrap();
            let cache_path = project.path.join(AXES_DIR).join(CONFIG_CACHE_FILENAME);

            println!("Inspecting cache for project '{}'...", context);
            println!("Cache file path: {}", cache_path.display());

            if !cache_path.exists() {
                println!("\nCache file does not exist.");
                return Ok(());
            }

            let bytes = fs::read(&cache_path).with_context(|| {
                format!("Failed to read cache file at {}", cache_path.display())
            })?; // .with_context aquí es correcto porque usamos format!

            if bytes.is_empty() {
                println!("\nCache file is empty.");
                return Ok(());
            }

            let (cache_data, _): (SerializableConfigCache, usize) =
                bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                    .context("Failed to deserialize cache file. It might be corrupt.")?; // CORREGIDO: .context() en lugar de .with_context()

            let json_output = serde_json::to_string_pretty(&cache_data)
                .context("Failed to serialize cache data to JSON.")?; // CORREGIDO: .context() en lugar de .with_context()

            println!("\n--- Cache Content (as JSON) ---");
            println!("{}", json_output);
        }
        CacheCommand::Clear { context } => {
            let index = crate::core::index_manager::load_and_ensure_global_project()?;
            let (uuid, _) = crate::core::context_resolver::resolve_context(&context, &index)?;
            let project = index.projects.get(&uuid).unwrap();
            let cache_path = project.path.join(AXES_DIR).join(CONFIG_CACHE_FILENAME);

            if cache_path.exists() {
                fs::remove_file(&cache_path)?;
                println!("✅ Successfully cleared cache for project '{}'.", context);
            } else {
                println!(
                    "- Cache for project '{}' did not exist. Nothing to do.",
                    context
                );
            }
        }
    }

    Ok(())
}
