// src/models.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// --- MODELOS PARA PARSEO DE PARÁMETROS ---
// Estas son las estructuras primarias que se usarán tanto en tiempo de ejecución
// como para la serialización en el caché binario.

// NUEVAS ESTRUCTURAS PARA LA EJECUCIÓN
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandExecution {
    pub template: Vec<TemplateComponent>,
    pub ignore_errors: bool,
    pub run_in_parallel: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Task {
    pub commands: Vec<CommandExecution>,
    pub desc: Option<String>,
}

/// Representa un único componente de una plantilla de script pre-parseada.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TemplateComponent {
    Literal(String),
    Parameter(ParameterDef),
    GenericParams,
}

/// Define declarativamente un parámetro esperado por un script.
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

/// Representa un valor en el caché que puede estar en su estado crudo (del .toml)
/// o ya expandido y parseado en componentes para una ejecución rápida.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CacheableValue {
    Raw {
        command: Command,
        desc: Option<String>,
    },
    Expanded(Task),
}

// --- PUBLIC COMMAND MODELS (FOR TOML) ---
// These are what the user sees and uses in axes.toml

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum Runnable {
    Sequence(Vec<String>),
    Single(String),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ExtendedCommand {
    pub run: Runnable,
    pub desc: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PlatformCommand {
    #[serde(default)]
    pub default: Option<Runnable>,
    pub windows: Option<Runnable>,
    pub linux: Option<Runnable>,
    pub macos: Option<Runnable>,
    pub desc: Option<String>,
}

/// Represents a command in `axes.toml`. Uses `untagged` for flexible syntax.
/// It's only for deserializing from TOML, not for serializing to bincode.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum Command {
    Sequence(Vec<String>),
    Simple(String),
    Extended(ExtendedCommand),
    Platform(PlatformCommand),
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct OptionsConfig {
    pub at_start: Option<Command>,
    pub at_exit: Option<Command>,
    pub shell: Option<String>,
    #[serde(default)]
    pub open_with: HashMap<String, Command>,
}

// --- `axes.toml` MODELS (What is read from the configuration file) ---

/// Represents the deserialized structure of an `axes.toml` file.
/// Only needs `Deserialize`.
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

impl ProjectConfig {
    /// Creates a new, default ProjectConfig. This is used to generate
    /// the initial `axes.toml` for the global project and for `axes init`.
    pub fn new() -> Self {
        let mut open_with_defaults = HashMap::new();

        // --- Editor scripts ---
        // Uses a variable `<axes::vars::editor_cmd>` so the user can easily
        // override it (e.g., to "code-insiders" or "vim").
        open_with_defaults.insert(
            "editor".to_string(),
            Command::Simple("<axes::vars::editor_cmd> \"<axes::path>\"".to_string()),
        );
        open_with_defaults.insert(
            "idea".to_string(),
            Command::Simple("<axes::vars::idea_cmd> \"<axes::path>\"".to_string()),
        );

        // --- OS-Specific File Explorer scripts ---
        if cfg!(target_os = "windows") {
            open_with_defaults.insert(
                "explorer".to_string(),
                Command::Simple("-explorer \"<axes::path>\"".to_string()),
            );
            // The default action on Windows is to open the file explorer.
            open_with_defaults.insert(
                "default".to_string(), 
                Command::Simple("explorer".to_string())
            );
        } else if cfg!(target_os = "macos") {
            open_with_defaults.insert(
                "finder".to_string(), 
                Command::Simple("open \"<axes::path>\"".to_string())
            );
            open_with_defaults.insert(
                "default".to_string(), 
                Command::Simple("finder".to_string())
            );
        } else {
            // Linux and other Unix-like systems.
            open_with_defaults.insert(
                "files".to_string(), 
                Command::Simple("xdg-open \"<axes::path>\"".to_string())
            );
            open_with_defaults.insert(
                "default".to_string(), 
                Command::Simple("files".to_string())
            );
        }

        // --- Terminal/Shell Command ---
        // This is useful for quickly opening a new terminal session at the project root.
        // It doesn't start an `axes` session, just a native terminal.
        if cfg!(target_os = "windows") {
            open_with_defaults.insert(
                "shell".to_string(),
                Command::Simple("start cmd.exe /K \"cd /D <axes::path>\"".to_string())
            );
        } else {
            // This is more complex on Linux/macOS as it depends on the terminal emulator.
            // We provide a common default that users can override.
            open_with_defaults.insert(
                "shell".to_string(),
                Command::Simple("<axes::vars::terminal_cmd>".to_string())
            );
        }

        // --- Default Variables ---
        let mut vars_defaults = HashMap::new();
        vars_defaults.insert("editor_cmd".to_string(), "code".to_string());
        vars_defaults.insert("idea_cmd".to_string(), "idea".to_string());

        // A sensible default for terminal command on non-Windows systems.
        // The user is expected to change this to their preferred terminal (e.g., "kitty", "alacritty").
        vars_defaults.insert(
            "terminal_cmd".to_string(),
            "gnome-terminal --working-directory=<axes::path>".to_string(),
        );

        Self {
            // For `init`, these provide a nice starting point.
            // For `global`, they serve as documentation.
            name: Some("global".to_string()),
            version: Some("0.1.0".to_string()),
            description: Some("A new project managed by `axes`.".to_string()),

            // `scripts` is empty by default. `init` could add a "hello" script,
            // but the `global` project itself doesn't need it.
            scripts: HashMap::new(),

            options: OptionsConfig {
                open_with: open_with_defaults,
                at_start: None,
                at_exit: None,
                shell: None,
            },

            vars: vars_defaults,

            env: HashMap::new(),
        }
    }

    /// Creates a minimal yet structurally complete ProjectConfig for `axes init`.
    /// It acts as a scaffold, guiding the user without being prescriptive.
    pub fn new_for_init(name: &str, version: &str, description: &str) -> Self {
        let mut scripts = HashMap::new();
        let mut vars = HashMap::new();

        // --- A single, simple command to verify the setup ---
        scripts.insert(
            "test".to_string(),
            Command::Extended(ExtendedCommand {
                desc: Some("Run a simple test echo command.".to_string()),
                run: Runnable::Single("echo \"Test for '<axes::name>' successful!\"".to_string()),
            }),
        );

        // --- A placeholder variable ---
        vars.insert("GREETING".to_string(), "Hello from there!".to_string());

        // --- Placeholders for session hooks in [options] ---
        // We use a command that is unlikely to exist to prevent accidental execution,
        // but shows the user where to put their real scripts.
        // A commented-out example is even better, but TOML serialization
        // of comments is not standard. An empty string is the cleanest approach.
        let options = OptionsConfig {
            at_start: Some(Command::Simple("".to_string())), // Placeholder for environment setup (e.g., `source .venv/bin/activate`)
            at_exit: Some(Command::Simple("".to_string())),  // Placeholder for cleanup (e.g., `docker-compose down`)
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

// --- GLOBAL INDEX MODELS ---

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub name: String,
    pub path: PathBuf,
    pub parent: Option<Uuid>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct GlobalIndex {
    #[serde(default)]
    pub projects: HashMap<Uuid, IndexEntry>,
    #[serde(default)]
    pub aliases: HashMap<String, Uuid>,
    pub last_used: Option<Uuid>,
}

// --- LOCAL CACHE MODELS ---

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ChildCache {
    #[serde(default)]
    pub children: HashMap<String, Uuid>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct LastUsedCache {
    pub child_uuid: Option<Uuid>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectRef {
    pub self_uuid: Uuid,
    pub parent_uuid: Option<Uuid>,
    pub name: String,
}

// NEW: A resolved version of `OptionsConfig` that uses `CacheableValue`.
#[derive(Debug, Clone, Default)]
pub struct ResolvedOptionsConfig {
    pub at_start: Option<CacheableValue>,
    pub at_exit: Option<CacheableValue>,
    pub shell: Option<String>,
    pub open_with: HashMap<String, CacheableValue>,
}

// --- IN-MEMORY MODELS (Our internal working representation) ---

/// The final, merged view of the configuration.
/// Does not need `Serialize` or `Deserialize` because it is NEVER directly written/read.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub uuid: Uuid,
    pub qualified_name: String,
    pub project_root: PathBuf,
    pub version: Option<String>,
    pub description: Option<String>,
    pub scripts: HashMap<String, CacheableValue>,
    pub vars: HashMap<String, CacheableValue>,
    pub env: HashMap<String, String>,
    pub options: ResolvedOptionsConfig,
}

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

// --- SERIALIZATION SUBSTITUTES MODELS (For the binary cache) ---
// These are private to the crate and are only used for conversion.

/// A substitute `enum` for `Command` that is explicit and serializable by `bincode`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) enum SerializableCommand {
    Sequence(Vec<String>),
    Simple(String),
    Extended(SerializableExtendedCommand),
    Platform(SerializablePlatformCommand),
}

/// Substitute for `Runnable` that is bincode-safe (tagged).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) enum SerializableRunnable {
    Sequence(Vec<String>),
    Single(String),
}

/// Substitute for `ExtendedCommand` that uses `SerializableRunnable`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct SerializableExtendedCommand {
    pub run: SerializableRunnable,
    pub desc: Option<String>,
}

/// Substitute for `PlatformCommand` that uses `SerializableRunnable`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct SerializablePlatformCommand {
    #[serde(default)]
    pub default: Option<SerializableRunnable>,
    pub windows: Option<SerializableRunnable>,
    pub linux: Option<SerializableRunnable>,
    pub macos: Option<SerializableRunnable>,
    pub desc: Option<String>,
}

/// A `SystemTime` wrapper that is serializable.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub(crate) struct SerializableSystemTime(Duration);

/// The substitute for `ResolvedConfig` that uses serializable types (`String` instead of `PathBuf`).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct SerializableResolvedConfig {
    pub uuid: Uuid,
    pub qualified_name: String,
    pub project_root: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub scripts: HashMap<String, CacheableValue>,
    pub vars: HashMap<String, CacheableValue>,
    pub env: HashMap<String, String>,
    pub options: SerializableResolvedOptionsConfig,
}

// ¡NUEVO! Sustituto serializable para `TemplateComponent`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) enum SerializableTemplateComponent {
    Literal(String),
    // Almacenamos los componentes de ParameterDef directamente.
    Parameter {
        kind: SerializableParameterKind,
        modifiers: SerializableParameterModifiers,
        original_token: String,
    },
    GenericParams,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum SerializableParameterKind {
    Positional { index: usize },
    Named { name: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub(crate) struct SerializableParameterModifiers {
    pub required: bool,
    pub default_value: Option<String>,
    pub alias: Option<String>,
    pub map: Option<String>,
}

/// The main container for the configuration cache that is written to disk.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct SerializableConfigCache {
    pub resolved_config: SerializableResolvedConfig,
    pub dependencies: HashMap<String, SerializableSystemTime>,
}

/// --- Conversions TO Serializable models (for writing the cache) ---
impl From<&Runnable> for SerializableRunnable {
    fn from(value: &Runnable) -> Self {
        match value {
            Runnable::Sequence(s) => SerializableRunnable::Sequence(s.clone()),
            Runnable::Single(s) => SerializableRunnable::Single(s.clone()),
        }
    }
}

impl From<&ExtendedCommand> for SerializableExtendedCommand {
    fn from(value: &ExtendedCommand) -> Self {
        Self {
            run: (&value.run).into(),
            desc: value.desc.clone(),
        }
    }
}

impl From<&PlatformCommand> for SerializablePlatformCommand {
    fn from(value: &PlatformCommand) -> Self {
        Self {
            // Here we correctly handle Option using .map() instead of a forbidden impl
            default: value.default.as_ref().map(|v| v.into()),
            windows: value.windows.as_ref().map(|v| v.into()),
            linux: value.linux.as_ref().map(|v| v.into()),
            macos: value.macos.as_ref().map(|v| v.into()),
            desc: value.desc.clone(),
        }
    }
}

// This is the only implementation for From<&Command>
impl From<&Command> for SerializableCommand {
    fn from(value: &Command) -> Self {
        match value {
            Command::Sequence(s) => SerializableCommand::Sequence(s.clone()),
            Command::Simple(s) => SerializableCommand::Simple(s.clone()),
            // `e` is already a reference, `into()` will work directly
            Command::Extended(e) => SerializableCommand::Extended(e.into()),
            Command::Platform(p) => SerializableCommand::Platform(p.into()),
        }
    }
}

impl From<&ResolvedConfig> for SerializableResolvedConfig {
    fn from(value: &ResolvedConfig) -> Self {
        Self {
            uuid: value.uuid,
            qualified_name: value.qualified_name.clone(),
            project_root: value.project_root.to_string_lossy().into_owned(),
            version: value.version.clone(),
            description: value.description.clone(),
            scripts: value.scripts.clone(),
            vars: value.vars.clone(),
            env: value.env.clone(),
            options: (&value.options).into(),
        }
    }
}



// --- Conversions FROM Serializable models (for reading the cache) ---

impl From<SerializableRunnable> for Runnable {
    fn from(value: SerializableRunnable) -> Self {
        match value {
            SerializableRunnable::Sequence(s) => Runnable::Sequence(s),
            SerializableRunnable::Single(s) => Runnable::Single(s),
        }
    }
}

impl From<SerializableExtendedCommand> for ExtendedCommand {
    fn from(value: SerializableExtendedCommand) -> Self {
        Self {
            run: value.run.into(),
            desc: value.desc,
        }
    }
}

impl From<SerializablePlatformCommand> for PlatformCommand {
    fn from(value: SerializablePlatformCommand) -> Self {
        Self {
            default: value.default.map(|v| v.into()),
            windows: value.windows.map(|v| v.into()),
            linux: value.linux.map(|v| v.into()),
            macos: value.macos.map(|v| v.into()),
            desc: value.desc,
        }
    }
}

impl From<SerializableCommand> for Command {
    fn from(value: SerializableCommand) -> Self {
        match value {
            SerializableCommand::Sequence(s) => Command::Sequence(s),
            SerializableCommand::Simple(s) => Command::Simple(s),
            SerializableCommand::Extended(e) => Command::Extended(e.into()),
            SerializableCommand::Platform(p) => Command::Platform(p.into()),
        }
    }
}

impl From<SerializableResolvedConfig> for ResolvedConfig {
    fn from(value: SerializableResolvedConfig) -> Self {
        Self {
            uuid: value.uuid,
            qualified_name: value.qualified_name,
            project_root: PathBuf::from(value.project_root),
            version: value.version,
            description: value.description,
            scripts: value.scripts,
            vars: value.vars,
            env: value.env,
            options: value.options.into(),
        }
    }
}

// --- Conversions for SystemTime ---

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

// Extras for change pos

// NEW: A serializable version of `ResolvedOptionsConfig`.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub(crate) struct SerializableResolvedOptionsConfig {
    pub at_start: Option<CacheableValue>,
    pub at_exit: Option<CacheableValue>,
    pub shell: Option<String>,
    pub open_with: HashMap<String, CacheableValue>,
}

// NEW: Conversions for `ResolvedOptionsConfig`
impl From<&ResolvedOptionsConfig> for SerializableResolvedOptionsConfig {
    fn from(value: &ResolvedOptionsConfig) -> Self {
        Self {
            at_start: value.at_start.clone(),
            at_exit: value.at_exit.clone(),
            shell: value.shell.clone(),
            open_with: value.open_with.clone(),
        }
    }
}

impl From<SerializableResolvedOptionsConfig> for ResolvedOptionsConfig {
    fn from(value: SerializableResolvedOptionsConfig) -> Self {
        Self {
            at_start: value.at_start,
            at_exit: value.at_exit,
            shell: value.shell,
            open_with: value.open_with,
        }
    }
}