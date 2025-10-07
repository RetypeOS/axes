// src/core/paths.rs

use crate::constants::GLOBAL_INDEX_FILENAME;
use crate::models::GlobalIndex;
use anyhow::{Result, anyhow};
use lazy_static::lazy_static;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

lazy_static! {
    static ref AXES_CONFIG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
}

#[derive(Error, Debug)]
pub enum PathError {
    #[error("Could not find system config directory.")]
    ConfigDirNotFound,
    #[error("Could not create config directory at '{path}': {source}")]
    ConfigDirCreation {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Returns the path to the Axes configuration directory (`~/.config/axes`).
/// This function is memoized for high performance.
pub fn get_axes_config_dir() -> Result<PathBuf, PathError> {
    let mut cached_path_guard = AXES_CONFIG_DIR.lock().unwrap();
    if let Some(path) = &*cached_path_guard {
        return Ok(path.clone());
    }
    let config_path = dirs::config_dir()
        .ok_or(PathError::ConfigDirNotFound)?
        .join("axes");
    if !config_path.exists() {
        fs::create_dir_all(&config_path).map_err(|e| PathError::ConfigDirCreation {
            path: config_path.display().to_string(),
            source: e,
        })?;
    }
    *cached_path_guard = Some(config_path.clone());
    Ok(config_path)
}

/// Returns the path to the global `index.toml` file.
/// This is the main file in the axes configuration directory.
pub fn get_global_index_path() -> Result<PathBuf, PathError> {
    get_axes_config_dir().map(|dir| dir.join(GLOBAL_INDEX_FILENAME))
}

/// Expands a path template string, resolving home directory, environment variables,
/// and a limited set of safe `axes` tokens.
///
/// # Arguments
/// * `template` - The template string (e.g., "~/.cache/axes/<axes::uuid>").
/// * `project_uuid` - The UUID of the project to use for token expansion.
///
/// # Errors
/// Returns an error if the template contains unsupported dynamic tokens like
/// `<axes::params...>` or `<axes::run...>`, or if path expansion fails.
/// Expands a path template string, resolving home directory and environment variables.
fn expand_path_template(template: &str) -> Result<PathBuf> {
    // We remove the project-specific tokens here as the root path should be generic.
    if template.contains("<axes::") {
        return Err(anyhow!(
            "The 'cache_dir' path template should only define a root directory and must not contain dynamic axes tokens like <axes::uuid>. Invalid template: '{}'",
            template
        ));
    }
    let expanded_path_str = shellexpand::full(template)
        .map_err(|e| anyhow!("Failed to expand cache path template '{}': {}", template, e))?;
    Ok(PathBuf::from(expanded_path_str.into_owned()))
}

/// Returns the platform-specific default root directory for all axes caches.
/// This is the single source of truth for the default cache location.
/// - Windows: `%LOCALAPPDATA%\axes\cache`
/// - Linux/macOS: `~/.cache/axes`
fn get_default_cache_root() -> Result<PathBuf> {
    if cfg!(windows) {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            // Fallback to a subdir in the config dir if LOCALAPPDATA is not set
            .unwrap_or_else(|_| get_axes_config_dir().unwrap().join("cache_fallback"));
        Ok(local_app_data.join("axes").join("cache"))
    } else {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        Ok(home_dir.join(".cache").join("axes"))
    }
}

/// Returns the platform-specific default absolute path for a project's cache directory.
pub fn get_default_cache_dir_for_project(uuid: Uuid) -> Result<PathBuf> {
    // All projects, including 'global', will now have their cache in a subdirectory
    // named after their UUID inside `.../cache/projects`. This is consistent.
    let cache_root = get_default_cache_root()?;
    Ok(cache_root.join("projects").join(uuid.to_string()))
}

///
/// It determines the cache root by searching the hierarchy for a `cache_dir` option,
/// falling back to the system default, and then ALWAYS appends `/projects/<uuid>`.
///
/// # Arguments
/// * `uuid` - The UUID of the project for which to resolve the cache directory.
/// * `index` - The global index, used for traversing the project hierarchy.
/// * `compiled_layer_options` - The compiled `OptionsConfig` from the project's own `axes.toml`.
///   This is passed in to give priority to the project's own definition before starting inheritance search.
pub fn resolve_cache_dir_for_project(
    uuid: Uuid,
    index: &GlobalIndex,
    compiled_layer_options: &crate::models::CachedOptionsConfig,
) -> Result<PathBuf> {
    // 1. Determine the Cache Root Path.
    let cache_root = if let Some(template) = &compiled_layer_options.cache_dir {
        // Priority 1: The project defines its own cache root.
        expand_path_template(template)?
    } else {
        // Priority 2: Search ancestors for an inherited cache root.
        let mut inherited_root = None;
        let mut current_uuid = index.projects.get(&uuid).and_then(|e| e.parent);
        while let Some(id) = current_uuid {
            if let Some(entry) = index.projects.get(&id) {
                // We need to look at the parent's *resolved* cache dir and go one level up
                // to find its root. This assumes the parent's dir is already in the index.
                if let Some(parent_cache_dir) = &entry.cache_dir
                    && let Some(parent_root) = parent_cache_dir.parent().and_then(|p| p.parent())
                {
                    inherited_root = Some(parent_root.to_path_buf());
                    break;
                }

                current_uuid = entry.parent;
            } else {
                break; // Broken link
            }
        }

        // Use the inherited root or fall back to the system default.
        inherited_root.unwrap_or(get_default_cache_root()?)
    };

    // 2. Append the standard project-specific path. This is now universal.
    Ok(cache_root.join("projects").join(uuid.to_string()))
}
