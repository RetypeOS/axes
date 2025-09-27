// src/core/config_resolver.rs

use crate::constants::{AXES_DIR, CONFIG_CACHE_FILENAME, PROJECT_CONFIG_FILENAME};
use crate::core::parameters;
use crate::models::{
    CacheableValue, Command as ProjectCommand, CommandExecution, GlobalIndex, IndexEntry, ProjectConfig, ResolvedConfig, ResolvedOptionsConfig, Runnable, SerializableConfigCache, Task, TemplateComponent
};
use anyhow::{anyhow, Result};
use bincode::error::DecodeError;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;
use uuid::Uuid;

const MAX_RECURSION_DEPTH: u32 = 32;

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

type ResolverResult<T> = Result<T, ResolverError>;
type ExpansionCache = HashMap<String, Task>;

// --- PUBLIC API ---

pub fn resolve_config_for_uuid(
    target_uuid: Uuid,
    qualified_name: String,
    index: &GlobalIndex,
) -> ResolverResult<ResolvedConfig> {
    let leaf_entry = index.projects.get(&target_uuid).ok_or(ResolverError::UuidNotFoundInIndex { uuid: target_uuid })?;
    let config_cache_path = leaf_entry.path.join(AXES_DIR).join(CONFIG_CACHE_FILENAME);

    if let Some(cached_config) = read_and_validate_config_cache(&config_cache_path, &qualified_name)? {
        log::debug!("Valid configuration cache found for '{}'.", qualified_name);
        return Ok(cached_config);
    }

    log::debug!("Invalid or no config cache found. Resolving '{}' from source...", qualified_name);
    let inheritance_chain = build_inheritance_chain(target_uuid, index)?;
    let dependencies = get_dependencies_timestamps(&inheritance_chain)?;

    let configs_in_chain: Vec<ProjectConfig> = inheritance_chain.into_iter().map(|(_, p)| p).collect();
    let mut resolved_config = merge_chain_into_config(configs_in_chain);

    resolved_config.uuid = target_uuid;
    resolved_config.qualified_name = qualified_name;
    resolved_config.project_root = leaf_entry.path.clone();

    write_config_cache(&config_cache_path, &resolved_config, dependencies)?;
    log::debug!("New raw config cache saved at '{}'.", config_cache_path.display());

    Ok(resolved_config)
}

/// Retrieves the fully expanded and parsed `Task` for a given script.
pub fn resolve_task(config: &ResolvedConfig, script_name: &str) -> Result<Task> {
    // 1. Create a fresh, empty cache for this operation.
    let mut cache = ExpansionCache::new();
    
    // 2. Call the internal recursive expander.
    let task = expand_and_get_task_internal(
        script_name,
        "scripts",
        config,
        &mut cache,
        &mut HashSet::new(),
        0,
    )?;

    // 3. Return a clone of the final task. The cache is dropped here.
    Ok(task.clone())
}

pub fn save_config_cache(config: &ResolvedConfig, index: &GlobalIndex) -> ResolverResult<()> {
    let entry = index.projects.get(&config.uuid).ok_or(ResolverError::UuidNotFoundInIndex { uuid: config.uuid })?;
    let cache_path = entry.path.join(AXES_DIR).join(CONFIG_CACHE_FILENAME);
    let inheritance_chain = build_inheritance_chain(config.uuid, index)?;
    let dependencies = get_dependencies_timestamps(&inheritance_chain)?;

    write_config_cache(&cache_path, config, dependencies)
}

// --- LAZY EXPANSION ENGINE ---

/// Expands a single raw string template, resolving static tokens recursively.
fn expand_composite_tokens_recursively(
    key: &str,
    value_type: &str,
    raw_string: &str,
    config: &ResolvedConfig,
    cache: &mut ExpansionCache,
    recursion_stack: &mut HashSet<String>,
    depth: u32,
) -> ResolverResult<String> {
    log::debug!(
        "{:indent$}Expanding static tokens for '{}:{}'",
        "", key, value_type, indent = (depth as usize) * 2
    );
    log::debug!("{:indent$}Raw value: '{}'", "", raw_string, indent = (depth as usize) * 2);

    if depth >= MAX_RECURSION_DEPTH {
        return Err(ResolverError::MaxRecursionDepth { depth, key: key.to_string() });
    }

    let stack_key = format!("{}::{}", value_type, key);
    if !recursion_stack.insert(stack_key.clone()) {
        let cycle_path = recursion_stack.iter().cloned().collect::<Vec<_>>().join(" -> ");
        return Err(ResolverError::CircularDependency { cycle_path: format!("{} -> {}", cycle_path, stack_key) });
    }

    // --- FASE 2: Expandir tokens compuestos y recursivos (vars, scripts) ---
    let re = Regex::new(r"<axes::(vars|scripts)::([^>]+)>").unwrap();
    let mut current_str = raw_string.to_string();

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

            log::debug!(
                "{:indent$}Found sub-token: '{}'. Resolving '{}:{}'.",
                "", full_match.as_str(), namespace, sub_key, indent = (depth as usize) * 2
            );

            next_str.push_str(&current_str[last_match_end..full_match.start()]);
            
            // La llamada recursiva sigue igual, ahora con el caché explícito
            let sub_task = expand_and_get_task_internal(sub_key, namespace, config, cache, recursion_stack, depth + 1)?;
            let sub_value_str = sub_task.commands.iter().map(|cmd_exec| {
                cmd_exec.template.iter().map(|c| match c {
                    TemplateComponent::Literal(s) => s.clone(),
                    TemplateComponent::Parameter(p) => p.original_token.clone(),
                    TemplateComponent::GenericParams => "<axes::params>".to_string(),
                }).collect::<String>()
            }).collect::<Vec<_>>().join(" && ");
            
            log::debug!(
                "{:indent$}'{}:{}' resolved to flattened string: '{}'",
                "", namespace, sub_key, sub_value_str, indent = (depth as usize) * 2
            );

            next_str.push_str(&sub_value_str);
            last_match_end = full_match.end();
        }
        next_str.push_str(&current_str[last_match_end..]);
        current_str = next_str;
    }

    recursion_stack.remove(&stack_key);
    log::debug!(
        "{:indent$}Finished static expansion for '{}:{}': '{}'",
        "", key, value_type, current_str, indent = (depth as usize) * 2
    );
    Ok(current_str)
}

/// The core JIT function to resolve a `CacheableValue` into a `Task`.
fn expand_and_get_task_internal<'a>(
    key: &str,
    value_type: &str,
    config: &'a ResolvedConfig, // Now immutable
    cache: &'a mut ExpansionCache, // The mutable state
    recursion_stack: &mut HashSet<String>,
    depth: u32,
) -> ResolverResult<&'a Task> {
    let cache_key = format!("{}::{}", value_type, key);
    log::debug!(
        "{:indent$}Resolving task for '{}'",
        "", &cache_key, indent = (depth as usize) * 2
    );

    // Step 1: Check the temporary cache first.
    if let Some(task) = cache.get(&cache_key) {
        log::debug!("{:indent$}Value is already in expansion cache. Returning reference.", "", indent = (depth as usize) * 2);
        // We need to bypass the borrow checker here because we are returning a reference
        // from a mutable borrow. This is safe because we know we are not modifying
        // this specific entry further up the stack.
        // It's a classic interior mutability pattern implemented manually.
        let task_ptr = task as *const Task;
        return Ok(unsafe { &*task_ptr });
    }
    
    // Step 2: Get the Raw value from the immutable config.
    let (command_to_process, desc_to_process) = {
        let value_map = match value_type {
            "vars" => &config.vars,
            "scripts" => &config.scripts,
            _ => return Err(ResolverError::InvalidValueType { value_type: value_type.to_string(), key: key.to_string() }),
        };
        if let Some(CacheableValue::Raw { command, desc }) = value_map.get(key) {
            (command.clone(), desc.clone())
        } else {
            // It might already be expanded in the main config, but not our temp cache. This is an error.
            return Err(ResolverError::ValueNotFound { key: key.to_string(), value_type: value_type.to_string() });
        }
    };
    
    let command_lines = get_command_lines_from_command(&command_to_process, key);
    let mut new_task = Task { commands: Vec::new(), desc: desc_to_process };

    for line in command_lines {
        let (ignore_errors, run_in_parallel, template_str) = if value_type == "script" {
            // Los prefijos solo se aplican a los SCRIPTS.
            parse_execution_prefixes(&line)
        } else {
            // Las VARS se tratan como texto literal puro.
            (false, false, line.as_str())
        };

        // Expansión de compuestos
        let semi_expanded_str = expand_composite_tokens_recursively(key, value_type, template_str, config, cache, recursion_stack, depth)?;
        
        // Parseo a componentes
        let mut components = parameters::discover_and_parse(&semi_expanded_str)?;
        
        // ¡NUEVO PASO! Expansión final de tokens simples sobre los literales.
        for component in components.iter_mut() {
            if let TemplateComponent::Literal(s) = component {
                *s = s.replace("<axes::path>", &config.project_root.to_string_lossy());
                *s = s.replace("<axes::name>", &config.qualified_name);
                *s = s.replace("<axes::uuid>", &config.uuid.to_string());
                if let Some(version) = &config.version {
                    *s = s.replace("<axes::version>", version);
                }
            }
        }
        
        log::debug!("{:indent$}Final components after simple expansion: {:?}", "", components, indent = (depth as usize) * 2);
        
        new_task.commands.push(CommandExecution {
            template: components,
            ignore_errors,
            run_in_parallel,
        });
    }
    
    cache.insert(cache_key.clone(), new_task);
    Ok(cache.get(&cache_key).unwrap())
}

// --- INHERITANCE AND MERGE LOGIC ---

/// Walks up the project tree from a leaf node to the root, collecting all `IndexEntry`s
/// and their corresponding `ProjectConfig`s from `axes.toml` files.
/// The resulting vector is ordered from the root down to the leaf.
///
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

    // The chain is built from leaf to root, so we reverse it to get root-to-leaf order for merging.
    chain.reverse();
    Ok(chain)
}

fn merge_chain_into_config(chain: Vec<ProjectConfig>) -> ResolvedConfig {
    // Initialize with a default, empty `ResolvedConfig`.
    let mut resolved = ResolvedConfig {
        uuid: Uuid::nil(),
        qualified_name: String::new(),
        project_root: PathBuf::new(),
        scripts: HashMap::new(),
        vars: HashMap::new(),
        env: HashMap::new(),
        options: ResolvedOptionsConfig::default(),
        version: None,
        description: None,
    };

    // Iterate through the chain from parent to child.
    for config in chain {
        // Metadata fields are overwritten if Some.
        resolved.version = config.version.or(resolved.version);
        resolved.description = config.description.or(resolved.description);
        
        // Simple HashMap fields are extended. Children's values overwrite parents'.
        resolved.env.extend(config.env);
        
        // Convert `vars` (String) into `CacheableValue::Raw`.
        resolved.vars.extend(
            config.vars.into_iter().map(|(k, v)| {
                // Un `var` debe tratarse como un Command para poder ser resuelto por el mismo motor.
                // Lo definimos como Simple, que es lo más cercano a un string.
                (k, CacheableValue::Raw { 
                    command: ProjectCommand::Simple(v), 
                    desc: None 
                })
            })
        );

        // Convert `scripts` (`Command`) into `CacheableValue::Raw`.
        resolved.scripts.extend(
            config.scripts.into_iter().map(|(k, cmd)| {
                let desc = get_desc_from_command(&cmd);
                (k, CacheableValue::Raw { command: cmd, desc })
            })
        );
        
        // --- NEW LOGIC for merging `OptionsConfig` ---

        // `shell` is a simple overwrite.
        resolved.options.shell = config.options.shell.or(resolved.options.shell);

        // For `at_start` and `at_exit`, if the child defines one, it overwrites the parent's.
        if let Some(cmd) = config.options.at_start {
            resolved.options.at_start = Some(CacheableValue::Raw { command: cmd, desc: None });
        }
        if let Some(cmd) = config.options.at_exit {
            resolved.options.at_exit = Some(CacheableValue::Raw { command: cmd, desc: None });
        }

        // `open_with` is a HashMap, so we extend it, allowing children to add or overwrite entries.
        resolved.options.open_with.extend(
            config.options.open_with.into_iter().map(|(k, cmd)| {
                let desc = get_desc_from_command(&cmd);
                (k, CacheableValue::Raw { command: cmd, desc })
            })
        );
    }

    resolved
}


fn get_desc_from_command(command: &ProjectCommand) -> Option<String> {
    match command {
        ProjectCommand::Extended(ext) => ext.desc.clone(),
        ProjectCommand::Platform(pc) => pc.desc.clone(),
        _ => None,
    }
}

fn get_command_lines_from_command(command: &ProjectCommand, key: &str) -> Vec<String> {
    let runnable = match command {
        ProjectCommand::Simple(s) => return vec![s.clone()],
        ProjectCommand::Sequence(s) => return s.clone(),
        ProjectCommand::Extended(ext) => &ext.run,
        ProjectCommand::Platform(pc) => {
            let os = std::env::consts::OS;
            let os_specific = if os == "windows" { pc.windows.as_ref() }
                              else if os == "linux" { pc.linux.as_ref() }
                              else if os == "macos" { pc.macos.as_ref() }
                              else { None };
            match os_specific.or(pc.default.as_ref()) {
                Some(r) => r,
                None => {
                    log::warn!("Script '{}' has no platform implementation for '{}' and no default.", key, os);
                    return Vec::new();
                }
            }
        }
    };
    match runnable {
        Runnable::Single(s) => vec![s.clone()],
        Runnable::Sequence(s) => s.clone(),
    }
}

fn parse_execution_prefixes(line: &str) -> (bool, bool, &str) {
    let mut trimmed_line = line.trim_start();
    let mut ignore_errors = false;
    let mut run_in_parallel = false;

    // Loop to handle multiple prefixes like `->`
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

/// Reads and deserializes a single `axes.toml` file into a `ProjectConfig` struct.
fn load_project_config(entry: &IndexEntry) -> ResolverResult<ProjectConfig> {
    let config_path = entry.path.join(AXES_DIR).join(PROJECT_CONFIG_FILENAME);
    if !config_path.is_file() {
        // A missing config file is a critical error in the resolution path.
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

/// Gathers the last-modified timestamps for all `axes.toml` files in an inheritance chain.
/// This is used to validate the freshness of the cache.
fn get_dependencies_timestamps<'a>(
    inheritance_chain: &[(&'a IndexEntry, ProjectConfig)],
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

/// Reads the binary cache from disk and validates its freshness against file timestamps.
/// Returns `Some(ResolvedConfig)` if the cache is valid, otherwise `None`.
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

    // If the cache file is empty, it's invalid.
    if cached_bytes.is_empty() {
        return Ok(None);
    }

    let decode_result: Result<(SerializableConfigCache, usize), _> =
        bincode::serde::decode_from_slice(&cached_bytes, bincode::config::standard());

    let serializable_cache = match decode_result {
        Ok((cache, _)) => cache,
        Err(e) => {
            // A corrupt cache is not a fatal error; we just log it and regenerate.
            if !matches!(e, DecodeError::Io { .. }) {
                log::warn!(
                    "Config cache at '{}' is corrupt or outdated. Regenerating. (Error: {})",
                    cache_path.display(),
                    e
                );
                // Attempt to remove the corrupt file to prevent future errors.
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
                log::debug!("Cache dependency '{}' no longer exists. Cache invalid.", path.display());
                return Ok(None); // A dependency is missing, cache is invalid.
            }
        };

        let cached_mod_time: SystemTime = (*cached_mod_time_serializable).into();
        if current_mod_time > cached_mod_time {
            log::debug!("Cache dependency '{}' has been modified. Cache invalid.", path.display());
            return Ok(None);
        }
    }

    Ok(Some(serializable_cache.resolved_config.into()))
}

/// Writes a `ResolvedConfig` and its dependencies to a binary cache file.
/// This function serializes the in-memory representation for fast retrieval on subsequent runs.
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

///
/// A simple, non-recursive interpolator for static values like `at_start`, `at_exit`,
/// and `open_with` commands. It does not handle script parameters.
///
pub fn interpolate_simple_string(template: &str, config: &ResolvedConfig) -> Result<String> {
    let re = Regex::new(r"<axes::([^>]+)>").unwrap();
    let mut result = template.to_string();

    // Limit iterations to prevent infinite loops with vars referencing each other.
    for _ in 0..MAX_RECURSION_DEPTH {
        let captures: Vec<_> = re.captures_iter(&result).collect();
        if captures.is_empty() {
            return Ok(result); // No more tokens to expand
        }

        let mut next_result = String::new();
        let mut last_match_end = 0;

        for caps in captures {
            let full_match = caps.get(0).unwrap();
            let token_path = caps.get(1).unwrap().as_str();

            next_result.push_str(&result[last_match_end..full_match.start()]);

            let expanded_value = match token_path.split("::").collect::<Vec<_>>().as_slice() {
                ["name"] => config.qualified_name.clone(),
                ["path"] => config.project_root.to_string_lossy().to_string(),
                ["uuid"] => config.uuid.to_string(),
                ["version"] => config.version.clone().unwrap_or_default(),
                ["vars", key] => {
                    match config.vars.get(*key) {
                        Some(CacheableValue::Raw { command, .. }) => {
                            // Un 'var' siempre se almacena como un ProjectCommand::Simple
                            if let ProjectCommand::Simple(s) = command {
                                s.clone()
                            } else {
                                String::new() // O un error, pero esto es más seguro
                            }
                        },
                        Some(CacheableValue::Expanded(task)) => {
                             // Aplanar la tarea expandida a un string
                             task.commands.iter().map(|cmd_exec| {
                                cmd_exec.template.iter().map(|c| match c {
                                    TemplateComponent::Literal(s) => s.clone(),
                                    TemplateComponent::Parameter(p) => p.original_token.clone(),
                                    TemplateComponent::GenericParams => "<axes::params>".to_string(),
                                }).collect::<String>()
                            }).collect::<Vec<_>>().join(" && ")
                        }
                        None => String::new(),
                    }
                }
                _ => {
                    // Un token desconocido o un token de parámetro no se expande.
                    log::warn!(
                        "Unsupported token in simple interpolation: {}",
                        full_match.as_str()
                    );
                    full_match.as_str().to_string()
                }
            };
            next_result.push_str(&expanded_value);
            last_match_end = full_match.end();
        }
        next_result.push_str(&result[last_match_end..]);
        result = next_result;
    }

    Err(anyhow!(
        "Interpolation exceeded max depth, possible circular reference in [vars]."
    ))
}
