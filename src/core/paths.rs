// src/core/paths.rs

use crate::constants::GLOBAL_INDEX_FILENAME;
use lazy_static::lazy_static;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use thiserror::Error;

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
