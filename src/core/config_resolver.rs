// EN: src/core/config_resolver.rs

use crate::constants::{
    AXES_DIR, CONFIG_CACHE_FILENAME, MAX_RECURSION_DEPTH, PROJECT_CONFIG_FILENAME,
};
use crate::core::parameters;
use crate::models::{
    CacheableValue, CanonicalCommand, Command, CommandExecution, FlattenedCommand, GlobalIndex,
    IndexEntry, ProjectConfig, ResolvedConfig, ResolvedOptionsConfig, Runnable,
    SerializableConfigCache, Task, TemplateComponent,
};
use anyhow::Result;
use bincode::error::DecodeError;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ResolverError {
    #[error("Filesystem Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Error parsing TOML in '{path}': {source}")]
    TomlParse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("Error decoding cache: {0}")]
    BincodeDecode(#[from] bincode::error::DecodeError),
    #[error("Error encoding cache: {0}")]
    BincodeEncode(#[from] bincode::error::EncodeError),
    #[error("Project with UUID '{uuid}' referenced in index not found.")]
    UuidNotFoundInIndex { uuid: Uuid },
    #[error("Configuration file for project '{name}' not found at '{path}'.")]
    ConfigFileNotFound { name: String, path: String },
    #[error("Maximum recursion depth ({depth}) exceeded during static expansion of '{key}'.")]
    MaxRecursionDepth { depth: u32, key: String },
    #[error("Circular dependency detected during expansion: {cycle_path}")]
    CircularDependency { cycle_path: String },
    #[error("Invalid value type '{value_type}' requested for key '{key}'.")]
    InvalidValueType { value_type: String, key: String },
    #[error("Key '{key}' of type '{value_type}' not found in configuration.")]
    ValueNotFound { key: String, value_type: String },
    #[error("Expansion/Parsing Error: {0}")]
    Expansion(#[from] anyhow::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// A command or sequence of commands. Interprets execution prefixes ('>', '-').
    Script,
    /// An interpolable string fragment. Does NOT interpret execution prefixes.
    Variable,
}

type ResolverResult<T> = Result<T, ResolverError>;

// --- PUBLIC API ---

pub fn resolve_config_for_uuid(
    target_uuid: Uuid,
    qualified_name: String,
    index: &GlobalIndex,
) -> ResolverResult<ResolvedConfig> {
    let leaf_entry = index
        .projects
        .get(&target_uuid)
        .ok_or(ResolverError::UuidNotFoundInIndex { uuid: target_uuid })?;
    let config_cache_path = leaf_entry.path.join(AXES_DIR).join(CONFIG_CACHE_FILENAME);

    if let Some(cached_config) =
        read_and_validate_config_cache(&config_cache_path, &qualified_name)?
    {
        log::debug!("Valid configuration cache found for '{}'.", qualified_name);
        return Ok(cached_config);
    }

    log::debug!(
        "Invalid or no config cache found. Resolving '{}' from source...",
        qualified_name
    );
    let inheritance_chain = build_inheritance_chain(target_uuid, index)?;
    let dependencies = get_dependencies_timestamps(&inheritance_chain)?;

    let configs_in_chain: Vec<ProjectConfig> =
        inheritance_chain.into_iter().map(|(_, p)| p).collect();
    let mut resolved_config = merge_chain_into_config(configs_in_chain);

    resolved_config.uuid = target_uuid;
    resolved_config.qualified_name = qualified_name;
    resolved_config.project_root = leaf_entry.path.clone();

    write_config_cache(&config_cache_path, &resolved_config, dependencies)?;
    log::debug!(
        "New raw config cache saved at '{}'.",
        config_cache_path.display()
    );

    Ok(resolved_config)
}

/// Public entry point to resolve a script/var into a `Task`.
/// It mutates the passed `ResolvedConfig` by caching the expanded task.
pub fn resolve_task(config: &mut ResolvedConfig, task_key: &str, kind: ValueKind) -> Result<Task> {
    expand_and_get_task_internal(task_key, kind, config, &mut HashSet::new(), 0)
}

/// The internal recursive engine. It reads from `config`, and if a value is `Raw`,
/// it expands it, MUTATES the `config` to store the `Expanded` version, and returns a clone.
fn expand_and_get_task_internal(
    key: &str,
    kind: ValueKind,
    config: &mut ResolvedConfig,
    recursion_stack: &mut HashSet<String>,
    depth: u32,
) -> Result<Task> {
    log::debug!(
        "{:indent$}Resolving task for '{:?}:{}'",
        "",
        kind,
        key,
        indent = (depth as usize) * 2
    );

    // 1. Clonar el `FlattenedCommand` para liberarnos del préstamo y poder pasar `&mut config` a la recursión.
    let flattened_command = {
        let cacheable = get_cacheable_value(config, key, kind)?;
        match cacheable {
            CacheableValue::Expanded(task) => {
                log::debug!(
                    "{:indent$}Value is EXPANDED. Returning clone from cache.",
                    "",
                    indent = (depth as usize) * 2
                );
                return Ok(task.clone());
            }
            CacheableValue::Raw(fc) => fc.clone(), // Clonamos el FlattenedCommand
        }
    };

    log::debug!(
        "{:indent$}Value is RAW. Preparing for expansion.",
        "",
        indent = (depth as usize) * 2
    );

    // 2. Procesar el `FlattenedCommand` clonado.
    let mut new_task = Task {
        commands: Vec::new(),
        desc: flattened_command.desc,
    };
    for line in &flattened_command.command_lines {
        let (ignore_errors, run_in_parallel, template_str) = if kind == ValueKind::Script {
            parse_execution_prefixes(line)
        } else {
            (false, false, line.as_str())
        };
        let expanded_str = expand_composite_tokens_recursively(
            key,
            kind,
            template_str,
            config,
            recursion_stack,
            depth,
        )?;
        let mut components = parameters::discover_and_parse(&expanded_str)?;
        expand_simple_tokens_in_literals(&mut components, config);

        new_task.commands.push(CommandExecution {
            template: components,
            ignore_errors,
            run_in_parallel,
        });
    }

    // 3. Mutar el `config` para insertar la nueva tarea `Expanded`.
    let cacheable_mut = get_cacheable_value_mut(config, key, kind)?;
    *cacheable_mut = CacheableValue::Expanded(new_task.clone());

    // 4. Devolver la tarea poseída.
    Ok(new_task)
}

/// Helper recursive function to expand composite tokens like `<axes::vars::...>`
fn expand_composite_tokens_recursively(
    key: &str,
    kind: ValueKind,
    raw_string: &str,
    config: &mut ResolvedConfig,
    recursion_stack: &mut HashSet<String>,
    depth: u32,
) -> ResolverResult<String> {
    if depth >= MAX_RECURSION_DEPTH {
        return Err(ResolverError::MaxRecursionDepth {
            depth,
            key: key.to_string(),
        });
    }
    let stack_key = format!("{:?}::{}", kind, key);
    if !recursion_stack.insert(stack_key.clone()) {
        let cycle_path = recursion_stack
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(ResolverError::CircularDependency {
            cycle_path: format!("{} -> {}", cycle_path, stack_key),
        });
    }

    let re = Regex::new(r"<axes::(vars|scripts)::([^>]+)>").unwrap();
    let mut current_str = raw_string.to_string();

    // Loop to handle multiple tokens in the same string
    loop {
        let captures: Vec<_> = re.captures_iter(&current_str).collect();
        if captures.is_empty() {
            break;
        }

        let mut next_str = String::new();
        let mut last_match_end = 0;

        for caps in captures {
            let full_match = caps.get(0).unwrap();
            let namespace = caps.get(1).unwrap().as_str();
            let sub_key = caps.get(2).unwrap().as_str();
            let sub_kind = if namespace == "vars" {
                ValueKind::Variable
            } else {
                ValueKind::Script
            };
            let sub_task = expand_and_get_task_internal(
                sub_key,
                sub_kind,
                config,
                recursion_stack,
                depth + 1,
            )?;

            next_str.push_str(&current_str[last_match_end..full_match.start()]);

            let sub_value_str = sub_task
                .commands
                .iter()
                .map(|cmd_exec| {
                    cmd_exec
                        .template
                        .iter()
                        .map(|c| match c {
                            TemplateComponent::Literal(s) => s.clone(),
                            TemplateComponent::Parameter(p) => p.original_token.clone(),
                            TemplateComponent::GenericParams => "<axes::params>".to_string(),
                        })
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join(" && ");

            next_str.push_str(&sub_value_str);
            last_match_end = full_match.end();
        }
        next_str.push_str(&current_str[last_match_end..]);
        current_str = next_str;
    }

    recursion_stack.remove(&stack_key);
    Ok(current_str)
}

pub fn save_config_cache(config: &ResolvedConfig, index: &GlobalIndex) -> ResolverResult<()> {
    // NOTE: This function is now less critical, as the on-disk cache only stores the `Raw` state.
    // However, it's good practice to have it in case we reintroduce lazy-writing to the cache.
    // For now, we can make it a no-op or simply write the current state. We'll write.
    let entry = index
        .projects
        .get(&config.uuid)
        .ok_or(ResolverError::UuidNotFoundInIndex { uuid: config.uuid })?;
    let cache_path = entry.path.join(AXES_DIR).join(CONFIG_CACHE_FILENAME);
    let inheritance_chain = build_inheritance_chain(config.uuid, index)?;
    let dependencies = get_dependencies_timestamps(&inheritance_chain)?;
    write_config_cache(&cache_path, config, dependencies)
}

// --- TASK EXPANSION ENGINE ---

// --- INHERITANCE AND MERGE LOGIC ---

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            uuid: Uuid::nil(),
            qualified_name: String::new(),
            project_root: PathBuf::new(),
            scripts: HashMap::new(),
            vars: HashMap::new(),
            env: HashMap::new(),
            options: ResolvedOptionsConfig::default(),
            version: None,
            description: None,
        }
    }
}

fn build_inheritance_chain(
    leaf_uuid: Uuid,
    index: &GlobalIndex,
) -> ResolverResult<Vec<(&IndexEntry, ProjectConfig)>> {
    let mut chain = Vec::new();
    let mut current_uuid_opt = Some(leaf_uuid);

    while let Some(current_uuid) = current_uuid_opt {
        let entry = index
            .projects
            .get(&current_uuid)
            .ok_or(ResolverError::UuidNotFoundInIndex { uuid: current_uuid })?;
        let config = load_project_config(entry)?;
        chain.push((entry, config));
        current_uuid_opt = entry.parent;
    }
    chain.reverse();
    Ok(chain)
}

fn merge_chain_into_config(chain: Vec<ProjectConfig>) -> ResolvedConfig {
    let mut resolved = ResolvedConfig::default();

    for config in chain {
        resolved.version = config.version.or(resolved.version);
        resolved.description = config.description.or(resolved.description);
        resolved.env.extend(config.env);

        let os = std::env::consts::OS;

        resolved.scripts.extend(
            config
                .scripts
                .into_iter()
                .map(|(k, v)| (k, flatten_command(v.0, os))),
        );
        resolved.vars.extend(config.vars.into_iter().map(|(k, v)| {
            let cmd = Command(CanonicalCommand {
                default: Some(Runnable::Single(v)),
                ..Default::default()
            });
            (k, flatten_command(cmd.0, os))
        }));

        resolved.options.shell = config.options.shell.or(resolved.options.shell);
        if let Some(cmd) = config.options.at_start {
            resolved.options.at_start = Some(flatten_command(cmd.0, os));
        }
        if let Some(cmd) = config.options.at_exit {
            resolved.options.at_exit = Some(flatten_command(cmd.0, os));
        }
        resolved.options.open_with.extend(
            config
                .options
                .open_with
                .into_iter()
                .map(|(k, v)| (k, flatten_command(v.0, os))),
        );
    }
    resolved
}

fn flatten_command(cmd: CanonicalCommand, os: &str) -> CacheableValue {
    let runnable = if os == "windows" {
        cmd.windows.or(cmd.default)
    } else if os == "linux" {
        cmd.linux.or(cmd.default)
    } else if os == "macos" {
        cmd.macos.or(cmd.default)
    } else {
        cmd.default
    };

    let command_lines = match runnable {
        Some(Runnable::Single(s)) => vec![s],
        Some(Runnable::Sequence(s)) => s,
        None => Vec::new(),
    };

    CacheableValue::Raw(FlattenedCommand {
        command_lines,
        desc: cmd.desc,
    })
}

fn parse_execution_prefixes(line: &str) -> (bool, bool, &str) {
    let mut trimmed_line = line.trim_start();
    let mut ignore_errors = false;
    let mut run_in_parallel = false;

    loop {
        if let Some(rest) = trimmed_line.strip_prefix('-') {
            ignore_errors = true;
            trimmed_line = rest.trim_start();
        } else if let Some(rest) = trimmed_line.strip_prefix('>') {
            run_in_parallel = true;
            trimmed_line = rest.trim_start();
        } else {
            break;
        }
    }
    (ignore_errors, run_in_parallel, trimmed_line)
}

// --- CACHE READ/WRITE LOGIC ---

fn load_project_config(entry: &IndexEntry) -> ResolverResult<ProjectConfig> {
    let config_path = entry.path.join(AXES_DIR).join(PROJECT_CONFIG_FILENAME);
    if !config_path.is_file() {
        return Err(ResolverError::ConfigFileNotFound {
            name: entry.name.clone(),
            path: config_path.display().to_string(),
        });
    }
    let content = fs::read_to_string(&config_path)?;
    toml::from_str(&content).map_err(|e| ResolverError::TomlParse {
        path: config_path.display().to_string(),
        source: e,
    })
}

fn get_dependencies_timestamps(
    inheritance_chain: &[(&IndexEntry, ProjectConfig)],
) -> ResolverResult<HashMap<PathBuf, SystemTime>> {
    inheritance_chain
        .iter()
        .map(|(entry, _)| {
            let config_path = entry.path.join(AXES_DIR).join(PROJECT_CONFIG_FILENAME);
            let metadata = fs::metadata(&config_path)?;
            Ok((config_path, metadata.modified()?))
        })
        .collect()
}

fn read_and_validate_config_cache(
    cache_path: &Path,
    expected_name: &str,
) -> ResolverResult<Option<ResolvedConfig>> {
    if !cache_path.exists() {
        return Ok(None);
    }
    let cached_bytes = match fs::read(cache_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    if cached_bytes.is_empty() {
        return Ok(None);
    }

    let decode_result: Result<(SerializableConfigCache, usize), _> =
        bincode::serde::decode_from_slice(&cached_bytes, bincode::config::standard());

    let serializable_cache = match decode_result {
        Ok((cache, _)) => cache,
        Err(e) => {
            if !matches!(e, DecodeError::Io { .. }) {
                log::warn!(
                    "Config cache at '{}' is corrupt. Regenerating. (Error: {})",
                    cache_path.display(),
                    e
                );
                let _ = fs::remove_file(cache_path);
            }
            return Ok(None);
        }
    };

    if serializable_cache.resolved_config.qualified_name != expected_name {
        log::debug!(
            "Cache is for a different project name ('{}' vs '{}'). Invalid.",
            serializable_cache.resolved_config.qualified_name,
            expected_name
        );
        return Ok(None);
    }

    for (path_str, cached_mod_time_serializable) in serializable_cache.dependencies.iter() {
        let path = PathBuf::from(path_str);
        let current_mod_time = match fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(time) => time,
            Err(_) => {
                log::debug!(
                    "Cache dependency '{}' no longer exists. Cache invalid.",
                    path.display()
                );
                return Ok(None);
            }
        };
        let cached_mod_time: SystemTime = (*cached_mod_time_serializable).into();
        if current_mod_time > cached_mod_time {
            log::debug!(
                "Cache dependency '{}' has been modified. Cache invalid.",
                path.display()
            );
            return Ok(None);
        }
    }
    Ok(Some(serializable_cache.resolved_config.into()))
}

fn write_config_cache(
    cache_path: &Path,
    config: &ResolvedConfig,
    dependencies: HashMap<PathBuf, SystemTime>,
) -> ResolverResult<()> {
    let cache_dir = cache_path.parent().unwrap_or_else(|| Path::new("."));
    if !cache_dir.exists() {
        fs::create_dir_all(cache_dir)?;
    }
    let serializable_deps = dependencies
        .into_iter()
        .map(|(path, time)| (path.to_string_lossy().into_owned(), time.into()))
        .collect();
    let cache_data = SerializableConfigCache {
        resolved_config: config.into(),
        dependencies: serializable_deps,
    };
    let bytes = bincode::serde::encode_to_vec(cache_data, bincode::config::standard())?;
    fs::write(cache_path, &bytes)?;
    Ok(())
}

// ---MARK: HELPERS to access fields in ResolvedConfig ---

/// Retrieves an immutable reference to a `CacheableValue` from the correct map in `ResolvedConfig`.
/// This function centralizes the logic for finding any script-like or var-like value.
fn get_cacheable_value<'a>(
    config: &'a ResolvedConfig,
    key: &str,
    kind: ValueKind,
) -> ResolverResult<&'a CacheableValue> {
    let cacheable = match kind {
        ValueKind::Variable => config.vars.get(key),
        ValueKind::Script => config
            .scripts
            .get(key)
            .or_else(|| {
                config
                    .options
                    .at_start
                    .as_ref()
                    .filter(|_| key == "at_start")
            })
            .or_else(|| config.options.at_exit.as_ref().filter(|_| key == "at_exit"))
            .or_else(|| config.options.open_with.get(key)),
    };

    cacheable.ok_or_else(|| ResolverError::ValueNotFound {
        key: key.to_string(),
        value_type: format!("{:?}", kind),
    })
}

/// Retrieves a mutable reference to a `CacheableValue` from the correct map in `ResolvedConfig`.
fn get_cacheable_value_mut<'a>(
    config: &'a mut ResolvedConfig,
    key: &str,
    kind: ValueKind,
) -> ResolverResult<&'a mut CacheableValue> {
    let cacheable = match kind {
        ValueKind::Variable => config.vars.get_mut(key),
        ValueKind::Script => {
            // Lógica unificada
            if config.scripts.contains_key(key) {
                config.scripts.get_mut(key)
            } else if key == "at_start" {
                config.options.at_start.as_mut()
            } else if key == "at_exit" {
                config.options.at_exit.as_mut()
            } else if config.options.open_with.contains_key(key) {
                config.options.open_with.get_mut(key)
            } else {
                None
            }
        }
    };
    cacheable.ok_or_else(|| ResolverError::ValueNotFound {
        key: key.to_string(),
        value_type: format!("{:?}", kind),
    })
}

/// Expands simple, non-recursive tokens (e.g., `<axes::path>`) on a set of components.
/// This is the final expansion pass performed on literal strings.
fn expand_simple_tokens_in_literals(components: &mut [TemplateComponent], config: &ResolvedConfig) {
    for component in components.iter_mut() {
        if let TemplateComponent::Literal(s) = component {
            // Using `Cow` to avoid multiple allocations if no tokens are found.
            let mut cow = std::borrow::Cow::Borrowed(s.as_str());

            if cow.contains("<axes::path>") {
                cow = std::borrow::Cow::Owned(
                    cow.replace("<axes::path>", &config.project_root.to_string_lossy()),
                );
            }
            if cow.contains("<axes::name>") {
                cow = std::borrow::Cow::Owned(cow.replace("<axes::name>", &config.qualified_name));
            }
            if cow.contains("<axes::uuid>") {
                cow =
                    std::borrow::Cow::Owned(cow.replace("<axes::uuid>", &config.uuid.to_string()));
            }
            if let Some(version) = &config.version
                && cow.contains("<axes::version>")
            {
                cow = std::borrow::Cow::Owned(cow.replace("<axes::version>", version));
            }

            // Only re-assign if changes were actually made.
            if let std::borrow::Cow::Owned(owned_string) = cow {
                *s = owned_string;
            }
        }
    }
}
