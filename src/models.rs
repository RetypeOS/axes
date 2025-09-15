// src/models.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

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
    // Explicit fields for key options
    pub at_start: Option<String>,
    pub at_exit: Option<String>,
    pub shell: Option<String>,

    // The `open_with` sub-table
    #[serde(default)]
    pub open_with: HashMap<String, String>,
}

// --- `axes.toml` MODELS (What is read from the configuration file) ---

/// Represents the deserialized structure of an `axes.toml` file.
/// Only needs `Deserialize`.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ProjectConfig {
    pub version: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub commands: HashMap<String, Command>,
    #[serde(default)]
    pub options: OptionsConfig,
    #[serde(default)]
    pub vars: HashMap<String, String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl ProjectConfig {
    pub fn new() -> Self {
        let mut commands = HashMap::new();
        commands.insert(
            "hello".to_string(),
            Command::Simple("echo 'Hello from axes!'".to_string()),
        );
        let mut open_with_defaults = HashMap::new();
        if cfg!(target_os = "windows") {
            open_with_defaults.insert("explorer".to_string(), "-explorer '{root}'".to_string());
            open_with_defaults.insert("vsc".to_string(), "code {root}".to_string());
            open_with_defaults.insert("default".to_string(), "explorer".to_string());
        } else if cfg!(target_os = "macos") {
            open_with_defaults.insert("finder".to_string(), "open {root}".to_string());
            open_with_defaults.insert("vsc".to_string(), "code {root}".to_string());
            open_with_defaults.insert("default".to_string(), "finder".to_string());
        } else {
            // Linux y otros
            open_with_defaults.insert("xdg".to_string(), "xdg-open {root}".to_string());
            open_with_defaults.insert("nautilus".to_string(), "nautilus {root}".to_string());
            open_with_defaults.insert("vsc".to_string(), "code {root}".to_string());
            open_with_defaults.insert("default".to_string(), "xdg".to_string());
        }
        Self {
            version: Some("0.1.0".to_string()),
            description: Some("A new project managed by axes.".to_string()),
            commands: HashMap::new(),
            options: OptionsConfig {
                open_with: open_with_defaults,
                at_start: None,
                at_exit: None,
                shell: None,
            },
            ..Default::default()
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
    pub commands: HashMap<String, Command>,
    pub options: OptionsConfig,
    pub vars: HashMap<String, String>,
    pub env: HashMap<String, String>,
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
    pub commands: HashMap<String, SerializableCommand>,
    pub options: OptionsConfig,
    pub vars: HashMap<String, String>,
    pub env: HashMap<String, String>,
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
            commands: value
                .commands
                .iter()
                .map(|(k, v)| (k.clone(), v.into()))
                .collect(),
            options: value.options.clone(),
            vars: value.vars.clone(),
            env: value.env.clone(),
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
            commands: value
                .commands
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
            options: value.options,
            vars: value.vars,
            env: value.env,
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
