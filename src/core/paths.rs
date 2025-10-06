// src/core/paths.rs

use crate::{constants::GLOBAL_INDEX_FILENAME, models::ResolvedConfig};
use lazy_static::lazy_static;
use uuid::Uuid;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use thiserror::Error;
use anyhow::{Result, anyhow};

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
/// Creates it if it doesn't exist.
///
/// This function is memoized: the first call computes and caches the path,
/// subsequent calls return the cached value instantly.
pub fn get_axes_config_dir() -> Result<PathBuf, PathError> {
    // Acquire a lock on the cached path. This is a fast operation if not contended.
    let mut cached_path_guard = AXES_CONFIG_DIR.lock().unwrap();

    // If the path is already cached, clone it and return immediately.
    if let Some(path) = &*cached_path_guard {
        return Ok(path.clone());
    }

    // --- Cache miss: compute the path for the first time ---

    // 1. Find the system's generic config directory. This is the expensive part.
    let config_path = dirs::config_dir()
        .ok_or(PathError::ConfigDirNotFound)?
        .join("axes");

    // 2. Ensure the directory exists on the filesystem.
    if !config_path.exists() {
        fs::create_dir_all(&config_path).map_err(|e| PathError::ConfigDirCreation {
            path: config_path.display().to_string(),
            source: e,
        })?;
    }

    // 3. Store the computed path in the cache for future calls.
    *cached_path_guard = Some(config_path.clone());

    // 4. Return the computed path.
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
pub fn expand_path_template(template: &str, project_uuid: Uuid) -> Result<PathBuf> {
    // 1. Validate against dynamic, runtime-only tokens.
    if template.contains("<axes::params") || template.contains("<axes::run") {
        return Err(anyhow!(
            "The 'cache_dir' path template cannot contain dynamic tokens like <axes::params...> or <axes::run...>. Invalid template: '{}'",
            template
        ));
    }

    // 2. Expand `axes` tokens.
    let with_axes_tokens = template.replace("<axes::uuid>", &project_uuid.to_string());
    
    // 3. Expand home directory (`~`) and environment variables (`$VAR` or `%VAR%`).
    // `shellexpand::full` handles both home dir and env vars across platforms.
    let expanded_path_str = shellexpand::full(&with_axes_tokens)
        .map_err(|e| anyhow!("Failed to expand cache path template '{}': {}", template, e))?;

    Ok(PathBuf::from(expanded_path_str.into_owned()))
}

/// Determines the cache directory for a project based on its configuration or system defaults.
///
/// # Arguments
/// * `config` - The fully resolved configuration of the project.
///
/// # Returns
/// The absolute, resolved path to the cache directory for this project.
pub fn get_cache_dir_for_project(config: &ResolvedConfig) -> Result<PathBuf> {
    let template = match &config.options.cache_dir {
        Some(template_str) => template_str.clone(),
        None => {
            // Determine platform-specific default cache location.
            if cfg!(target_os = "windows") {
                // %LOCALAPPDATA%\axes\cache\<axes::uuid>
                let local_app_data = std::env::var("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| get_axes_config_dir().unwrap().join("cache")); // Fallback
                return Ok(local_app_data.join("axes").join("cache").join(config.uuid.to_string()));
            } else {
                // ~/.cache/axes/<axes::uuid> (XDG Base Directory Specification)
                let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
                return Ok(home_dir.join(".cache").join("axes").join(config.uuid.to_string()));
            }
        }
    };
    
    // Expand the user-defined template.
    expand_path_template(&template, config.uuid)
}