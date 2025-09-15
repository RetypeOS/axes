// src/core/paths.rs

use crate::constants::GLOBAL_INDEX_FILENAME; // Usar la nueva constante
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

/// Devuelve la ruta al directorio de configuración de Axes.
/// Lo crea si no existe.
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

/// Devuelve la ruta al archivo `index.toml` global.
/// Este es el archivo principal en el directorio de configuración de axes.
pub fn get_global_index_path() -> Result<PathBuf, PathError> {
    get_axes_config_dir().map(|dir| dir.join(GLOBAL_INDEX_FILENAME))
}
