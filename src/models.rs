//! # Data Models
//!
//! This module defines the core data structures used throughout the application, organized into four main categories:
//!
//! 1.  **User-Facing TOML Syntax Models**: These structs (`ProjectConfig`, `TomlScript`, `TomlVar`, etc.)
//!     are designed for flexibility and ergonomics, directly mapping to the syntax a user can write in an
//!     `axes.toml` file. They use enums and `#[serde(untagged)]` to allow for multiple ways of defining
//!     the same logical entity (e.g., a script can be a simple string, a sequence of commands, or a
//!     table with a description).
//!
//! 2.  **Platform-Agnostic AST & Runtime Models**: After the initial parsing, the user-facing models are
//!     compiled into a stricter, more efficient Abstract Syntax Tree (AST). Structs like `Task`, `CachedVar`,
//!     and `CommandExecution` represent the compiled, platform-agnostic logic. This is the representation
//!     that gets stored in the binary cache and is used for most of the program's internal logic.
//!
//! 3.  **Persistence & System Models**: These structs (`GlobalIndex`, `IndexEntry`, `ProjectRef`) are used for
//!     serialization to disk. They represent the application's persistent state, such as the list of all
//!     registered projects and their metadata. They are optimized for fast loading and saving via `bincode`.
//!
//! 4.  **High-Level Runtime Models**: At runtime, the application uses high-level facade models like
//!     `ResolvedConfig` to provide an intelligent, lazy-loaded view of a project's entire configuration,
//!     including inherited settings. These models handle the on-demand loading and caching of configuration
//!     layers to ensure high performance.
//!
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use uuid::Uuid;

// --- Core Concurrency & State Types ---

/// The result of loading and compiling a single configuration layer.
pub type LayerResult = Result<Arc<CachedProjectConfig>>;

/// A thread-safe, one-time settable container for a future `LayerResult`.
/// This acts as a "promise" that consumers can wait on until the layer is loaded by a worker thread.
pub type LayerPromise = Arc<OnceLock<LayerResult>>;

/// A data structure to securely pass updates for the `GlobalIndex` from worker threads
/// back to the main thread for sequential application. This is created on a cache miss.
#[derive(Debug, Clone)]
pub struct IndexUpdate {
    /// The UUID of the project whose cache information needs updating.
    pub uuid: Uuid,
    /// The new BLAKE3 content hash of the project's `axes.toml` file.
    pub new_hash: String,
    /// The newly resolved cache directory path for the project.
    pub new_cache_dir: PathBuf,
}

// =========================================================================
// === 1. USER-FACING TOML SYNTAX MODELS (V0.3 Architecture)
// =========================================================================
// These structs are designed for maximum flexibility, defining the ergonomic
// syntax a user can write in an `axes.toml` file.

/// Represents a platform-specific dictionary for a command or a variable's value.
/// `deny_unknown_fields` ensures that typos in keys (e.g., `defalt = "..."`) are caught as errors.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct PlatformCommand {
    /// The default command, used if no platform-specific version is available.
    #[serde(default)]
    pub default: Option<String>,
    /// The command to run specifically on Windows.
    #[serde(default)]
    pub windows: Option<String>,
    /// The command to run specifically on Linux.
    #[serde(default)]
    pub linux: Option<String>,
    /// The command to run specifically on macOS.
    #[serde(default)]
    pub macos: Option<String>,
}

/// Represents a single command line within a script's sequence.
/// It can be a simple string (which may include `axes` prefixes) or a platform-specific block.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum TomlCommand {
    /// A single command string, e.g., `"echo 'hello'"`
    Simple(String),
    /// A platform-specific block, e.g., `{ windows = "dir", default = "ls" }`
    Platform(PlatformCommand),
}

/// Represents an extended script definition with a description.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TomlScriptExtended {
    /// A user-facing description of what the script does.
    pub desc: Option<String>,
    /// The command(s) to run, which can be any `TomlScript` variant.
    pub run: Box<TomlScript>,
}

/// Represents a direct platform-specific script definition with a description.
/// This allows syntax like `[scripts.build] desc = "..." windows = "..."`.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TomlScriptPlatformDirect {
    /// A user-facing description of what the script does.
    pub desc: Option<String>,
    /// The platform-specific commands.
    #[serde(flatten)]
    pub platform: PlatformCommand,
}

/// Represents the flexible syntax for a script in `axes.toml`.
/// This enum allows a script to be defined as a single command, a sequence, or an extended table.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum TomlScript {
    /// A single command string: `script = "..."`
    Simple(String),
    /// A sequence of commands: `script = [ "...", { ... } ]`
    Sequence(Vec<TomlCommand>),
    /// A platform-specific block: `script = { windows = "...", ... }`
    Platform(PlatformCommand),
    /// A table with description and platform keys: `[scripts.my_script]`
    PlatformDirect(TomlScriptPlatformDirect),
    /// A table with description and a nested `run` key: `[scripts.my_script]`
    Extended(TomlScriptExtended),
}

/// Represents the value part of a variable definition in `axes.toml`.
/// A variable's value can be a simple string or a platform-specific block, but not a sequence.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum TomlVarValue {
    /// A single var string: var = "..."
    Simple(String),
    /// A platform-specific block: `var = { windows = "...", ... }`
    Platform(PlatformCommand),
}

/// Represents an extended variable definition with a description and value.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TomlVarExtended {
    /// A user-facing description of the variable.
    pub desc: Option<String>,
    /// The value of the variable.
    pub value: TomlVarValue,
}

/// Represents the flexible syntax for a variable in `axes.toml`.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum TomlVar {
    /// A simple string value: `my_var = "..."`
    Simple(String),
    /// An extended table with description and value: `[vars.my_var]`
    Extended(TomlVarExtended),
}

/// Represents the user-configurable `[options.open_with]` table in `axes.toml`.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct TomlOpenWithConfig {
    /// The key of the default command to use for `axes open`.
    #[serde(default)]
    pub default: Option<String>,
    /// A map of custom commands, where keys are identifiers (e.g., "editor") and values are scripts.
    #[serde(flatten)]
    pub commands: HashMap<String, TomlScript>,
}

/// Represents the `[options]` section in `axes.toml`.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct OptionsConfig {
    /// A script to run automatically when entering a project session via `axes start`.
    #[serde(default)]
    pub at_start: Option<TomlScript>,
    /// A script to run automatically when exiting a project session.
    #[serde(default)]
    pub at_exit: Option<TomlScript>,
    /// The name of the shell (from `shells.toml`) to use for `axes start`.
    #[serde(default)]
    pub shell: Option<String>,
    /// Configuration for the `axes open` command.
    #[serde(default)]
    pub open_with: TomlOpenWithConfig,
    /// A template for the root directory where project caches are stored.
    #[serde(default)]
    pub cache_dir: Option<String>,
    /// A template for the shell prompt used inside an `axes start` session.
    #[serde(default)]
    pub prompt: Option<String>,
}

/// Represents the direct, top-level structure of an `axes.toml` file.
/// This is the entry point for deserialization by the compiler.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    /// The simple name of the project. Primarily for informational purposes.
    #[serde(default)]
    pub name: Option<String>,
    /// The version of the project. Can be interpolated via the `<version>` token.
    #[serde(default)]
    pub version: Option<String>,
    /// A short description of the project.
    #[serde(default)]
    pub description: Option<String>,
    /// A map of named scripts the user can run.
    #[serde(default)]
    pub scripts: HashMap<String, TomlScript>,
    /// Global options for the project's behavior.
    #[serde(default)]
    pub options: OptionsConfig,
    /// A map of user-defined variables for interpolation.
    #[serde(default)]
    pub vars: HashMap<String, TomlVar>,
    /// A map of environment variables to set during script execution or sessions.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

// =========================================================================
// === 2. PLATFORM-AGNOSTIC AST & RUNTIME MODELS (V0.3 Architecture)
// =========================================================================
// These are the primary internal structs used by the program logic after
// the `axes.toml` has been compiled. They are optimized for performance
// and binary serialization.

// --- Parameter & Token Models ---

/// Defines the contract for a parameter token (`<params::...>`) found in a script.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParameterDef {
    /// The kind of parameter (positional or named).
    pub kind: ParameterKind,
    /// Modifiers that control the parameter's behavior (e.g., required, default value).
    pub modifiers: ParameterModifiers,
    /// The original, full token string as it appeared in the config (e.g., `<params::0(required)>`).
    pub original_token: String,
}

/// Distinguishes between positional (`<params::0>`) and named (`<params::name>`) parameters.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParameterKind {
    /// A parameter identified by its zero-based position in the argument list.
    Positional {
        /// The index of the positional argument.
        index: usize,
    },
    /// A parameter identified by a name, usually corresponding to a CLI flag (e.g., `--name`).
    Named {
        /// The long name of the flag (without the `--` prefix).
        name: String,
    },
}

/// Defines the modifiers for a parameter (e.g., `required`, `default`, `alias`).
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ParameterModifiers {
    /// If true, the parameter must be provided by the user.
    pub required: bool,
    /// A default value to use if the parameter is not provided.
    pub default_value: Option<String>,
    /// An alternative name for a named parameter (e.g., `-n` as an alias for `--name`).
    pub alias: Option<String>,
    /// A string to prepend to the parameter's value, used for formatting flags.
    pub map: Option<String>,
    /// If true, the parameter's value will be wrapped in quotes and escaped.
    pub literal: bool,
}

/// Represents a dynamic execution token (`<run(...)>`).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RunSpec {
    /// A literal shell command to execute, e.g., `<run('git rev-parse --short HEAD')>`.
    Literal(String),
}

/// An enum representing all possible token types that can appear in a command string.
/// This is a fundamental part of the compiled AST.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TemplateComponent {
    /// A static, literal part of the command string.
    Literal(String),
    /// A user-defined parameter from the CLI, e.g., `<params::0>`.
    Parameter(ParameterDef),
    /// A token that expands to all unconsumed CLI arguments, e.g., `<params>`.
    GenericParams {
        /// If true, each argument is individually quoted.
        literal: bool,
    },
    /// A command to execute whose output is substituted in place, e.g., `<run('...')>`.
    Run(RunSpec),
    /// A token that expands to the project's root filesystem path.
    Path,
    /// A token that expands to the project's simple name.
    Name,
    /// A token that expands to the project's UUID.
    Uuid,
    /// A token that expands to the project's version string.
    Version,
    /// An ANSI color/style code, e.g., `<#red>`.
    Color(AnsiStyle),
    /// A reference to another script, e.g., `<scripts::build>`.
    Script(String),
    /// A reference to a variable, e.g., `<vars::my_var>`.
    Var(String),
}

/// Represents the specific action for a single line in a script (execute vs. print).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CommandAction {
    /// Execute a shell command composed of token components.
    Execute(Vec<TemplateComponent>),
    /// Print a string composed of token components directly to the console.
    Print(Vec<TemplateComponent>),
}

/// Represents a single, fully compiled command line, including its action and execution modifiers.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[non_exhaustive]
pub struct CommandExecution {
    /// The action to perform (execute a command or print text).
    pub action: CommandAction,
    /// If true, the command's failure will not halt script execution.
    pub ignore_errors: bool,
    /// If true, this command can be run in parallel with subsequent parallel commands.
    pub run_in_parallel: bool,
    /// If true, the command itself will not be printed to the console before execution.
    pub silent_mode: bool,
}

// --- New Platform-Agnostic AST Models ---

/// The core building block of the AST. It holds a fully compiled `CommandExecution`
/// for each potential platform, ready for fast runtime selection.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PlatformExecution {
    /// The default execution, used as a fallback.
    pub default: Option<CommandExecution>,
    /// The Windows-specific execution.
    pub windows: Option<CommandExecution>,
    /// The Linux-specific execution.
    pub linux: Option<CommandExecution>,
    /// The macOS-specific execution.
    pub macos: Option<CommandExecution>,
}

/// The platform-agnostic AST representation of a script.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Task {
    /// The user-facing description of the script.
    pub desc: Option<String>,
    /// A sequence of platform-aware commands that compose the script.
    pub commands: Vec<PlatformExecution>,
}

/// Represents a `Task` that has been "specialized" for the current platform.
/// It contains a simple, flat list of commands to be executed, removing the need
/// for runtime platform selection in the hot loop of the executor.
#[derive(Debug, Clone, Default)]
pub struct PlatformSpecializedTask {
    /// The user-facing description, inherited from the original `Task`.
    pub desc: Option<String>,
    /// The flattened, platform-specific list of commands to execute.
    pub commands: Vec<CommandExecution>,
}

/// The platform-agnostic AST representation of a variable.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CachedVar {
    /// The user-facing description of the variable.
    pub desc: Option<String>,
    /// The platform-aware value of the variable.
    pub value: PlatformExecution,
}

// --- Cache & Resolved Config Models ---

/// Bincode-compatible representation of `[options.open_with]` in the binary cache.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CachedOpenWithConfig {
    /// The key of the default `open` command.
    pub default: Option<String>,
    /// The compiled map of available `open` commands.
    pub commands: HashMap<String, Task>,
}

/// Bincode-compatible representation of `[options]` in the binary cache.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CachedOptionsConfig {
    /// The compiled `at_start` hook.
    pub at_start: Option<Task>,
    /// The compiled `at_exit` hook.
    pub at_exit: Option<Task>,
    /// The name of the shell to use.
    pub shell: Option<String>,
    /// The compiled `open_with` configuration.
    #[serde(default)]
    pub open_with: CachedOpenWithConfig,
    /// The configured cache directory path template.
    #[serde(default)]
    pub cache_dir: Option<String>,
    /// The configured shell prompt template.
    #[serde(default)]
    pub prompt: Option<String>,
}

/// Represents the pre-compiled content of a single `axes.toml` file.
/// This is the unit that is stored in the binary cache file.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CachedProjectConfig {
    /// The project's version string.
    pub version: Option<String>,
    /// The project's description.
    pub description: Option<String>,
    /// The compiled map of scripts.
    pub scripts: HashMap<String, Task>,
    /// The compiled map of variables.
    pub vars: HashMap<String, CachedVar>,
    /// The map of environment variables.
    pub env: HashMap<String, String>,
    /// The compiled options configuration.
    pub options: CachedOptionsConfig,
}

// --- High-Level Runtime Models ---

/// A runtime representation of resolved `open_with` options, using `Arc` for efficient sharing.
#[derive(Debug, Clone, Default)]
pub struct ResolvedOpenWithConfig {
    /// The final, inherited key for the default `open` command.
    pub default: Option<String>,
    /// The final, merged map of available `open` commands.
    pub commands: HashMap<String, Arc<Task>>,
}

/// A runtime representation of all resolved `[options]`, using `Arc` for efficient sharing.
#[derive(Debug, Clone, Default)]
pub struct ResolvedOptionsConfig {
    /// The final, inherited `at_start` hook.
    pub at_start: Option<Arc<Task>>,
    /// The final, inherited `at_exit` hook.
    pub at_exit: Option<Arc<Task>>,
    /// The final, inherited shell name.
    pub shell: Option<String>,
    /// The final, merged `open_with` configuration.
    pub open_with: ResolvedOpenWithConfig,
    /// The final, absolute path to this project's cache directory.
    pub cache_dir: Option<String>,
    /// The final, inherited prompt template.
    pub prompt: Option<String>,
}

/// A thread-safe, shareable container for a fully merged environment map.
type MemoizedEnv = Arc<HashMap<String, String>>;

/// A generic, thread-safe, lockable container for a memoized (lazily computed) value.
type Memoized<T> = Arc<Mutex<Option<T>>>;

/// An intelligent facade (`Facade Pattern`) that provides access to a project's full, inherited configuration.
/// It loads and merges configuration layers from the inheritance chain on-demand and caches the results in memory.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// The unique identifier for this project.
    pub uuid: Uuid,
    /// The fully-qualified, slash-separated name of the project (e.g., `my-app/backend`).
    pub qualified_name: String,
    /// The absolute path to the project's root directory on the filesystem.
    pub project_root: PathBuf,
    /// An ordered list of UUIDs representing the inheritance chain, from self to the root.
    pub(crate) hierarchy: Arc<Vec<Uuid>>,
    /// A map of promises for each layer's compiled configuration, used for parallel loading.
    pub(crate) layers: Arc<HashMap<Uuid, LayerPromise>>,
    /// A memoization cache for resolved scripts.
    memoized_scripts: Memoized<HashMap<String, Option<Arc<Task>>>>,
    /// A memoization cache for resolved variables.
    memoized_vars: Memoized<HashMap<String, Option<Arc<CachedVar>>>>,
    /// A memoization cache for the merged environment.
    memoized_env: Memoized<MemoizedEnv>,
    /// A memoization cache for the resolved version string.
    memoized_version: Memoized<Option<String>>,
    /// A memoization cache for the resolved description.
    memoized_description: Memoized<Option<String>>,
    /// A memoization cache for the merged options configuration.
    memoized_options: Memoized<ResolvedOptionsConfig>,
}

impl ResolvedConfig {
    /// Creates a new lazy facade, ready to resolve data from the provided layer promises.
    /// # Arguments
    ///
    /// * `uuid` - The UUID of the project.
    /// * `qualified_name` - The fully qualified name of the project.
    /// * `project_root` - The root directory of the project.
    /// * `hierarchy` - The hierarchy of the project, from the project itself to the root.
    /// * `layers` - The layers of the project, which are promises that will be resolved to the actual
    ///   configuration.
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
            memoized_scripts: Default::default(),
            memoized_vars: Default::default(),
            memoized_env: Default::default(),
            memoized_version: Default::default(),
            memoized_description: Default::default(),
            memoized_options: Default::default(),
        }
    }

    // --- LAZY ACCESSOR METHODS (FULLY IMPLEMENTED FOR V0.3) ---

    /// Lazily finds and returns a script's AST by name, searching up the inheritance chain.
    ///
    /// It traverses from the current project upwards through its parents. The first script found
    /// with the matching name is returned. Results are memoized for subsequent calls.
    ///
    /// # Arguments
    /// * `name` - The simple name of the script to find.
    pub fn get_script(&self, name: &str) -> Result<Option<Arc<Task>>> {
        let mut guard = self.memoized_scripts.lock().unwrap();
        if let Some(cache) = &*guard
            && let Some(cached_result) = cache.get(name)
        {
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
        guard
            .get_or_insert_with(Default::default)
            .insert(name.to_string(), result.clone());
        Ok(result)
    }

    /// Lazily finds and returns a variable's AST by name, searching up the inheritance chain.
    ///
    /// It traverses from the current project upwards. The first variable found with the matching
    /// name is returned. Results are memoized.
    ///
    /// # Arguments
    /// * `name` - The name of the variable to find.
    pub fn get_var(&self, name: &str) -> Result<Option<Arc<CachedVar>>> {
        let mut guard = self.memoized_vars.lock().unwrap();
        if let Some(cache) = &*guard
            && let Some(cached_result) = cache.get(name)
        {
            return Ok(cached_result.clone());
        }

        let mut result = None;
        for &uuid in self.hierarchy.iter() {
            let layer = self.get_layer(uuid)?;
            if let Some(var) = layer.vars.get(name) {
                result = Some(Arc::new(var.clone()));
                break;
            }
        }
        guard
            .get_or_insert_with(Default::default)
            .insert(name.to_string(), result.clone());
        Ok(result)
    }

    /// Lazily merges and returns all environment variables from the entire hierarchy.
    /// The result is cached in an Arc for extremely fast subsequent calls.
    pub fn get_env(&self) -> Result<MemoizedEnv> {
        let mut guard = self.memoized_env.lock().unwrap();
        if let Some(env_arc) = &*guard {
            return Ok(env_arc.clone());
        }
        let mut final_env = HashMap::new();
        // Iterate in reverse to let children override parents
        for &uuid in self.hierarchy.iter().rev() {
            let layer = self.get_layer(uuid)?;
            final_env.extend(layer.env.clone());
        }
        let result_arc = Arc::new(final_env);
        *guard = Some(result_arc.clone());
        Ok(result_arc)
    }

    /// Lazily finds and returns the project's version by searching up the hierarchy.
    /// The first non-empty `version` field found while traversing up the chain is returned.
    pub fn get_version(&self) -> Result<Option<String>> {
        let mut guard = self.memoized_version.lock().unwrap();
        if let Some(version) = &*guard {
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
        *guard = Some(final_version.clone());
        Ok(final_version)
    }

    /// Lazily finds and returns the project's description by searching up the hierarchy.
    /// The first non-empty `description` field found is returned.
    pub fn get_description(&self) -> Result<Option<String>> {
        let mut guard = self.memoized_description.lock().unwrap();
        if let Some(desc) = &*guard {
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
        *guard = Some(final_desc.clone());
        Ok(final_desc)
    }

    /// Lazily merges and returns the final `ResolvedOptionsConfig` from the entire hierarchy.
    /// This method uses a two-pass approach to correctly handle inheritance:
    /// 1. A forward pass (child-to-parent) finds the *first* defined value for options
    ///    that do not merge, like hooks (`at_start`, `at_exit`). The child-most definition wins.
    /// 2. A reverse pass (parent-to-child) merges collections, allowing child definitions
    ///    to override parent definitions (e.g., `open_with` commands, `shell` value).
    pub fn get_options(&self) -> Result<ResolvedOptionsConfig> {
        let mut guard = self.memoized_options.lock().unwrap();
        if let Some(options) = &*guard {
            return Ok(options.clone());
        }

        let mut final_options = ResolvedOptionsConfig::default();
        let mut cache_dir_template: Option<String> = None;
        let mut at_start_found = false;
        let mut at_exit_found = false;

        // Iterate from child to parent (normal order) to find first-defined hooks
        for &uuid in self.hierarchy.iter() {
            let layer = self.get_layer(uuid)?;
            if cache_dir_template.is_none() && layer.options.cache_dir.is_some() {
                cache_dir_template = layer.options.cache_dir.clone();
            }
            let layer_options = &layer.options;

            if !at_start_found && layer_options.at_start.is_some() {
                final_options.at_start = layer_options.at_start.clone().map(Arc::new);
                at_start_found = true;
            }
            if !at_exit_found && layer_options.at_exit.is_some() {
                final_options.at_exit = layer_options.at_exit.clone().map(Arc::new);
                at_exit_found = true;
            }
        }

        // --- Resolve the cache_dir template ---
        let final_cache_root_string = match cache_dir_template {
            Some(template) => {
                // expand_path_template returns a PathBuf, convert it back to a string for storage.
                let path = crate::core::paths::expand_path_template(&template)?;
                path.to_string_lossy().into_owned()
            }
            None => {
                let path = crate::core::paths::get_default_cache_root()?;
                path.to_string_lossy().into_owned()
            }
        };

        // The final path for THIS project's cache, as a String.
        // We construct a PathBuf temporarily to join, then convert back.
        let final_path_for_project = PathBuf::from(&final_cache_root_string)
            .join("projects")
            .join(self.uuid.to_string());
        final_options.cache_dir = Some(final_path_for_project.to_string_lossy().into_owned());

        // --- Pass 2: Merge overriding values (parent-to-child) ---

        // Iterate in reverse (child overrides parent) for merge-able options
        for &uuid in self.hierarchy.iter().rev() {
            let layer = self.get_layer(uuid)?;
            let layer_options = &layer.options;

            if layer_options.shell.is_some() {
                final_options.shell = layer_options.shell.clone();
            }
            if layer_options.cache_dir.is_some() {
                final_options.cache_dir = layer_options.cache_dir.clone();
            }
            if layer_options.open_with.default.is_some() {
                final_options.open_with.default = layer_options.open_with.default.clone();
            }
            if layer_options.prompt.is_some() {
                final_options.prompt = layer_options.prompt.clone();
            }

            final_options.open_with.commands.extend(
                layer_options
                    .open_with
                    .commands
                    .iter()
                    .map(|(k, v)| (k.clone(), Arc::new(v.clone()))),
            );
        }

        *guard = Some(final_options.clone());
        Ok(final_options)
    }

    // --- Helpers for `info` and `run` commands ---

    /// Lazily merges and returns all scripts from the entire hierarchy.
    /// This is used by commands like `axes run` (no script) to list all available scripts.
    /// Child definitions override parent definitions.
    pub fn get_all_scripts(&self) -> Result<HashMap<String, Arc<Task>>> {
        let mut final_scripts = HashMap::new();
        for &uuid in self.hierarchy.iter().rev() {
            let layer = self.get_layer(uuid)?;
            for (name, task) in layer.scripts.iter() {
                // Child definitions override parent ones.
                final_scripts.insert(name.clone(), Arc::new(task.clone()));
            }
        }
        Ok(final_scripts)
    }

    /// Lazily merges and returns all variables from the entire hierarchy.
    /// This is used by `axes info` to display all defined variables.
    /// Child definitions override parent definitions.
    pub fn get_all_vars(&self) -> Result<HashMap<String, Arc<CachedVar>>> {
        let mut final_vars = HashMap::new();
        for &uuid in self.hierarchy.iter().rev() {
            let layer = self.get_layer(uuid)?;
            for (name, var) in layer.vars.iter() {
                final_vars.insert(name.clone(), Arc::new(var.clone()));
            }
        }
        Ok(final_vars)
    }

    /// Selects the correct `CommandExecution` for the current OS from a `PlatformExecution` block.
    /// The selection logic prefers the specific OS version and falls back to `default`.
    ///
    /// # Arguments
    /// * `plat_exec` - The platform-agnostic execution block to select from.
    pub fn select_platform_exec<'a>(
        &self,
        plat_exec: &'a PlatformExecution,
    ) -> Option<&'a CommandExecution> {
        let os = std::env::consts::OS;
        if os == "windows" {
            plat_exec.windows.as_ref().or(plat_exec.default.as_ref())
        } else if os == "linux" {
            plat_exec.linux.as_ref().or(plat_exec.default.as_ref())
        } else if os == "macos" {
            plat_exec.macos.as_ref().or(plat_exec.default.as_ref())
        } else {
            plat_exec.default.as_ref()
        }
    }

    // --- Private Core Helper ---

    /// Core helper to get a layer. It waits on the promise to be resolved by the `ConfigLoader`.
    pub(crate) fn get_layer(&self, uuid: Uuid) -> Result<Arc<CachedProjectConfig>> {
        let promise = self.layers.get(&uuid).ok_or_else(|| {
            anyhow!(
                "Internal logic error: attempt to get a layer for UUID {} not in the hierarchy.",
                uuid
            )
        })?;
        let layer_result = promise.get().ok_or_else(|| {
            anyhow!(
                "Internal logic error: LayerPromise for UUID {} was never set.",
                uuid
            )
        })?;
        match layer_result {
            Ok(layer_arc) => Ok(layer_arc.clone()),
            Err(e) => Err(anyhow!(e.to_string())),
        }
    }

    // --- TASK FLATTENING LOGIC ---

    /// This function iterates through the `PlatformExecution` blocks of a universal task
    /// and selects the appropriate `CommandExecution` for the current operating system.
    /// The result is a flat, simple list of commands, ready for the fastest possible execution,
    /// eliminating the need for platform checks in the executor's hot loop.
    pub fn specialize_task_for_platform(
        &self,
        universal_task: &Arc<Task>,
    ) -> PlatformSpecializedTask {
        let specialized_commands: Vec<CommandExecution> = universal_task
            .commands
            .iter()
            .filter_map(|plat_exec| self.select_platform_exec(plat_exec).cloned())
            .collect();

        PlatformSpecializedTask {
            desc: universal_task.desc.clone(),
            commands: specialized_commands,
        }
    }

    /// Recursively flattens a `Task` by resolving all `scripts::` compositions.
    ///
    /// This process expands script references into a single, linear sequence of commands,
    /// while also propagating execution modifiers (e.g., `@`, `-`) from the parent
    /// script to the composed child commands. It also detects circular dependencies.
    ///
    /// # Arguments
    /// * `top_level_task` - The initial task to flatten.
    pub fn flatten_task(&self, top_level_task: &Arc<Task>) -> Result<Arc<Task>> {
        let mut call_stack = HashSet::new();
        self.flatten_task_recursive(top_level_task, &mut call_stack)
    }

    fn flatten_task_recursive(
        &self,
        task: &Arc<Task>,
        call_stack: &mut HashSet<String>,
    ) -> Result<Arc<Task>> {
        let mut new_commands = Vec::new();
        for plat_exec in &task.commands {
            let maybe_composition =
                self.select_platform_exec(plat_exec)
                    .and_then(|cmd_exec| match &cmd_exec.action {
                        CommandAction::Execute(tpl) if tpl.len() == 1 => Some((&tpl[0], cmd_exec)),
                        _ => None,
                    });
            if let Some((TemplateComponent::Script(name), parent_cmd_exec)) = maybe_composition {
                let key = format!("script::{}", name);
                if !call_stack.insert(key.clone()) {
                    return Err(anyhow!(
                        "Circular dependency detected involving script: '{}'",
                        name
                    ));
                }
                let sub_task = self.get_script(name)?.ok_or_else(|| {
                    anyhow!("Broken Reference: Script '<scripts::{}>' not found.", name)
                })?;
                let flattened_sub_task = self.flatten_task_recursive(&sub_task, call_stack)?;
                let mut inherited_commands = flattened_sub_task.commands.clone();
                for sub_plat_exec in &mut inherited_commands {
                    for cmd_exec in [
                        &mut sub_plat_exec.default,
                        &mut sub_plat_exec.windows,
                        &mut sub_plat_exec.linux,
                        &mut sub_plat_exec.macos,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        cmd_exec.ignore_errors |= parent_cmd_exec.ignore_errors;
                        cmd_exec.run_in_parallel |= parent_cmd_exec.run_in_parallel;
                        cmd_exec.silent_mode |= parent_cmd_exec.silent_mode;
                    }
                }
                new_commands.extend(inherited_commands);
                call_stack.remove(&key);
            } else {
                let new_plat_exec = PlatformExecution {
                    default: self
                        .flatten_command_exec_recursive(plat_exec.default.as_ref(), call_stack)?,
                    windows: self
                        .flatten_command_exec_recursive(plat_exec.windows.as_ref(), call_stack)?,
                    linux: self
                        .flatten_command_exec_recursive(plat_exec.linux.as_ref(), call_stack)?,
                    macos: self
                        .flatten_command_exec_recursive(plat_exec.macos.as_ref(), call_stack)?,
                };
                new_commands.push(new_plat_exec);
            }
        }
        Ok(Arc::new(Task {
            commands: new_commands,
            desc: task.desc.clone(),
        }))
    }

    fn flatten_command_exec_recursive(
        &self,
        cmd_exec: Option<&CommandExecution>,
        call_stack: &mut HashSet<String>,
    ) -> Result<Option<CommandExecution>> {
        if let Some(cmd) = cmd_exec {
            let mut new_cmd = cmd.clone();
            let (new_action, template) = match &cmd.action {
                CommandAction::Execute(tpl) => (CommandAction::Execute(Vec::new()), tpl),
                CommandAction::Print(tpl) => (CommandAction::Print(Vec::new()), tpl),
            };
            let flattened_template = self.flatten_template_recursive(template, call_stack)?;
            new_cmd.action = match new_action {
                CommandAction::Execute(_) => CommandAction::Execute(flattened_template),
                CommandAction::Print(_) => CommandAction::Print(flattened_template),
            };
            Ok(Some(new_cmd))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn flatten_template_recursive(
        &self,
        template: &[TemplateComponent],
        call_stack: &mut HashSet<String>,
    ) -> Result<Vec<TemplateComponent>> {
        let mut final_components = Vec::new();
        for component in template {
            match component {
                TemplateComponent::Script(name) | TemplateComponent::Var(name) => {
                    let is_var = matches!(component, TemplateComponent::Var(_));
                    let (token_type, key) = if is_var {
                        ("var", format!("var::{}", name))
                    } else {
                        ("script", format!("script::{}", name))
                    };
                    if !call_stack.insert(key.clone()) {
                        return Err(anyhow!(
                            "Circular dependency detected involving {}: '{}'",
                            token_type,
                            name
                        ));
                    }
                    if is_var {
                        let var = self.get_var(name)?.ok_or_else(|| {
                            anyhow!("Broken Reference: Var '<vars::{}>' not found.", name)
                        })?;
                        if let Some(cmd_exec) = self.select_platform_exec(&var.value) {
                            let (CommandAction::Execute(tpl) | CommandAction::Print(tpl)) =
                                &cmd_exec.action;
                            final_components
                                .extend(self.flatten_template_recursive(tpl, call_stack)?);
                        } else {
                            return Err(anyhow!(
                                "Var '{}' has no value for the current platform.",
                                name
                            ));
                        }
                    } else {
                        let script = self.get_script(name)?.ok_or_else(|| {
                            anyhow!("Broken Reference: Script '<scripts::{}>' not found.", name)
                        })?;
                        if script.commands.len() > 1 {
                            return Err(anyhow!(
                                "Inline composition of multi-line script '{}' is not supported.",
                                name
                            ));
                        }
                        if let Some(plat_exec) = script.commands.first()
                            && let Some(cmd_exec) = self.select_platform_exec(plat_exec)
                        {
                            let (CommandAction::Execute(tpl) | CommandAction::Print(tpl)) =
                                &cmd_exec.action;
                            final_components
                                .extend(self.flatten_template_recursive(tpl, call_stack)?);
                        }
                    }
                    call_stack.remove(&key);
                }
                _ => {
                    final_components.push(component.clone());
                }
            }
        }
        Ok(final_components)
    }
}

// =========================================================================
// === 3. PERSISTENCE & SYSTEM MODELS
// =========================================================================

/// Represents a project's entry in the global `index.bin` file.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexEntry {
    /// The simple, non-unique name of the project (e.g., "backend").
    pub name: String,
    /// The absolute path to the project's root directory on the filesystem.
    pub path: PathBuf,
    /// The UUID of the parent project. `None` only for the virtual "global" project.
    pub parent: Option<Uuid>,
    /// The last known content hash of the project's `axes.toml` file, used for caching.
    pub config_hash: Option<String>,
    /// The last known absolute path to the project's cache directory.
    pub cache_dir: Option<PathBuf>,
    /// The UUID of the direct child that was most recently used as a context.
    pub last_used_child: Option<Uuid>,
}

/// Represents the global index, the single source of truth for all registered projects.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct GlobalIndex {
    /// A map from a project's unique `Uuid` to its metadata.
    #[serde(default)]
    pub projects: HashMap<Uuid, IndexEntry>,
    /// A map from a user-defined alias (e.g., "backend") to a project's `Uuid`.
    #[serde(default)]
    pub aliases: HashMap<String, Uuid>,
    /// The `Uuid` of the project that was most recently used as a context.
    pub last_used: Option<Uuid>,
}

/// Represents a project's local identity file (`.axes/project_ref.bin`).
/// This file makes the system resilient and self-repairing by storing the project's
/// core identity independently of the global index.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct ProjectRef {
    /// The project's own unique identifier.
    pub self_uuid: Uuid,
    /// The UUID of the project's parent at the time of writing.
    pub parent_uuid: Option<Uuid>,
    /// The project's simple name at the time of writing.
    pub name: String,
}

/// Represents a configured shell in `shells.toml`.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ShellConfig {
    /// The path to the shell executable.
    pub path: PathBuf,
    /// A list of arguments required to start the shell in interactive mode and execute an init script.
    pub interactive_args: Option<Vec<String>>,
}

/// Represents the top-level structure of `shells.toml`.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ShellsConfig {
    /// A map from a shell's name (e.g., "bash", "cmd") to its configuration.
    #[serde(default)]
    pub shells: HashMap<String, ShellConfig>,
}

/// Enum for supported ANSI colors and styles, used by the `<#color>` token.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiStyle {
    // --- Attributes ---
    /// Resets all styles and colors.
    Reset,
    /// Bold/intense text.
    Bold,
    /// Dim/faint text.
    Dim,
    /// Italic text.
    Italic,
    /// Underlined text.
    Underline,

    // --- Standard Colors ---
    /// Black Color
    Black,
    /// Red Color
    Red,
    /// Green Color
    Green,
    /// Yellow Color
    Yellow,
    /// Blue Color
    Blue,
    /// Magenta Color
    Magenta,
    /// Cyan Color
    Cyan,
    /// White Color
    White,

    // --- Bright (Intense) Colors ---
    /// Often rendered as Gray.
    BrightBlack,
    /// Bright version of Red, often used for errors or highlights.
    BrightRed,
    /// Bright version of Green, often used for success messages.
    BrightGreen,
    /// Bright version of Yellow, often used for warnings or emphasis.
    BrightYellow,
    /// Bright version of Blue, often used for informational text.
    BrightBlue,
    /// Bright version of Magenta, often used for accents or special highlights.
    BrightMagenta,
    /// Bright version of Cyan, often used for secondary information.
    BrightCyan,
    /// Bright version of White, often rendered as pure white.
    BrightWhite,
}

// =========================================================================
// === 4. CONSTRUCTORS & IMPLEMENTATIONS
// =========================================================================

impl ProjectConfig {
    /// Creates a new, default ProjectConfig.
    ///
    /// This function is used to generate the initial `axes.toml` for the `global` project
    /// when it's first created. It provides a set of sensible, platform-aware defaults
    /// for common actions, making `axes` useful out-of-the-box.
    pub fn new() -> Self {
        let mut open_with_commands = HashMap::new();

        // --- Editor scripts ---
        open_with_commands.insert(
            "editor".to_string(),
            TomlScript::Simple("<vars::editor_cmd> \"<path>\"".to_string()),
        );
        open_with_commands.insert(
            "idea".to_string(),
            TomlScript::Simple("<vars::idea_cmd> \"<path>\"".to_string()),
        );

        // --- OS-Specific File Explorer scripts ---
        // We use `cfg!` to compile platform-specific defaults, providing
        // a sensible out-of-the-box experience for `axes open`.
        if cfg!(target_os = "windows") {
            open_with_commands.insert(
                "explorer".to_string(),
                TomlScript::Simple("-explorer \"<path>\"".to_string()),
            );
        } else if cfg!(target_os = "macos") {
            open_with_commands.insert(
                "finder".to_string(),
                TomlScript::Simple("open \"<path>\"".to_string()),
            );
        } else {
            // Linux and others
            open_with_commands.insert(
                "files".to_string(),
                TomlScript::Simple("xdg-open \"<path>\"".to_string()),
            );
        }

        // --- Terminal/Shell Command ---
        if cfg!(target_os = "windows") {
            open_with_commands.insert(
                "shell".to_string(),
                TomlScript::Simple("start cmd.exe /K \"cd /D <path>\"".to_string()),
            );
        } else {
            open_with_commands.insert(
                "shell".to_string(),
                TomlScript::Simple("<vars::terminal_cmd>".to_string()),
            );
        }

        let open_with_config = TomlOpenWithConfig {
            default: Some(if cfg!(target_os = "windows") {
                "explorer".to_string()
            } else if cfg!(target_os = "macos") {
                "finder".to_string()
            } else {
                "files".to_string()
            }),
            commands: open_with_commands,
        };

        // --- Default Variables ---
        let mut vars_defaults = HashMap::new();
        vars_defaults.insert(
            "editor_cmd".to_string(),
            TomlVar::Simple("code".to_string()),
        );
        vars_defaults.insert("idea_cmd".to_string(), TomlVar::Simple("idea".to_string()));
        vars_defaults.insert(
            "terminal_cmd".to_string(),
            TomlVar::Simple("gnome-terminal --working-directory=<path>".to_string()),
        );

        Self {
            name: Some("global".to_string()),
            version: Some("0.1.0".to_string()),
            description: Some("The global axes project configuration.".to_string()),
            scripts: HashMap::new(),
            options: OptionsConfig {
                open_with: open_with_config,
                prompt: Some("(axes: <#cyan><name><#reset>) $ ".to_string()),
                ..Default::default()
            },
            vars: vars_defaults,
            env: HashMap::new(),
        }
    }

    /// Creates a minimal yet structurally complete ProjectConfig for `axes init`.
    /// It acts as a scaffold for new projects.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the project.
    /// * `version` - The version of the project.
    /// * `description` - The description of the project.
    pub fn new_for_init(name: &str, version: &str, description: &str) -> Self {
        let mut scripts = HashMap::new();
        let mut vars = HashMap::new();

        // then wrap it in the `TomlScript::Extended` variant.
        let test_script = TomlScript::Extended(TomlScriptExtended {
            desc: Some("Run a simple test echo command.".to_string()),
            run: Box::new(TomlScript::Simple(
                "echo \"✅ Test for '<name>' successful!\"".to_string(),
            )),
        });
        scripts.insert("test".to_string(), test_script);

        vars.insert(
            "GREETING".to_string(),
            TomlVar::Simple("Hello from an axes variable!".to_string()),
        );

        let options = OptionsConfig {
            at_start: Some(TomlScript::Extended(TomlScriptExtended {
                desc: Some(
                    "Commands to run when entering a session (e.g., `source .venv/bin/activate`)"
                        .to_string(),
                ),
                run: Box::new(TomlScript::Simple(
                    "# echo 'Entering session...'".to_string(),
                )),
            })),
            at_exit: Some(TomlScript::Extended(TomlScriptExtended {
                desc: Some(
                    "Commands to run when exiting a session (e.g., `docker-compose down`)"
                        .to_string(),
                ),
                run: Box::new(TomlScript::Simple(
                    "# echo 'Exiting session...'".to_string(),
                )),
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
