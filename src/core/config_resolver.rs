// EN: src/core/config_resolver.rs

use crate::{
    core::{cache, parameters, paths},
    models::{
        CachedProjectConfig, CanonicalCommand, Command, CommandAction, CommandExecution,
        GlobalIndex, IndexEntry, OptionsConfig, ProjectConfig, ResolvedConfig, RunSpec, Runnable,
        Task, TemplateComponent,
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
    let project_config: ProjectConfig =
        toml::from_str(&content).map_err(|e| ResolverError::TomlParse {
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

/// This is the new top-level entry point for configuration resolution.
/// It will be fully implemented in the next iterations.
pub fn resolve_config(
    _uuid: Uuid,
    _index: &mut GlobalIndex,
    _memoizer: &mut HashMap<Uuid, Arc<ResolvedConfig>>,
) -> Result<Arc<ResolvedConfig>> {
    unimplemented!("Phase 2: Main recursive resolver logic to be built here.");
}

/// Loads a single configuration layer (`CachedProjectConfig`) for a given UUID.
/// This function performs cache validation for the single layer and recompiles it if necessary.
pub fn load_layer_for_uuid(
    uuid: Uuid,
    index: &mut GlobalIndex,
) -> Result<Arc<CachedProjectConfig>> {
    // FIX: Corrected typo `CachedProject-Config`
    log::debug!("Loading layer for UUID: {}", uuid);

    let entry = index
        .projects
        .get(&uuid)
        .ok_or_else(|| anyhow!("Project UUID {} not found in index.", uuid))?
        .clone();

    let config_path = entry.path.join(".axes").join("axes.toml");
    let current_toml_hash = if config_path.exists() {
        cache::calculate_validation_data(&config_path)?.content_hash
    } else {
        "empty".to_string()
    };

    if let (Some(saved_hash), Some(cache_dir)) = (&entry.config_hash, &entry.cache_dir) {
        if *saved_hash == current_toml_hash {
            let cache_file_path = cache_dir.join(saved_hash);
            if let Ok(cached_layer) = read_cached_layer(&cache_file_path) {
                log::debug!("Cache HIT for layer '{}'.", entry.name);
                return Ok(Arc::new(cached_layer));
            }
        }
    }

    log::debug!("Cache MISS for layer '{}'. Recompiling.", entry.name);
    let new_layer = load_and_compile_layer(&entry)?;

    // NOTE: This cache_dir resolution is temporary for Phase 1.
    // In Phase 2, this will be determined by resolving the parent's config first.
    let cache_dir = entry.cache_dir.clone().unwrap_or_else(|| {
        paths::get_axes_config_dir()
            .unwrap()
            .join("cache")
            .join("projects")
    });

    let new_cache_file_path = cache_dir.join(&current_toml_hash);
    write_cached_layer(&new_cache_file_path, &new_layer)?;

    let index_entry_mut = index.projects.get_mut(&uuid).unwrap();
    index_entry_mut.config_hash = Some(current_toml_hash);
    index_entry_mut.cache_dir = Some(cache_dir);

    Ok(Arc::new(new_layer))
}

pub(crate) fn compile_command_map(
    command_map: HashMap<String, Command>,
) -> Result<HashMap<String, Task>> {
    command_map
        .into_iter()
        .map(|(name, cmd)| compile_command_to_task(cmd.0).map(|task| (name, task)))
        .collect()
}

pub(crate) fn compile_command_to_task(command: CanonicalCommand) -> Result<Task> {
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
            components.push(TemplateComponent::Literal(
                text[last_index..full_match.start()].to_string(),
            ));
        }
        let content = caps.get(1).unwrap().as_str().trim();
        let component = if let Some(param_spec) = content.strip_prefix("params::") {
            TemplateComponent::Parameter(parameters::parse_parameter_token(
                full_match.as_str(),
                param_spec,
            )?)
        } else if content == "params" {
            TemplateComponent::GenericParams
        } else if let Some(run_spec) = content.strip_prefix("run") {
            if let Some(cmd) = run_spec
                .strip_prefix("('")
                .and_then(|s| s.strip_suffix("')"))
            {
                TemplateComponent::Run(RunSpec::Literal(cmd.to_string()))
            } else {
                return Err(anyhow!(
                    "Invalid run syntax in token: {}",
                    full_match.as_str()
                ));
            }
        } else {
            // Static tokens and NEW symbolic references
            match content {
                "path" => TemplateComponent::Path,
                "name" => TemplateComponent::Name,
                "uuid" => TemplateComponent::Uuid,
                "version" => TemplateComponent::Version,

                // FIX: Instead of erroring, create symbolic components.
                s if s.starts_with("scripts::") => {
                    TemplateComponent::Script(s.strip_prefix("scripts::").unwrap().to_string())
                }
                s if s.starts_with("vars::") => {
                    TemplateComponent::Var(s.strip_prefix("vars::").unwrap().to_string())
                }

                _ => {
                    return Err(anyhow!(
                        "Unknown token namespace in: '{}'",
                        full_match.as_str()
                    ));
                }
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
    ignore_errors: bool,
    run_in_parallel: bool,
    silent_mode: bool,
    is_echo: bool,
}

fn parse_prefixes(line: &str) -> (Prefixes, &str) {
    let mut prefixes = Prefixes::default();
    if let Some(command_text) = line.strip_prefix('#') {
        prefixes.is_echo = true;
        return (
            prefixes,
            command_text.strip_prefix(' ').unwrap_or(command_text),
        );
    }
    let mut current_pos = 0;
    for (i, char) in line.char_indices() {
        match char {
            '-' => prefixes.ignore_errors = true,
            '>' => prefixes.run_in_parallel = true,
            '@' => prefixes.silent_mode = true,
            '|' => {
                current_pos = i + 1;
                break;
            }
            _ if !char.is_whitespace() => {
                current_pos = i;
                break;
            }
            _ => (),
        }
        if i == line.len() - 1 {
            current_pos = line.len();
        }
    }
    (prefixes, line.get(current_pos..).unwrap_or("").trim_start())
}

// --- CACHE I/O ---
fn read_cached_layer(path: &Path) -> Result<CachedProjectConfig> {
    let bytes = fs::read(path)
        .with_context(|| format!("Failed to read cache file at '{}'", path.display()))?;
    let (cached_layer, _): (CachedProjectConfig, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).with_context(
            || {
                format!(
                    "Failed to deserialize cache file '{}'. It might be corrupt.",
                    path.display()
                )
            },
        )?;
    Ok(cached_layer)
}

fn write_cached_layer(path: &Path, layer: &CachedProjectConfig) -> Result<()> {
    if let Some(parent_dir) = path.parent() {
        fs::create_dir_all(parent_dir).with_context(|| {
            format!(
                "Failed to create cache directory '{}'",
                parent_dir.display()
            )
        })?;
    }
    let bytes = bincode::serde::encode_to_vec(layer, bincode::config::standard())
        .context("Failed to serialize cache layer.")?;
    fs::write(path, &bytes)
        .with_context(|| format!("Failed to write cache file to '{}'", path.display()))?;
    Ok(())
}

// --- MARK: TESTS

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::index_manager::GLOBAL_PROJECT_UUID;
    use crate::models::{GlobalIndex, TemplateComponent};
    use std::fs;
    use tempfile::tempdir;

    fn setup_test_index() -> GlobalIndex {
        let mut index = GlobalIndex::default();
        index.projects.insert(
            GLOBAL_PROJECT_UUID,
            IndexEntry {
                name: "global".to_string(),
                path: PathBuf::from("/tmp/global"),
                parent: None,
                ..Default::default()
            },
        );
        index
    }

    fn setup_test_project(
        index: &mut GlobalIndex,
        parent: Option<Uuid>,
        name: &str,
        toml_content: &str,
    ) -> Uuid {
        // FIX: Use `TempDir::into_path` is deprecated, but for tests it is fine.
        // Let's keep it but acknowledge the warning. For production code we would use `keep()`.
        let dir = tempdir().unwrap();
        let project_path = dir.keep();

        let axes_dir = project_path.join(".axes");
        fs::create_dir_all(&axes_dir).unwrap();
        fs::write(axes_dir.join("axes.toml"), toml_content).unwrap();

        let uuid = Uuid::new_v4();
        index.projects.insert(
            uuid,
            IndexEntry {
                name: name.to_string(),
                path: project_path,
                parent,
                ..Default::default()
            },
        );
        uuid
    }

    #[test]
    fn test_lazy_facade_inheritance_and_override() {
        let mut index = setup_test_index();

        let parent_toml = r#"
            version = "1.0.0"
            description = "Parent description"
            [scripts]
            common = "echo 'from parent'"
            [vars]
            theme = "dark"
            [env]
            PARENT_VAR = "parent_value"
            SHARED_VAR = "from_parent"
        "#;
        let parent_uuid =
            setup_test_project(&mut index, Some(GLOBAL_PROJECT_UUID), "parent", parent_toml);

        let child_toml = r#"
            version = "2.0.0" # Override
            [scripts]
            child_script = "echo 'from child'"
            [env]
            CHILD_VAR = "child_value"
            SHARED_VAR = "from_child" # Override
        "#;
        // FIX: Use the returned UUID to avoid unused_variable warning
        let _child_uuid = setup_test_project(&mut index, Some(parent_uuid), "child", child_toml);

        let config = crate::cli::handlers::commons::resolve_config_for_context(
            Some("parent/child".to_string()),
            &mut index,
        )
        .unwrap();

        assert_eq!(
            config.get_version(&mut index).unwrap(),
            Some("2.0.0".to_string())
        );
        assert_eq!(
            config.get_description(&mut index).unwrap(),
            Some("Parent description".to_string())
        );
        assert!(
            config
                .get_script("child_script", &mut index, 0)
                .unwrap()
                .is_some()
        );
        assert!(
            config
                .get_script("common", &mut index, 0)
                .unwrap()
                .is_some()
        );
        assert!(
            config
                .get_script("non_existent", &mut index, 0)
                .unwrap()
                .is_none()
        );
        assert!(config.get_var("theme", &mut index, 0).unwrap().is_some());
        let env = config.get_env(&mut index).unwrap();
        assert_eq!(env.get("SHARED_VAR"), Some(&"from_child".to_string()));
    }

    #[test]
    fn test_load_and_compile_simple_layer() {
        let mut index = setup_test_index();
        // FIX: Use a valid multiline TOML string
        let toml = r#"
            [scripts]
            hello = "echo 'Hello'"
        "#;
        let uuid = setup_test_project(&mut index, Some(GLOBAL_PROJECT_UUID), "test", toml);
        let entry = index.projects.get(&uuid).unwrap();

        // --- Execute ---
        let result = load_and_compile_layer(entry);

        // --- Assert ---
        assert!(result.is_ok());
        let layer = result.unwrap();
        assert!(layer.scripts.contains_key("hello"));
    }

    // --- TEST 3: Circular Dependency Detection via Depth Limit ---
    /// Ensures that the lazy resolver correctly stops and reports an error
    /// when a circular dependency would lead to infinite recursion.
    // --- TEST 3: Circular Dependency Detection via Depth Limit ---
    #[test]
    fn test_circular_dependency_detection() {
        let mut index = setup_test_index();
        let toml = r#"
            [scripts]
            a = "<axes::scripts::b>"
            b = "<axes::scripts::a>"
        "#;
        setup_test_project(&mut index, Some(GLOBAL_PROJECT_UUID), "cycle", toml);

        // FIX: Use the project's name as context, not its UUID.
        let config = crate::cli::handlers::commons::resolve_config_for_context(
            Some("cycle".to_string()),
            &mut index,
        )
        .unwrap();

        fn resolve_step(
            script_name: &str,
            config: &ResolvedConfig,
            index: &mut GlobalIndex,
            depth: u32,
        ) -> Result<String> {
            let task = config
                .get_script(script_name, index, depth)?
                .ok_or_else(|| anyhow!("Script not found"))?;

            let next_script_name = if let Some(cmd) = task.commands.first() {
                if let CommandAction::Execute(template) = &cmd.action {
                    if let Some(TemplateComponent::Script(name)) = template.first() {
                        name.clone()
                    } else {
                        return Err(anyhow!("End of chain"));
                    }
                } else {
                    return Err(anyhow!("End of chain"));
                }
            } else {
                return Err(anyhow!("End of chain"));
            };

            resolve_step(&next_script_name, config, index, depth + 1)
        }

        let result = resolve_step("a", &config, &mut index, 0);

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(
            error_message.contains("Maximum recursion depth exceeded"),
            "Error message was: {}",
            error_message
        );
    }
}
