/// The name of the directory containing axes configuration for a project.
pub const AXES_DIR: &str = ".axes";

/// The name of the main configuration file for a project (inside .axes/).
pub const PROJECT_CONFIG_FILENAME: &str = "axes.toml";

/// The name of the global index file (in ~/.config/axes/).
pub const GLOBAL_INDEX_FILENAME: &str = "index.bin";

/// The name of the file containing a project's identity and references.
pub const PROJECT_REF_FILENAME: &str = "project_ref.bin";

/// -
pub const MAX_RECURSION_DEPTH: u32 = 32;
