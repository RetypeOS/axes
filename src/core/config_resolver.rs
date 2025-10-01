// src/core/config_resolver.rs

use crate::constants::{
    AXES_DIR, CONFIG_CACHE_FILENAME, MAX_RECURSION_DEPTH, PROJECT_CONFIG_FILENAME,
};
use crate::core::parameters::{self};
use crate::models::{
    CacheableValue, CanonicalCommand, Command, CommandExecution, FlattenedCommand, GlobalIndex,
    IndexEntry, ProjectConfig, ResolvedConfig, ResolvedOptionsConfig, RunSpec, Runnable,
    SerializableConfigCache, Task, TemplateComponent,
};
use anyhow::{Context, Result, anyhow};
use bincode::error::DecodeError;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;
use uuid::Uuid;

lazy_static! {
    // A simple, fast regex that finds ANY <axes::...> token.
    // The logic is now in the expansion engine, not the regex.
    static ref TOKEN_RE: Regex = Regex::new(r"<axes::([^>]+)>").unwrap();
}

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
    #[error("Invalid value type '{kind:?}' requested for key '{key}'.")]
    InvalidValueType { kind: ValueKind, key: String },
    #[error("Key '{key}' of type '{kind:?}' not found in configuration.")]
    ValueNotFound { kind: ValueKind, key: String },
    #[error("Expansion/Parsing Error: {0}")]
    Expansion(#[from] anyhow::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hook {
    AtStart,
    AtExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// A command or sequence of commands. Interprets execution prefixes ('>', '-').
    Script,
    /// An interpolable string fragment. Does NOT interpret execution prefixes.
    Variable,
    Hook(Hook),
    OpenWith,
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
pub fn resolve_script_task(
    config: &mut ResolvedConfig,
    task_key: &str,
    index: &GlobalIndex,
) -> Result<Task> {
    let (task, dirty) = expand_and_get_task_internal(task_key, ValueKind::Script, config, 0)?;

    if dirty {
        save_config_cache(config, index)
            .with_context(|| "Failed to save updated configuration cache.")?;
    }

    Ok(task)
}

/// Public entry point to resolve an `open_with` task into a `Task`.
pub fn resolve_open_with_task(
    config: &mut ResolvedConfig,
    task_key: &str,
    index: &GlobalIndex,
) -> Result<Task> {
    let (task, dirty) = expand_and_get_task_internal(task_key, ValueKind::OpenWith, config, 0)?;

    if dirty {
        save_config_cache(config, index)
            .with_context(|| "Failed to save updated configuration cache.")?;
    }

    Ok(task)
}

/// Public entry point to resolve a session hook task (`at_start`/`at_exit`) into a `Task`.
pub fn resolve_hook_task(
    config: &mut ResolvedConfig,
    hook: Hook,
    index: &GlobalIndex,
) -> Result<Option<Task>> {
    // Hooks are optional, so we check for existence first.
    let key = match hook {
        Hook::AtStart => "at_start",
        Hook::AtExit => "at_exit",
    };
    if get_cacheable_value(config, key, ValueKind::Hook(hook))?.is_none() {
        return Ok(None);
    }

    let (task, dirty) = expand_and_get_task_internal(key, ValueKind::Hook(hook), config, 0)?;

    if dirty {
        save_config_cache(config, index)
            .with_context(|| "Failed to save updated configuration cache.")?;
    }

    Ok(Some(task))
}

/// The main orchestrator for the expansion engine.
/// It handles caching, recursion safety, and calls the linear expansion
/// function `expand_line_to_components` for each line of a command.
fn expand_and_get_task_internal(
    key: &str,
    kind: ValueKind,
    config: &mut ResolvedConfig,
    depth: u32,
) -> Result<(Task, bool)> {
    let stack_key = format!("{:?}::{}", kind, key);

    println!("Resolving...: {:?}\ndepth: {:?}\n", stack_key, depth);

    // --- Safety Check: Prevent infinite recursion ---
    if depth >= MAX_RECURSION_DEPTH {
        return Err(ResolverError::MaxRecursionDepth {
            depth,
            key: stack_key,
        }
        .into());
    }

    // --- In-Memory Cache Check ---
    if let Some(CacheableValue::Expanded(task)) = get_cacheable_value(config, key, kind)? {
        return Ok((task.clone(), false));
    }

    // --- Get Raw Value to Expand ---
    let flattened_command = match get_cacheable_value(config, key, kind)? {
        Some(CacheableValue::Raw(fc)) => fc.clone(),
        Some(CacheableValue::Expanded(_)) => unreachable!(), // Handled by cache check above
        None => {
            return Err(ResolverError::ValueNotFound {
                key: key.to_string(),
                kind,
            }
            .into());
        }
    };

    let mut final_task = Task {
        desc: flattened_command.desc,
        ..Default::default()
    };

    // --- Main Expansion Loop for Each Line ---
    for line in &flattened_command.command_lines {
        let (ignore_errors, run_in_parallel, clean_line) = parse_execution_prefixes(line);

        // This is the core logic: expand the line.
        // It will either return a template to be added as a new command,
        // or it will have already modified `final_task` via structural composition.
        let template = expand_line(
            clean_line,
            &mut final_task,
            config,
            depth + 1,
        )?;

        // If the expansion resulted in components (i.e., it wasn't a pure multi-line script),
        // package them into a new CommandExecution.
        if !template.is_empty() {
            final_task.commands.push(CommandExecution {
                template,
                ignore_errors,
                run_in_parallel,
            });
        }
    }

    // --- Optimization and Caching ---
    for cmd in &mut final_task.commands {
        optimize_literals(&mut cmd.template);
    }

    let cacheable_mut = get_cacheable_value_mut(config, key, kind)?.unwrap(); // Safe due to previous checks
    *cacheable_mut = CacheableValue::Expanded(final_task.clone());

    println!("Task: {:?}\n", final_task);
    Ok((final_task, true))
}

/// Expands a single line into a vector of `TemplateComponent`s or structurally
/// composes a multi-line script into the `parent_task`.
fn expand_line(
    line: &str,
    parent_task: &mut Task,
    config: &mut ResolvedConfig,
    depth: u32,
) -> Result<Vec<TemplateComponent>> {
    let mut components = Vec::new();
    let mut last_index = 0;

    let captures: Vec<_> = TOKEN_RE.captures_iter(line).collect();

    // --- Structural Composition Fast Path ---
    // If a line is a single, pure script token, we compose it structurally.
    if captures.len() == 1 && captures[0].get(0).unwrap().as_str() == line {
        let content = captures[0].get(1).unwrap().as_str().trim();
        if let Some(script_key) = content.strip_prefix("scripts::") {
            let (sub_task, _) = expand_and_get_task_internal(
                script_key,
                ValueKind::Script,
                config,
                depth, // Note: depth is passed directly, not incremented here
            )?;
            // Directly extend the parent task with the sub-task's commands.
            parent_task.commands.extend(sub_task.commands);
            // Return an empty template to signify that this line has been fully processed.
            return Ok(Vec::new());
        }
    }

    // --- Inline Expansion Slow Path ---
    // If the line is not a pure script composition, expand all tokens inline.
    for caps in captures {
        let full_match = caps.get(0).unwrap();
        let literal_part = &line[last_index..full_match.start()];
        if !literal_part.is_empty() {
            add_literal(&mut components, literal_part.to_string());
        }

        let content = caps.get(1).unwrap().as_str().trim();

        if let Some(var_key) = content.strip_prefix("vars::") {
            let (var_task, _) = expand_and_get_task_internal(var_key, ValueKind::Variable, config, depth)?;
            if var_task.commands.len() != 1 { return Err(anyhow!("Variable '{}' must expand to a single-line value.", var_key)); }
            // Inline compose the variable's components.
            components.extend(var_task.commands[0].template.clone());

        } else if let Some(script_key) = content.strip_prefix("scripts::") {
            // This is an inline script composition.
            let (script_task, _) = expand_and_get_task_internal(script_key, ValueKind::Script, config, depth)?;
            if script_task.commands.len() > 1 {
                return Err(anyhow!("Inline script composition '<axes::scripts::{}>' is not supported for multi-line scripts. Use it on its own line.", script_key));
            }
            if let Some(cmd) = script_task.commands.first() {
                components.extend(cmd.template.clone());
            }
        } else if let Some(param_spec) = content.strip_prefix("params::") {
            let def = parameters::parse_parameter_token(full_match.as_str(), param_spec)?;
            components.push(TemplateComponent::Parameter(def));
        } else if content == "params" {
            components.push(TemplateComponent::GenericParams);
        } else if let Some(run_spec) = content.strip_prefix("run") {
            if run_spec.starts_with("('") && run_spec.ends_with("')") {
                let cmd = run_spec.strip_prefix("('").unwrap().strip_suffix("')").unwrap();
                components.push(TemplateComponent::Run(RunSpec::Literal(cmd.to_string())));
            } else { return Err(anyhow!("Invalid run syntax: {}", full_match.as_str())); }
        } else {
            // Default case: simple static tokens become enum variants
            let component = match content {
                "path" => TemplateComponent::Path,
                "name" => TemplateComponent::Name,
                "uuid" => TemplateComponent::Uuid,
                "version" => TemplateComponent::Version,
                _ => return Err(anyhow!("Unknown token namespace in: '{}'", full_match.as_str())),
            };
            components.push(component);
        }

        last_index = full_match.end();
    }

    let final_literal = &line[last_index..];
    if !final_literal.is_empty() {
        add_literal(&mut components, final_literal.to_string());
    }

    Ok(components)
}

/// Utility to add a literal and merge it with the previous one if possible.
fn add_literal(components: &mut Vec<TemplateComponent>, s: String) {
    if let Some(TemplateComponent::Literal(last)) = components.last_mut() {
        last.push_str(&s);
    } else {
        components.push(TemplateComponent::Literal(s));
    }
}

/// Merges adjacent `Literal` components in a template for optimization.
fn optimize_literals(template: &mut Vec<TemplateComponent>) {
    if template.is_empty() {
        return;
    }
    let mut i = 0;
    while i < template.len() - 1 {
        if let (Some(TemplateComponent::Literal(s1)), Some(TemplateComponent::Literal(s2))) =
            (template.get(i), template.get(i + 1))
        {
            let merged = format!("{}{}", s1, s2);
            template[i] = TemplateComponent::Literal(merged);
            template.remove(i + 1);
        } else {
            i += 1;
        }
    }
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
) -> Result<Option<&'a CacheableValue>> {
    let cacheable = match kind {
        ValueKind::Variable => config.vars.get(key),
        ValueKind::Script => config.scripts.get(key),
        ValueKind::OpenWith => config.options.open_with.get(key),
        ValueKind::Hook(Hook::AtStart) => config.options.at_start.as_ref(),
        ValueKind::Hook(Hook::AtExit) => config.options.at_exit.as_ref(),
    };

    Ok(cacheable)
}

/// Retrieves a mutable reference to a `CacheableValue` from the correct map in `ResolvedConfig`.
fn get_cacheable_value_mut<'a>(
    config: &'a mut ResolvedConfig,
    key: &str,
    kind: ValueKind,
) -> Result<Option<&'a mut CacheableValue>> {
    let cacheable = match kind {
        ValueKind::Variable => config.vars.get_mut(key),
        ValueKind::Script => config.scripts.get_mut(key),
        ValueKind::OpenWith => config.options.open_with.get_mut(key),
        ValueKind::Hook(Hook::AtStart) => config.options.at_start.as_mut(),
        ValueKind::Hook(Hook::AtExit) => config.options.at_exit.as_mut(),
    };
    Ok(cacheable)
}
