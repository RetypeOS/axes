// src/core/paths.rs

use crate::constants::GLOBAL_INDEX_FILENAME; // Use the new constant
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

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

/// Returns the path to the Axes configuration directory.
/// Creates it if it doesn't exist.
pub fn get_axes_config_dir() -> Result<PathBuf, PathError> {
    let config_path = dirs::config_dir()
        .ok_or(PathError::ConfigDirNotFound)?
        .join("axes");

    if !config_path.exists() {
        fs::create_dir_all(&config_path).map_err(|e| PathError::ConfigDirCreation {
            path: config_path.display().to_string(),
            source: e,
        })?;
    }
    Ok(config_path)
}

/// Returns the path to the global `index.toml` file.
/// This is the main file in the axes configuration directory.
pub fn get_global_index_path() -> Result<PathBuf, PathError> {
    get_axes_config_dir().map(|dir| dir.join(GLOBAL_INDEX_FILENAME))
}
