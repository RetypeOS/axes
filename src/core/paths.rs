//! # Filesystem Path Management
//!
//! This module provides centralized, robust, and platform-aware functions for handling
//! filesystem paths used by `axes`. It ensures that configuration files, caches, and
//! other assets are stored in predictable and conventional locations across different
//! operating systems.
//!
//! ## Key Functions
//!
//! - `get_axes_config_dir`: The single source of truth for the main `axes` config directory
//!   (e.g., `~/.config/axes`). It's memoized for performance.
//! - `get_default_cache_root`: Determines the default base directory for all caches, respecting
//!   OS conventions (e.g., `~/.cache/axes` on Linux, `%LOCALAPPDATA%\axes\cache` on Windows).
//! - `expand_path_template`: Safely expands shell-like path strings (e.g., `~/some/path`).

use crate::constants::GLOBAL_INDEX_FILENAME;
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

/// Represents errors that can occur during path resolution.
#[derive(Error, Debug)]
pub enum PathError {
    /// The system's standard configuration directory (e.g., `~/.config`) could not be determined.
    #[error("Could not find system config directory.")]
    ConfigDirNotFound,
    /// An I/O error occurred while attempting to create the `axes` configuration directory.
    #[error("Could not create config directory at '{path}': {source}")]
    ConfigDirCreation {
        /// The path that could not be created.
        path: String,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// Returns the path to the global index file (e.g., `~/.config/axes/index.bin`).
/// This is the main persistence file for the application's state.
pub fn get_axes_config_dir() -> Result<PathBuf, PathError> {
    let mut cached_path_guard = AXES_CONFIG_DIR
        .lock()
        .expect("Config dir mutex should not be poisoned");
    if let Some(path) = &*cached_path_guard {
        return Ok(path.clone());
    }

    let config_path = dirs::config_dir()
        .ok_or(PathError::ConfigDirNotFound)?
        .join("axes");

    if !config_path.exists() {
        log::info!(
            "First run detected: creating config directory at '{}'.",
            config_path.display()
        );
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

/// Expands a path template string, resolving home directory (`~`) and environment variables.
///
/// This function is used to resolve user-configured paths, such as the `cache_dir` option.
/// It explicitly forbids `axes`-specific dynamic tokens (like `<uuid>`) to ensure that
/// the configured path is a stable root directory.
///
/// # Arguments
/// * `template` - The path string to expand (e.g., `~/.my-caches/axes`).
///
/// # Errors
/// Returns an error if the expansion fails or if the template contains forbidden tokens.
pub fn expand_path_template(template: &str) -> Result<PathBuf> {
    // We remove the project-specific tokens here as the root path should be generic.
    if template.contains("<") {
        return Err(anyhow!(
            "The 'cache_dir' path template should only define a root directory and must not contain dynamic axes tokens like <uuid>. Invalid template: '{}'",
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
pub fn get_default_cache_root() -> Result<PathBuf> {
    if cfg!(windows) {
        let local_app_data = std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            // Fallback to a subdir in the config dir if LOCALAPPDATA is not set
            .unwrap_or_else(|_| {
                get_axes_config_dir()
                    .expect("Axes config dir should be resolvable")
                    .join("cache_fallback")
            });
        Ok(local_app_data.join("axes").join("cache"))
    } else {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        Ok(home_dir.join(".cache").join("axes"))
    }
}

/// Returns the platform-specific default absolute path for a single project's cache directory.
/// The path is constructed as `<default_cache_root>/projects/<uuid>`.
///
/// # Arguments
/// * `uuid` - The UUID of the project.
pub fn get_default_cache_dir_for_project(uuid: Uuid) -> Result<PathBuf> {
    // All projects, including 'global', will now have their cache in a subdirectory
    // named after their UUID inside `.../cache/projects`. This is consistent.
    let cache_root = get_default_cache_root()?;
    Ok(cache_root.join("projects").join(uuid.to_string()))
}
