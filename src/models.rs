// EN: src/models.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// =========================================================================
// === 1. TOML CONFIGURATION MODELS (User-Facing)
// =========================================================================
// These structs define the flexible syntax a user can write in `axes.toml`.

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum Runnable {
    Sequence(Vec<String>),
    Single(String),
}

/// The fully-featured, platform-aware representation of a command definition.
/// All other syntaxes are converted into this one after deserialization.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CanonicalCommand {
    pub default: Option<Runnable>,
    pub windows: Option<Runnable>,
    pub linux: Option<Runnable>,
    pub macos: Option<Runnable>,
    pub desc: Option<String>,
}

/// A helper enum to deserialize all flexible command syntaxes from TOML.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum TomlCommand {
    Simple(String),
    Sequence(Vec<String>),
    Extended { run: Runnable, desc: Option<String> },
    Platform(CanonicalCommand),
}

/// Conversion from the flexible TOML enum to our strict, canonical struct.
impl From<TomlCommand> for CanonicalCommand {
    fn from(toml_cmd: TomlCommand) -> Self {
        match toml_cmd {
            TomlCommand::Simple(s) => Self {
                default: Some(Runnable::Single(s)),
                ..Default::default()
            },
            TomlCommand::Sequence(s) => Self {
                default: Some(Runnable::Sequence(s)),
                ..Default::default()
            },
            TomlCommand::Extended { run, desc } => Self {
                default: Some(run),
                desc,
                ..Default::default()
            },
            TomlCommand::Platform(pc) => pc,
        }
    }
}

/// A public wrapper that uses the `TomlCommand` enum for flexible deserialization.
/// This is the type that will be used in `ProjectConfig`.
#[derive(Serialize, Debug, Clone, Default)]
pub struct Command(pub CanonicalCommand);

// We need a way to create a Command from a simple string, for `vars`.
impl From<String> for Command {
    fn from(s: String) -> Self {
        Command(CanonicalCommand {
            default: Some(Runnable::Single(s)),
            ..Default::default()
        })
    }
}

impl<'de> Deserialize<'de> for Command {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Command(TomlCommand::deserialize(deserializer)?.into()))
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct OptionsConfig {
    pub at_start: Option<Command>,
    pub at_exit: Option<Command>,
    pub shell: Option<String>,
    #[serde(default)]
    pub open_with: HashMap<String, Command>,
    #[serde(default)]
    pub cache_dir: Option<String>,
}

/// Represents the direct structure of an `axes.toml` file.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ProjectConfig {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub scripts: HashMap<String, Command>,
    #[serde(default)]
    pub options: OptionsConfig,
    #[serde(default)]
    pub vars: HashMap<String, String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

// =========================================================================
// === 2. INTERNAL & RUNTIME MODELS
// =========================================================================
// These are the primary structs used by the program logic after configuration is loaded.

// --- Parameter & Task Execution Models ---

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParameterDef {
    pub kind: ParameterKind,
    pub modifiers: ParameterModifiers,
    pub original_token: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParameterKind {
    Positional { index: usize },
    Named { name: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ParameterModifiers {
    pub required: bool,
    pub default_value: Option<String>,
    pub alias: Option<String>,
    pub map: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RunSpec {
    /// Represents a literal shell command, e.g., `<axes::run("./get_version.sh")>`
    Literal(String),
    // /// Represents a reference to another axes script, e.g., `<axes::run::get_version_script>`
    // Script(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TemplateComponent {
    Literal(String),
    Parameter(ParameterDef),
    GenericParams,
    Run(RunSpec),
    Path,
    Name,
    Uuid,
    Version,
}

/// Represents the specific action to be performed for a single line in a script.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CommandAction {
    /// Execute a shell command.
    Execute(Vec<TemplateComponent>),
    /// Print a line directly to the console.
    Print(Vec<TemplateComponent>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandExecution {
    pub action: CommandAction,
    pub ignore_errors: bool,
    pub run_in_parallel: bool,
    pub silent_mode: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Task {
    pub commands: Vec<CommandExecution>,
    pub desc: Option<String>,
}

// --- Cache & Resolved Config Models ---

#[derive(Debug, Clone, Default)]
pub struct ResolvedOptionsConfig {
    pub at_start: Option<Task>,
    pub at_exit: Option<Task>,
    pub shell: Option<String>,
    pub open_with: HashMap<String, Task>,
    pub cache_dir: Option<String>, // This is the template string
}

// This struct will now be built dynamically by merging `CachedProjectConfig` layers.
// For Phase 1, we will still build it monolithically, but the structure is prepared for Phase 2.
#[derive(Debug, Clone, Default)]
pub struct ResolvedConfig {
    pub uuid: Uuid,
    pub qualified_name: String,
    pub project_root: PathBuf,
    pub version: Option<String>,
    pub description: Option<String>,
    pub scripts: HashMap<String, Task>,
    pub vars: HashMap<String, Task>,
    pub env: HashMap<String, String>,
    pub options: ResolvedOptionsConfig, // This now holds compiled Tasks
}

// =========================================================================
// === 3. PERSISTENCE & SYSTEM MODELS
// =========================================================================

// --- Global Index & Local References ---

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexEntry {
    pub name: String,
    pub path: PathBuf,
    pub parent: Option<Uuid>,
    
    // The hash of the project's own axes.toml file content.
    // Used to validate the single-layer cache.
    pub config_hash: Option<String>,
    
    // The resolved, absolute path to the directory where this project's
    // cache objects are stored. Inherited and resolved.
    pub cache_dir: Option<PathBuf>,
    
    // UUID of the most recently used direct child of this project.
    pub last_used_child: Option<Uuid>,
}

/// Represents the pre-parsed and pre-expanded content of a single `axes.toml` file.
/// This is the unit that will be stored in the single-layer cache.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CachedProjectConfig {
    /// The project's own version, if defined.
    pub version: Option<String>,
    /// The project's own description, if defined.
    pub description: Option<String>,
    /// Scripts defined directly in this project's `axes.toml`, already expanded to AST.
    pub scripts: HashMap<String, Task>,
    /// Variables defined directly in this project's `axes.toml`, already expanded to AST.
    pub vars: HashMap<String, Task>,
    /// Environment variables defined directly in this project's `axes.toml`.
    pub env: HashMap<String, String>,
    /// Options defined directly in this project's `axes.toml`.
    /// Note: `cache_dir` is stored as a template string here.
    pub options: OptionsConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct GlobalIndex {
    #[serde(default)]
    pub projects: HashMap<Uuid, IndexEntry>,
    #[serde(default)]
    pub aliases: HashMap<String, Uuid>,
    pub last_used: Option<Uuid>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectRef {
    pub self_uuid: Uuid,
    pub parent_uuid: Option<Uuid>,
    pub name: String,
}

// --- Shell Configuration ---

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ShellConfig {
    pub path: PathBuf,
    pub interactive_args: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ShellsConfig {
    #[serde(default)]
    pub shells: HashMap<String, ShellConfig>,
}

// --- Binary Cache Serialization Substitutes ---

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub(crate) struct SerializableSystemTime(Duration);

// =========================================================================
// === 4. CONVERSIONS & IMPLEMENTATIONS
// =========================================================================

impl ProjectConfig {
    /// Creates a new, default ProjectConfig. This is used to generate
    /// the initial `axes.toml` for the global project.
    pub fn new() -> Self {
        let mut open_with_defaults = HashMap::new();

        // --- Editor scripts ---
        open_with_defaults.insert(
            "editor".to_string(),
            Command(CanonicalCommand {
                default: Some(Runnable::Single(
                    "<axes::vars::editor_cmd> \"<axes::path>\"".to_string(),
                )),
                ..Default::default()
            }),
        );
        open_with_defaults.insert(
            "idea".to_string(),
            Command(CanonicalCommand {
                default: Some(Runnable::Single(
                    "<axes::vars::idea_cmd> \"<axes::path>\"".to_string(),
                )),
                ..Default::default()
            }),
        );

        // --- OS-Specific File Explorer scripts ---
        if cfg!(target_os = "windows") {
            open_with_defaults.insert(
                "explorer".to_string(),
                Command(CanonicalCommand {
                    default: Some(Runnable::Single("-explorer \"<axes::path>\"".to_string())),
                    ..Default::default()
                }),
            );
            open_with_defaults.insert(
                "default".to_string(),
                Command(CanonicalCommand {
                    default: Some(Runnable::Single("explorer".to_string())),
                    ..Default::default()
                }),
            );
        } else if cfg!(target_os = "macos") {
            open_with_defaults.insert(
                "finder".to_string(),
                Command(CanonicalCommand {
                    default: Some(Runnable::Single("open \"<axes::path>\"".to_string())),
                    ..Default::default()
                }),
            );
            open_with_defaults.insert(
                "default".to_string(),
                Command(CanonicalCommand {
                    default: Some(Runnable::Single("finder".to_string())),
                    ..Default::default()
                }),
            );
        } else {
            // Linux and others
            open_with_defaults.insert(
                "files".to_string(),
                Command(CanonicalCommand {
                    default: Some(Runnable::Single("xdg-open \"<axes::path>\"".to_string())),
                    ..Default::default()
                }),
            );
            open_with_defaults.insert(
                "default".to_string(),
                Command(CanonicalCommand {
                    default: Some(Runnable::Single("files".to_string())),
                    ..Default::default()
                }),
            );
        }

        // --- Terminal/Shell Command ---
        if cfg!(target_os = "windows") {
            open_with_defaults.insert(
                "shell".to_string(),
                Command(CanonicalCommand {
                    default: Some(Runnable::Single(
                        "start cmd.exe /K \"cd /D <axes::path>\"".to_string(),
                    )),
                    ..Default::default()
                }),
            );
        } else {
            open_with_defaults.insert(
                "shell".to_string(),
                Command(CanonicalCommand {
                    default: Some(Runnable::Single("<axes::vars::terminal_cmd>".to_string())),
                    ..Default::default()
                }),
            );
        }

        // --- Default Variables ---
        let mut vars_defaults = HashMap::new();
        vars_defaults.insert("editor_cmd".to_string(), "code".to_string());
        vars_defaults.insert("idea_cmd".to_string(), "idea".to_string());
        vars_defaults.insert(
            "terminal_cmd".to_string(),
            "gnome-terminal --working-directory=<axes::path>".to_string(),
        );

        Self {
            name: Some("global".to_string()),
            version: Some("0.1.0".to_string()),
            description: Some("The global axes project configuration.".to_string()),
            scripts: HashMap::new(),
            options: OptionsConfig {
                open_with: open_with_defaults,
                at_start: None,
                at_exit: None,
                shell: None,
                cache_dir: None,
            },
            vars: vars_defaults,
            env: HashMap::new(),
        }
    }

    /// Creates a minimal yet structurally complete ProjectConfig for `axes init`.
    /// It acts as a scaffold for new projects.
    pub fn new_for_init(name: &str, version: &str, description: &str) -> Self {
        let mut scripts = HashMap::new();
        let mut vars = HashMap::new();

        // --- A simple, descriptive command to verify the setup ---
        let test_runnable =
            Runnable::Single("echo \"Test for '<axes::name>' successful!\"".to_string());
        let test_command = Command(CanonicalCommand {
            default: Some(test_runnable),
            desc: Some("Run a simple test echo command.".to_string()),
            ..Default::default()
        });
        scripts.insert("test".to_string(), test_command);

        // --- A placeholder variable ---
        vars.insert("GREETING".to_string(), "Hello from there!".to_string());

        // --- Placeholders for session hooks ---
        let options = OptionsConfig {
            at_start: Some(Command(CanonicalCommand {
                default: Some(Runnable::Single("".to_string())),
                desc: Some(
                    "Commands to run when entering a session (e.g., `source .venv/bin/activate`)"
                        .to_string(),
                ),
                ..Default::default()
            })),
            at_exit: Some(Command(CanonicalCommand {
                default: Some(Runnable::Single("".to_string())),
                desc: Some(
                    "Commands to run when exiting a session (e.g., `docker-compose down`)"
                        .to_string(),
                ),
                ..Default::default()
            })),
            ..Default::default()
        };

        Self {
            name: Some(name.to_string()),
            version: Some(version.to_string()),
            description: Some(description.to_string()),
            scripts,
            vars,
            options,
            env: HashMap::new(),
        }
    }
}

// --- Conversions for Serialization ---

impl From<SystemTime> for SerializableSystemTime {
    fn from(time: SystemTime) -> Self {
        Self(time.duration_since(UNIX_EPOCH).unwrap_or_default())
    }
}

impl From<SerializableSystemTime> for SystemTime {
    fn from(time: SerializableSystemTime) -> Self {
        UNIX_EPOCH + time.0
    }
}
