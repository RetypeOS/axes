// EN: src/core/config_resolver.rs (REPLACE ENTIRE FILE)

use crate::{
    core::{cache, parameters, paths},
    models::{
        CachedProjectConfig, CanonicalCommand, Command, CommandAction, CommandExecution, GlobalIndex, IndexEntry, OptionsConfig, ProjectConfig, ResolvedConfig, RunSpec, Runnable, Task, TemplateComponent
    },
};
use anyhow::{Context, Result, anyhow};
use lazy_static::lazy_static;
use regex::Regex;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
use uuid::Uuid;

lazy_static! {
    static ref TOKEN_RE: Regex = Regex::new(r"<axes::([^>]+)>").unwrap();
}

#[derive(Error, Debug)]
pub enum ResolverError {
    #[error("Project with UUID '{0}' not found in the index.")]
    UuidNotFound(Uuid),
    #[error("I/O error while processing configuration: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse TOML file at '{path}': {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

// --- MAIN RESOLVER FUNCTION ---
pub fn resolve_config(
    uuid: Uuid,
    index: &mut GlobalIndex,
    memoizer: &mut HashMap<Uuid, Arc<ResolvedConfig>>,
) -> Result<Arc<ResolvedConfig>> {
    log::debug!("Resolving config for UUID: {}", uuid);
    if let Some(config) = memoizer.get(&uuid) {
        log::trace!("Resolved config for {} found in in-session memoizer.", uuid);
        return Ok(config.clone());
    }

    let entry = index.projects.get(&uuid).ok_or_else(|| anyhow!("Project UUID {} not in index.", uuid))?.clone();
    let mut hash_memoizer = HashMap::new();
    let current_state_hash = get_state_hash(uuid, index, &mut hash_memoizer)?;

    if let Some(saved_hash) = &entry.config_hash {
        if *saved_hash == current_state_hash {
            if let Some(cache_dir) = &entry.cache_dir {
                let cache_file_path = cache_dir.join(saved_hash);
                if let Ok(cached_layer) = read_cached_layer(&cache_file_path) {
                    log::debug!("Cache HIT for project '{}'. Merging layers.", entry.name);
                    let final_config = if let Some(parent_uuid) = entry.parent {
                        let parent_config = resolve_config(parent_uuid, index, memoizer)?;
                        merge_configs(parent_config, &cached_layer, uuid, &entry)?
                    } else {
                        merge_configs(Arc::new(ResolvedConfig::default()), &cached_layer, uuid, &entry)?
                    };
                    let final_config_arc = Arc::new(final_config);
                    memoizer.insert(uuid, final_config_arc.clone());
                    return Ok(final_config_arc);
                }
            }
        }
    }

    log::debug!("Cache MISS for project '{}'. Recalculating.", entry.name);
    let parent_config = if let Some(parent_uuid) = entry.parent {
        resolve_config(parent_uuid, index, memoizer)?
    } else {
        Arc::new(ResolvedConfig::default())
    };

    let self_layer = load_and_compile_layer(&entry)?;
    let final_config = merge_configs(parent_config, &self_layer, uuid, &entry)?;
    let new_cache_dir = paths::get_cache_dir_for_project(&final_config)?;
    let new_cache_file_path = new_cache_dir.join(&current_state_hash);
    write_cached_layer(&new_cache_file_path, &self_layer)?;

    let index_entry_mut = index.projects.get_mut(&uuid).unwrap();
    index_entry_mut.config_hash = Some(current_state_hash);
    index_entry_mut.cache_dir = Some(new_cache_dir);

    let final_config_arc = Arc::new(final_config);
    memoizer.insert(uuid, final_config_arc.clone());
    Ok(final_config_arc)
}

// --- HASHING ---
fn get_state_hash(
    uuid: Uuid,
    index: &GlobalIndex,
    memoizer: &mut HashMap<Uuid, String>,
) -> Result<String> {
    if let Some(hash) = memoizer.get(&uuid) {
        return Ok(hash.clone());
    }

    let entry = index.projects.get(&uuid).ok_or(ResolverError::UuidNotFound(uuid))?;
    let parent_hash = if let Some(parent_uuid) = entry.parent {
        get_state_hash(parent_uuid, index, memoizer)?
    } else {
        "root".to_string()
    };

    let config_path = entry.path.join(".axes").join("axes.toml");
    let self_toml_hash = if config_path.exists() {
        cache::calculate_validation_data(&config_path)?.content_hash
    } else {
        "empty".to_string()
    };

    let combined_data = format!("parent:{}|self:{}", parent_hash, self_toml_hash);
    let state_hash = hex::encode(&blake3::hash(combined_data.as_bytes()).as_bytes()[..16]);
    memoizer.insert(uuid, state_hash.clone());
    Ok(state_hash)
}

// --- MERGING ---
fn merge_configs(
    parent_config: Arc<ResolvedConfig>,
    child_layer: &CachedProjectConfig,
    child_uuid: Uuid,
    child_entry: &IndexEntry,
) -> Result<ResolvedConfig> {
    let mut merged = Arc::try_unwrap(parent_config).unwrap_or_else(|arc| (*arc).clone());
    merged.scripts.extend(child_layer.scripts.clone());
    merged.vars.extend(child_layer.vars.clone());
    merged.env.extend(child_layer.env.clone());
    merged.version = child_layer.version.clone().or(merged.version);
    merged.description = child_layer.description.clone().or(merged.description);
    merged.options.shell = child_layer.options.shell.clone().or(merged.options.shell);
    merged.options.cache_dir = child_layer.options.cache_dir.clone().or(merged.options.cache_dir);

    if let Some(at_start_cmd) = child_layer.options.at_start.clone() {
        merged.options.at_start = Some(compile_command_to_task(at_start_cmd.0)?);
    }
    if let Some(at_exit_cmd) = child_layer.options.at_exit.clone() {
        merged.options.at_exit = Some(compile_command_to_task(at_exit_cmd.0)?);
    }
    let compiled_open_with = compile_command_map(child_layer.options.open_with.clone())?;
    merged.options.open_with.extend(compiled_open_with);

    merged.uuid = child_uuid;
    merged.project_root = child_entry.path.clone();

    if child_entry.parent.is_none() {
        merged.qualified_name = child_entry.name.clone();
    } else {
        merged.qualified_name = format!("{}/{}", merged.qualified_name, child_entry.name);
    }
    Ok(merged)
}

// --- LAYER COMPILATION ---
fn load_and_compile_layer(entry: &IndexEntry) -> Result<CachedProjectConfig> {
    let config_path = entry.path.join(".axes").join("axes.toml");
    if !config_path.exists() {
        // If a project has no axes.toml, it's an empty layer.
        return Ok(CachedProjectConfig {
            version: None,
            description: None,
            scripts: HashMap::new(),
            vars: HashMap::new(),
            env: HashMap::new(),
            options: OptionsConfig::default(),
        });
    }

    let content = fs::read_to_string(&config_path)?;
    let project_config: ProjectConfig = toml::from_str(&content).map_err(|e| ResolverError::TomlParse {
        path: config_path,
        source: e,
    })?;

    let scripts = compile_command_map(project_config.scripts)?;
    let vars_as_commands: HashMap<String, Command> = project_config
        .vars
        .into_iter()
        .map(|(k, v)| (k, Command::from(v)))
        .collect();
    let vars = compile_command_map(vars_as_commands)?;

    Ok(CachedProjectConfig {
        version: project_config.version,
        description: project_config.description,
        scripts,
        vars,
        env: project_config.env,
        options: project_config.options,
    })
}

fn compile_command_map(command_map: HashMap<String, Command>) -> Result<HashMap<String, Task>> {
    command_map
        .into_iter()
        .map(|(name, cmd)| compile_command_to_task(cmd.0).map(|task| (name, task)))
        .collect()
}

fn compile_command_to_task(command: CanonicalCommand) -> Result<Task> {
    let os = std::env::consts::OS;
    let runnable = if os == "windows" {
        command.windows.or(command.default)
    } else if os == "linux" {
        command.linux.or(command.default)
    } else if os == "macos" {
        command.macos.or(command.default)
    } else {
        command.default
    };

    let command_lines = match runnable {
        Some(Runnable::Single(s)) => vec![s],
        Some(Runnable::Sequence(s)) => s,
        None => Vec::new(),
    };

    let commands = command_lines
        .iter()
        .map(|line| expand_line_to_execution(line))
        .collect::<Result<_>>()?;

    Ok(Task {
        commands,
        desc: command.desc,
    })
}

fn expand_line_to_execution(line: &str) -> Result<CommandExecution> {
    let (prefixes, command_text) = parse_prefixes(line);
    let action = if prefixes.is_echo {
        CommandAction::Print(tokenize_string(command_text)?)
    } else {
        CommandAction::Execute(tokenize_string(command_text)?)
    };
    Ok(CommandExecution {
        action,
        ignore_errors: prefixes.ignore_errors,
        run_in_parallel: prefixes.run_in_parallel,
        silent_mode: prefixes.silent_mode,
    })
}

fn tokenize_string(text: &str) -> Result<Vec<TemplateComponent>> {
    let mut components = Vec::new();
    let mut last_index = 0;
    for caps in TOKEN_RE.captures_iter(text) {
        let full_match = caps.get(0).unwrap();
        if last_index < full_match.start() {
            components.push(TemplateComponent::Literal(text[last_index..full_match.start()].to_string()));
        }
        let content = caps.get(1).unwrap().as_str().trim();
        let component = if let Some(param_spec) = content.strip_prefix("params::") {
            TemplateComponent::Parameter(parameters::parse_parameter_token(full_match.as_str(), param_spec)?)
        } else if content == "params" {
            TemplateComponent::GenericParams
        } else if let Some(run_spec) = content.strip_prefix("run") {
            if let Some(cmd) = run_spec.strip_prefix("('").and_then(|s| s.strip_suffix("')")) {
                TemplateComponent::Run(RunSpec::Literal(cmd.to_string()))
            } else {
                return Err(anyhow!("Invalid run syntax in token: {}", full_match.as_str()));
            }
        } else {
            match content {
                "path" => TemplateComponent::Path, "name" => TemplateComponent::Name,
                "uuid" => TemplateComponent::Uuid, "version" => TemplateComponent::Version,
                s if s.starts_with("scripts::") || s.starts_with("vars::") => {
                    return Err(anyhow!(
                        "Inter-script composition ('{}') is not allowed in single-layer compilation. It will be handled by the lazy resolver.",
                        full_match.as_str()
                    ));
                }
                _ => return Err(anyhow!("Unknown token namespace in: '{}'", full_match.as_str())),
            }
        };
        components.push(component);
        last_index = full_match.end();
    }
    if last_index < text.len() {
        components.push(TemplateComponent::Literal(text[last_index..].to_string()));
    }
    Ok(components)
}

#[derive(Debug, Default)]
struct Prefixes {
    ignore_errors: bool, run_in_parallel: bool,
    silent_mode: bool, is_echo: bool,
}

fn parse_prefixes(line: &str) -> (Prefixes, &str) {
    let mut prefixes = Prefixes::default();
    if let Some(command_text) = line.strip_prefix('#') {
        prefixes.is_echo = true;
        return (prefixes, command_text.strip_prefix(' ').unwrap_or(command_text));
    }
    let mut current_pos = 0;
    for (i, char) in line.char_indices() {
        match char {
            '-' => prefixes.ignore_errors = true, '>' => prefixes.run_in_parallel = true,
            '@' => prefixes.silent_mode = true, '|' => { current_pos = i + 1; break; }
            _ if !char.is_whitespace() => { current_pos = i; break; }
            _ => (),
        }
        if i == line.len() - 1 { current_pos = line.len(); }
    }
    (prefixes, line.get(current_pos..).unwrap_or("").trim_start())
}

// --- CACHE I/O ---
fn read_cached_layer(path: &Path) -> Result<CachedProjectConfig> {
    let bytes = fs::read(path).with_context(|| format!("Failed to read cache file at '{}'", path.display()))?;
    let (cached_layer, _): (CachedProjectConfig, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
            .with_context(|| format!("Failed to deserialize cache file '{}'. It might be corrupt.", path.display()))?;
    Ok(cached_layer)
}

fn write_cached_layer(path: &Path, layer: &CachedProjectConfig) -> Result<()> {
    if let Some(parent_dir) = path.parent() {
        fs::create_dir_all(parent_dir).with_context(|| format!("Failed to create cache directory '{}'", parent_dir.display()))?;
    }
    let bytes = bincode::serde::encode_to_vec(layer, bincode::config::standard()).context("Failed to serialize cache layer.")?;
    fs::write(path, &bytes).with_context(|| format!("Failed to write cache file to '{}'", path.display()))?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Command, OptionsConfig, ProjectConfig, ResolvedConfig};
    use std::fs;
    use tempfile::tempdir;
    use uuid::Uuid;

    // --- TEST HELPER: Creates a temporary project structure ---
    fn setup_test_project(
        dir: &Path,
        toml_content: &str,
    ) -> IndexEntry {
        let axes_dir = dir.join(".axes");
        fs::create_dir_all(&axes_dir).unwrap();
        fs::write(axes_dir.join("axes.toml"), toml_content).unwrap();

        IndexEntry {
            name: dir.file_name().unwrap().to_str().unwrap().to_string(),
            path: dir.to_path_buf(),
            parent: None,
            config_hash: None,
            cache_dir: None,
            last_used_child: None,
        }
    }

    // --- TESTS FOR `load_and_compile_layer` ---
    
    #[test]
    fn test_load_and_compile_simple_layer() {
        // --- Setup ---
        let dir = tempdir().unwrap();
        let toml = r#"
            version = "1.0.0"
            description = "A test project"

            [scripts]
            hello = "echo 'Hello'"

            [vars]
            my_var = "World"
        "#;
        let entry = setup_test_project(dir.path(), toml);

        // --- Execute ---
        let result = load_and_compile_layer(&entry);

        // --- Assert ---
        assert!(result.is_ok());
        let layer = result.unwrap();

        assert_eq!(layer.version, Some("1.0.0".to_string()));
        assert_eq!(layer.description, Some("A test project".to_string()));
        assert!(layer.scripts.contains_key("hello"));
        assert!(layer.vars.contains_key("my_var"));

        // Check if the script was compiled to a Task
        let hello_task = layer.scripts.get("hello").unwrap();
        assert_eq!(hello_task.commands.len(), 1);
        match &hello_task.commands[0].action {
            CommandAction::Execute(template) => {
                assert_eq!(template.len(), 1);
                match &template[0] {
                    TemplateComponent::Literal(s) => assert_eq!(s, "echo 'Hello'"),
                    _ => panic!("Expected a literal component"),
                }
            }
            _ => panic!("Expected an Execute action"),
        }
    }
    
    // --- TESTS FOR `merge_configs` ---

    #[test]
    fn test_merge_configs_child_overrides_parent() {
        // --- Setup ---
        // Parent config
        let parent_config = Arc::new(ResolvedConfig {
            version: Some("1.0.0".to_string()),
            qualified_name: "parent".to_string(),
            scripts: HashMap::from([(
                "common".to_string(),
                Task { desc: Some("from parent".to_string()), ..Default::default() },
            )]),
            ..Default::default()
        });

        // Child layer
        let child_layer = CachedProjectConfig {
            version: Some("2.0.0".to_string()), // Override
            scripts: HashMap::from([(
                "child_script".to_string(),
                Task::default(),
            )]),
            ..Default::default()
        };
        
        let child_uuid = Uuid::new_v4();
        let child_entry = IndexEntry {
            name: "child".to_string(),
            path: PathBuf::from("/child"),
            parent: Some(Uuid::new_v4()), // Dummy parent UUID
            ..Default::default()
        };

        // --- Execute ---
        let result = merge_configs(parent_config, &child_layer, child_uuid, &child_entry);

        // --- Assert ---
        assert!(result.is_ok());
        let merged = result.unwrap();

        // Child's version overrides parent's
        assert_eq!(merged.version, Some("2.0.0".to_string()));
        // Scripts from both parent and child are present
        assert!(merged.scripts.contains_key("common"));
        assert!(merged.scripts.contains_key("child_script"));
        // Qualified name is correctly constructed
        assert_eq!(merged.qualified_name, "parent/child");
        // UUID is correctly set
        assert_eq!(merged.uuid, child_uuid);
    }
    
    // --- TESTS FOR `get_state_hash` (Placeholder - requires more complex setup) ---

    #[test]
    fn test_get_state_hash_single_node() {
        // This test ensures the basic hashing works for a root project.
        let dir = tempdir().unwrap();
        let toml = "[scripts]\nkey = 'val'";
        let mut entry = setup_test_project(dir.path(), toml);
        entry.parent = None; // Explicitly a root

        let mut index = GlobalIndex::default();
        let uuid = Uuid::new_v4();
        index.projects.insert(uuid, entry);
        
        let mut memoizer = HashMap::new();
        let result = get_state_hash(uuid, &index, &mut memoizer);

        assert!(result.is_ok());
        let hash = result.unwrap();

        // We expect a hash of (parent:"root" + self:hash(toml))
        let self_hash = cache::calculate_validation_data(&dir.path().join(".axes/axes.toml")).unwrap().content_hash;
        let combined = format!("parent:root|self:{}", self_hash);
        let expected_hash = hex::encode(&blake3::hash(combined.as_bytes()).as_bytes()[..16]);

        assert_eq!(hash, expected_hash);
    }
}