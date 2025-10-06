// EN: src/models.rs

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use uuid::Uuid;

use crate::constants::MAX_RECURSION_DEPTH;
use crate::core::config_resolver;

/// The result of loading a single configuration layer.
pub type LayerResult = Result<Arc<CachedProjectConfig>>;

/// A thread-safe container for a future `LayerResult`.
/// Consumers can wait on this promise until the layer is loaded by a worker thread.
pub type LayerPromise = Arc<OnceLock<LayerResult>>;

/// A data structure to securely pass updates for the GlobalIndex from worker threads
/// back to the main thread for sequential application.
#[derive(Debug, Clone)]
pub struct IndexUpdate {
    pub uuid: Uuid,
    pub new_hash: String,
    pub new_cache_dir: PathBuf,
}

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
    Script(String),
    Var(String),
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
    pub at_start: Option<Arc<Task>>,
    pub at_exit: Option<Arc<Task>>,
    pub shell: Option<String>,
    pub open_with: HashMap<String, Arc<Task>>,
    pub cache_dir: Option<String>,
}

/// An intelligent facade that provides access to the project's configuration.
/// It loads and merges configuration layers from the inheritance chain on-demand.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub uuid: Uuid,
    pub qualified_name: String,
    pub project_root: PathBuf,
    pub(crate) hierarchy: Arc<Vec<Uuid>>,
    pub(crate) layers: Arc<HashMap<Uuid, LayerPromise>>,
    memoized_scripts: Arc<Mutex<HashMap<String, Option<Arc<Task>>>>>,
    memoized_vars: Arc<Mutex<HashMap<String, Option<Arc<Task>>>>>,
    memoized_env: Arc<Mutex<Option<HashMap<String, String>>>>,
    memoized_version: Arc<Mutex<Option<Option<String>>>>,
    memoized_description: Arc<Mutex<Option<Option<String>>>>,
    memoized_options: Arc<Mutex<Option<ResolvedOptionsConfig>>>,
}

impl ResolvedConfig {
    /// Creates a new lazy facade, ready to resolve data from the provided layer promises.
    pub fn new(
        uuid: Uuid,
        qualified_name: String,
        project_root: PathBuf,
        hierarchy: Vec<Uuid>,
        layers: HashMap<Uuid, LayerPromise>,
    ) -> Self {
        Self {
            uuid,
            qualified_name,
            project_root,
            hierarchy: Arc::new(hierarchy),
            layers: Arc::new(layers),
            memoized_scripts: Arc::new(Mutex::new(HashMap::new())),
            memoized_vars: Arc::new(Mutex::new(HashMap::new())),
            memoized_env: Arc::new(Mutex::new(None)),
            memoized_version: Arc::new(Mutex::new(None)),
            memoized_description: Arc::new(Mutex::new(None)),
            memoized_options: Arc::new(Mutex::new(None)),
        }
    }

    // --- LAZY ACCESSOR METHODS ---
    // The public methods now only need `&mut GlobalIndex` because all layer
    // loading is channeled through the private `get_layer` method.

    /// Lazily finds and returns a script by name, searching up the inheritance chain.
    pub fn get_script(&self, name: &str, depth: u32) -> Result<Option<Arc<Task>>> {
        if depth > MAX_RECURSION_DEPTH {
            return Err(anyhow!(
                "Maximum recursion depth exceeded while resolving script '{}'.",
                name
            ));
        }
        if let Some(cached_result) = self.memoized_scripts.lock().unwrap().get(name) {
            return Ok(cached_result.clone());
        }

        let mut result = None;
        for &uuid in self.hierarchy.iter() {
            let layer = self.get_layer(uuid)?;
            if let Some(task) = layer.scripts.get(name) {
                result = Some(Arc::new(task.clone()));
                break;
            }
        }

        self.memoized_scripts
            .lock()
            .unwrap()
            .insert(name.to_string(), result.clone());
        Ok(result)
    }

    /// Lazily finds and returns a variable by name.
    pub fn get_var(&self, name: &str, depth: u32) -> Result<Option<Arc<Task>>> {
        if depth > MAX_RECURSION_DEPTH {
            return Err(anyhow!(
                "Maximum recursion depth exceeded while resolving var '{}'.",
                name
            ));
        }
        if let Some(cached_result) = self.memoized_vars.lock().unwrap().get(name) {
            return Ok(cached_result.clone());
        }

        let mut result = None;
        for &uuid in self.hierarchy.iter() {
            let layer = self.get_layer(uuid)?;
            if let Some(task) = layer.vars.get(name) {
                result = Some(Arc::new(task.clone()));
                break;
            }
        }

        self.memoized_vars
            .lock()
            .unwrap()
            .insert(name.to_string(), result.clone());
        Ok(result)
    }

    /// Lazily merges and returns all environment variables from the entire hierarchy.
    pub fn get_env(&self) -> Result<HashMap<String, String>> {
        if let Some(env) = self.memoized_env.lock().unwrap().as_ref() {
            return Ok(env.clone());
        }
        let mut final_env = HashMap::new();
        for &uuid in self.hierarchy.iter().rev() {
            let layer = self.get_layer(uuid)?;
            final_env.extend(layer.env.clone());
        }
        *self.memoized_env.lock().unwrap() = Some(final_env.clone());
        Ok(final_env)
    }

    /// Lazily finds and returns the project's version by searching up the hierarchy.
    pub fn get_version(&self) -> Result<Option<String>> {
        if let Some(version) = self.memoized_version.lock().unwrap().as_ref() {
            return Ok(version.clone());
        }
        let mut final_version = None;
        for &uuid in self.hierarchy.iter() {
            let layer = self.get_layer(uuid)?;
            if let Some(version) = &layer.version {
                final_version = Some(version.clone());
                break;
            }
        }
        *self.memoized_version.lock().unwrap() = Some(final_version.clone());
        Ok(final_version)
    }

    /// Lazily finds and returns the project's description by searching up the hierarchy.
    pub fn get_description(&self) -> Result<Option<String>> {
        if let Some(desc) = self.memoized_description.lock().unwrap().as_ref() {
            return Ok(desc.clone());
        }
        let mut final_desc = None;
        for &uuid in self.hierarchy.iter() {
            let layer = self.get_layer(uuid)?;
            if let Some(desc) = &layer.description {
                final_desc = Some(desc.clone());
                break;
            }
        }
        *self.memoized_description.lock().unwrap() = Some(final_desc.clone());
        Ok(final_desc)
    }

    /// Lazily merges and returns the final `ResolvedOptionsConfig` from the entire hierarchy.
    pub fn get_options(&self) -> Result<ResolvedOptionsConfig> {
        if let Some(options) = self.memoized_options.lock().unwrap().as_ref() {
            return Ok(options.clone());
        }
        let mut final_options = ResolvedOptionsConfig::default();
        for &uuid in self.hierarchy.iter().rev() {
            let layer = self.get_layer(uuid)?;
            final_options.shell = layer.options.shell.clone().or(final_options.shell);
            final_options.cache_dir = layer.options.cache_dir.clone().or(final_options.cache_dir);
            if let Some(cmd) = layer.options.at_start.clone() {
                final_options.at_start =
                    Some(config_resolver::compile_command_to_task(cmd.0)?.into());
            }
            if let Some(cmd) = layer.options.at_exit.clone() {
                final_options.at_exit =
                    Some(config_resolver::compile_command_to_task(cmd.0)?.into());
            }
            let compiled_open_with =
                config_resolver::compile_command_map(layer.options.open_with.clone())?;
            final_options.open_with.extend(
                compiled_open_with
                    .into_iter()
                    .map(|(k, v)| (k, Arc::new(v))),
            );
        }
        *self.memoized_options.lock().unwrap() = Some(final_options.clone());
        Ok(final_options)
    }

    // --- Helpers for `info` command ---

    /// Lazily merges and returns all scripts from the entire hierarchy.
    pub fn get_all_scripts(&self) -> Result<HashMap<String, Arc<Task>>> {
        let mut final_scripts = HashMap::new();
        for &uuid in self.hierarchy.iter().rev() {
            let layer = self.get_layer(uuid)?;
            for (name, task) in layer.scripts.iter() {
                final_scripts.insert(name.clone(), Arc::new(task.clone()));
            }
        }
        Ok(final_scripts)
    }

    /// Lazily merges and returns all vars from the entire hierarchy.
    pub fn get_all_vars(&self) -> Result<HashMap<String, Arc<Task>>> {
        let mut final_vars = HashMap::new();
        for &uuid in self.hierarchy.iter().rev() {
            let layer = self.get_layer(uuid)?;
            for (name, task) in layer.vars.iter() {
                final_vars.insert(name.clone(), Arc::new(task.clone()));
            }
        }
        Ok(final_vars)
    }

    // --- Private Core Helper ---

    /// Core helper to get a layer. It waits on the promise to be resolved by the `ConfigLoader`.
    /// This is the only method that needs to be aware of the underlying `layers` map.
    pub(crate) fn get_layer(&self, uuid: Uuid) -> Result<Arc<CachedProjectConfig>> {
        let promise = self.layers.get(&uuid).ok_or_else(|| {
            anyhow!(
                "Internal logic error: attempt to get a layer for UUID {} not in the hierarchy.",
                uuid
            )
        })?;

        // `get()` on an `OnceLock` will block if the value is not yet set.
        // This is where the main thread waits for a parallel worker thread to finish.
        let layer_result = promise.get().ok_or_else(|| {
            anyhow!(
                "Internal logic error: LayerPromise for UUID {} was never set.",
                uuid
            )
        })?;

        match layer_result {
            Ok(layer_arc) => Ok(layer_arc.clone()),
            Err(e) => Err(anyhow!(
                "Failed to load configuration layer for UUID {}: {}",
                uuid,
                e
            )),
        }
    }
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
