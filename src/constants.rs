// src/constants.rs

/// The name of the directory containing axes configuration for a project.
pub const AXES_DIR: &str = ".axes";

/// The name of the main configuration file for a project (inside .axes/).
pub const PROJECT_CONFIG_FILENAME: &str = "axes.toml";

/// The name of the cache file for a project's resolved configuration (inside .axes/).
pub const CONFIG_CACHE_FILENAME: &str = "config.cache.bin";

/// The name of the cache file for a project's children (inside .axes/).
pub const CHILDREN_CACHE_FILENAME: &str = "children.cache.bin";

/// The name of the global index file (in ~/.config/axes/).
pub const GLOBAL_INDEX_FILENAME: &str = "index.bin";

/// The name of the file containing a project's identity and references.
pub const PROJECT_REF_FILENAME: &str = "project_ref.bin";

pub const LAST_USED_CACHE_FILENAME: &str = "last_used.cache.bin";
