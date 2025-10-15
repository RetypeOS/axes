// This module is responsible for the Ahead-of-Time (AOT) compilation of `axes.toml` files.
// Its primary goal is to transform the user-friendly, flexible TOML syntax into a
// platform-agnostic, optimized Abstract Syntax Tree (AST) that can be cached and executed efficiently.

use anyhow::{Context, Result, anyhow};
use lazy_static::lazy_static;
use regex::Regex;
use std::{collections::HashMap, fs};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    core::{cache, color, parameters, paths},
    models::{
        CachedOpenWithConfig, CachedOptionsConfig, CachedProjectConfig, CachedVar, CommandAction,
        CommandExecution, GlobalIndex, IndexEntry, IndexUpdate, PlatformCommand, PlatformExecution,
        ProjectConfig, RunSpec, Task, TemplateComponent, TomlCommand, TomlScript, TomlVar,
        TomlVarValue,
    },
};

lazy_static! {
    // Regex to capture potential tokens, with an optional escape character `\`.
    static ref TOKEN_RE: Regex = Regex::new(r"\\?<([^>]+)>").unwrap();
}

#[derive(Error, Debug)]
pub enum CompilerError {
    #[error("I/O error while processing configuration: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse TOML file at '{path}': {source}")]
    TomlParse {
        path: std::path::PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

// --- PUBLIC COMPILER API ---

/// Compiles a user-facing `TomlScript` into a platform-agnostic `Task` (AST).
/// This is the main entry point for script compilation.
pub fn compile_script(toml_script: TomlScript) -> Result<Task> {
    match toml_script {
        TomlScript::Simple(s) => {
            let platform_exec = compile_toml_command_to_platform_execution(TomlCommand::Simple(s))
                .context("Failed to compile simple script string")?;
            Ok(Task {
                desc: None,
                commands: vec![platform_exec],
            })
        }
        TomlScript::Sequence(commands) => {
            let compiled_commands = commands
                .into_iter()
                .map(compile_toml_command_to_platform_execution)
                .collect::<Result<Vec<_>>>()?;
            Ok(Task {
                desc: None,
                commands: compiled_commands,
            })
        }
        TomlScript::Platform(pc) => {
            let platform_exec = compile_platform_command_to_platform_execution(pc, true)?;
            Ok(Task {
                desc: None,
                commands: vec![platform_exec],
            })
        }
        TomlScript::PlatformDirect(pd) => {
            // This is simple: we have a platform block and a description.
            // We compile the platform block into a single PlatformExecution.
            let platform_exec = compile_platform_command_to_platform_execution(pd.platform, true)?;
            // And create a Task with that single command.
            Ok(Task {
                desc: pd.desc,
                commands: vec![platform_exec],
            })
        }
        TomlScript::Extended(ext) => {
            // Recursively compile the `run` field, which is a TomlScript.
            let mut task = compile_script(*ext.run)
                .with_context(|| "Failed to compile 'run' field of extended script")?;
            // Then, assign the description from the outer table.
            task.desc = ext.desc;
            Ok(task)
        }
    }
}

/// Compiles a user-facing `TomlVar` into a platform-agnostic `CachedVar` (AST).
/// This is the main entry point for variable compilation.
pub fn compile_var(toml_var: TomlVar) -> Result<CachedVar> {
    match toml_var {
        TomlVar::Simple(s) => {
            // A simple string is a value for the `default` platform.
            // Action prefixes are NOT parsed for variables.
            let command_exec = compile_string_to_command_execution(&s, false)?;
            Ok(CachedVar {
                desc: None,
                value: PlatformExecution {
                    default: Some(command_exec),
                    ..Default::default()
                },
            })
        }
        TomlVar::Extended(ext) => {
            let platform_exec = match ext.value {
                TomlVarValue::Simple(s) => {
                    let command_exec = compile_string_to_command_execution(&s, false)?;
                    PlatformExecution {
                        default: Some(command_exec),
                        ..Default::default()
                    }
                }
                TomlVarValue::Platform(platform_block) => {
                    compile_platform_command_to_platform_execution(platform_block, false)?
                }
            };
            Ok(CachedVar {
                desc: ext.desc,
                value: platform_exec,
            })
        }
    }
}

// --- MAIN TASK LOGIC (called by ConfigLoader) ---

/// The main entry point for compiling a project layer from `axes.toml` to `CachedProjectConfig`.
pub fn load_and_compile_layer(entry: &IndexEntry) -> Result<CachedProjectConfig> {
    let config_path = entry.path.join(".axes").join("axes.toml");
    if !config_path.exists() {
        return Ok(CachedProjectConfig::default());
    }

    let content = fs::read_to_string(&config_path)?;
    let project_config: ProjectConfig =
        toml::from_str(&content).map_err(|e| CompilerError::TomlParse {
            path: config_path,
            source: e,
        })?;

    // --- Compile each section with detailed error context ---

    let scripts = project_config
        .scripts
        .into_iter()
        .map(|(name, ts)| {
            compile_script(ts)
                .with_context(|| format!("Failed to compile script '{}'", name))
                // FIX: Keep the name to form a (key, value) tuple for the HashMap
                .map(|task| (name, task))
        })
        .collect::<Result<HashMap<_, _>>>()
        .context("Error compiling [scripts] section")?;

    let vars = project_config
        .vars
        .into_iter()
        .map(|(name, tv)| {
            compile_var(tv)
                .with_context(|| format!("Failed to compile var '{}'", name))
                // FIX: Keep the name to form a (key, value) tuple for the HashMap
                .map(|cvar| (name, cvar))
        })
        .collect::<Result<HashMap<_, _>>>()
        .context("Error compiling [vars] section")?;

    let open_with_commands = project_config
        .options
        .open_with
        .commands
        .into_iter()
        .map(|(name, ts)| {
            compile_script(ts)
                .with_context(|| format!("Failed to compile open_with command '{}'", name))
                // FIX: Keep the name to form a (key, value) tuple for the HashMap
                .map(|task| (name, task))
        })
        .collect::<Result<HashMap<_, _>>>()
        .context("Error compiling [options.open_with] commands")?;

    let cached_options = CachedOptionsConfig {
        at_start: project_config
            .options
            .at_start
            .map(compile_script)
            .transpose()
            .context("Error compiling [options.at_start]")?,
        at_exit: project_config
            .options
            .at_exit
            .map(compile_script)
            .transpose()
            .context("Error compiling [options.at_exit]")?,
        shell: project_config.options.shell,
        cache_dir: project_config.options.cache_dir,
        open_with: CachedOpenWithConfig {
            default: project_config.options.open_with.default,
            commands: open_with_commands,
        },
    };

    Ok(CachedProjectConfig {
        version: project_config.version,
        description: project_config.description,
        scripts,
        vars,
        env: project_config.env,
        options: cached_options,
    })
}

/// The task executed in parallel by `ConfigLoader` for each layer in the hierarchy.
/// It handles cache validation (hit/miss) and triggers re-compilation if necessary.
pub fn load_layer_task(
    uuid: Uuid,
    index: &GlobalIndex,
) -> Result<(std::sync::Arc<CachedProjectConfig>, Option<IndexUpdate>)> {
    log::debug!("Executing load task for UUID: {}", uuid);
    let entry = index
        .projects
        .get(&uuid)
        .ok_or_else(|| anyhow!("Project UUID {} not found in index.", uuid))?;

    let config_path = entry.path.join(".axes").join("axes.toml");
    let current_toml_hash = if config_path.exists() {
        cache::calculate_validation_data(&config_path)?.content_hash
    } else {
        // Use a consistent hash for non-existent or empty files.
        "empty".to_string()
    };

    log::trace!(
        "Layer '{}' at '{}' has content hash: {}",
        entry.name,
        entry.path.display(),
        current_toml_hash
    );

    // --- CACHE HIT ATTEMPT ---
    if let (Some(saved_hash), Some(cache_dir)) = (&entry.config_hash, &entry.cache_dir)
        && *saved_hash == current_toml_hash
    {
        log::trace!(
            "Layer '{}' has saved hash in index: {}",
            entry.name,
            saved_hash
        );
        let cache_file_path = cache_dir.join(saved_hash);
        if let Ok(cached_layer) = read_cached_layer(&cache_file_path) {
            log::debug!("Cache HIT for layer '{}'.", entry.name);
            return Ok((std::sync::Arc::new(cached_layer), None));
        }
        log::warn!(
            "Cache file for '{}' missing or corrupt. Forcing re-compilation.",
            entry.name
        );
    }

    // --- CACHE MISS: RECOMPILE ---
    log::debug!("Cache MISS for layer '{}'. Recompiling.", entry.name);

    let new_layer = load_and_compile_layer(&entry)?;

    let final_cache_dir = paths::resolve_cache_dir_for_project(uuid, index, &new_layer.options)?;
    let new_cache_file_path = final_cache_dir.join(&current_toml_hash);
    write_cached_layer(&new_cache_file_path, &new_layer)?;

    // Prepare an update object to be sent back to the main thread.
    let update = IndexUpdate {
        uuid,
        new_hash: current_toml_hash,
        new_cache_dir: final_cache_dir,
    };

    Ok((std::sync::Arc::new(new_layer), Some(update)))
}

// --- HELPER IMPLEMENTATIONS ---

/// A struct to hold the parsed execution prefixes from a command string.
#[derive(Debug, Default)]
pub struct Prefixes {
    pub ignore_errors: bool,
    pub run_in_parallel: bool,
    pub silent_mode: bool,
    pub is_echo: bool, // Represents the '#' prefix
}

/// Parses `axes` execution prefixes (`@`, `-`, `>`, `#`) from the start of a line.
/// Returns the parsed prefixes and a slice of the string containing the actual command.
pub fn parse_prefixes(line: &str) -> (Prefixes, &str) {
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
                // Terminator: stop parsing prefixes.
                current_pos = i + 1;
                break;
            }
            _ if !char.is_whitespace() => {
                // First non-prefix, non-whitespace character.
                current_pos = i;
                break;
            }
            _ => (), // Whitespace, continue.
        }
        if i == line.len() - 1 {
            current_pos = line.len();
        }
    }
    (prefixes, line.get(current_pos..).unwrap_or("").trim_start())
}

/// Transforms a command string into a sequence of `TemplateComponent`s.
/// This is the "tokenizer" of the compiler.
pub fn tokenize_string(text: &str) -> Result<Vec<TemplateComponent>> {
    // Pre-allocate vector capacity. A rough estimate is fine.
    let mut components = Vec::with_capacity(text.len() / 15); // Avg token length guess
    let mut last_index = 0;

    for caps in TOKEN_RE.captures_iter(text) {
        let full_match = caps.get(0).unwrap();

        // Add the literal text between the last match and this one.
        if last_index < full_match.start() {
            // This requires changing `TemplateComponent::Literal` to accept `Into<String>`.
            // For now, let's stick to `to_string` but note this for future deep optimization.
            components.push(TemplateComponent::Literal(
                text[last_index..full_match.start()].to_string(),
            ));
        }

        if full_match.as_str().starts_with('\\') {
            // Escaped token
            components.push(TemplateComponent::Literal(
                full_match.as_str()[1..].to_string(),
            ));
        } else {
            let content = caps.get(1).unwrap().as_str();

            // Add context to errors for easier debugging of malformed tokens.
            let component = parse_token_content(content, full_match.as_str())
                .with_context(|| format!("Failed to parse token: '{}'", full_match.as_str()))?;
            components.push(component);
        }
        last_index = full_match.end();
    }

    // Add any remaining literal text after the last match.
    if last_index < text.len() {
        components.push(TemplateComponent::Literal(text[last_index..].to_string()));
    }

    Ok(components)
}

/// [NEW HELPER FUNCTION] Parses the inner content of a token like `<...>`
/// This refactor makes `tokenize_string` cleaner and improves error handling.
fn parse_token_content(content: &str, full_match: &str) -> Result<TemplateComponent> {
    let trimmed_content = content.trim();
    if let Some(param_spec) = trimmed_content.strip_prefix("params::") {
        Ok(TemplateComponent::Parameter(
            parameters::parse_parameter_token(full_match, param_spec)?,
        ))
    } else if let Some(modifiers_str) = trimmed_content
        .strip_prefix("params")
        .and_then(|s| s.strip_prefix('('))
        .and_then(|s| s.strip_suffix(')'))
    {
        let modifiers = parameters::parse_parameter_modifiers_from_str(modifiers_str)?;
        Ok(TemplateComponent::GenericParams {
            literal: modifiers.literal,
        })
    } else if trimmed_content == "params" {
        Ok(TemplateComponent::GenericParams { literal: false })
    } else if let Some(color_name) = trimmed_content.strip_prefix('#') {
        Ok(TemplateComponent::Color(color::parse_color_name(
            color_name,
        )?))
    } else if let Some(run_spec) = trimmed_content.strip_prefix("run") {
        if let Some(cmd) = run_spec
            .strip_prefix("('")
            .and_then(|s| s.strip_suffix("')"))
        {
            Ok(TemplateComponent::Run(RunSpec::Literal(cmd.to_string())))
        } else {
            Err(anyhow!("Invalid run syntax"))
        }
    } else {
        match trimmed_content {
            "path" => Ok(TemplateComponent::Path),
            "name" => Ok(TemplateComponent::Name),
            "uuid" => Ok(TemplateComponent::Uuid),
            "version" => Ok(TemplateComponent::Version),
            s if s.starts_with("scripts::") => Ok(TemplateComponent::Script(
                s.strip_prefix("scripts::").unwrap().to_string(),
            )),
            s if s.starts_with("vars::") => Ok(TemplateComponent::Var(
                s.strip_prefix("vars::").unwrap().to_string(),
            )),
            _ => Err(anyhow!("Unknown token namespace")),
        }
    }
}

/// Compiles a single `TomlCommand` (one line of a script) into a `PlatformExecution` block.
fn compile_toml_command_to_platform_execution(
    toml_command: TomlCommand,
) -> Result<PlatformExecution> {
    match toml_command {
        TomlCommand::Simple(s) => {
            let command_exec = compile_string_to_command_execution(&s, true)?;
            Ok(PlatformExecution {
                default: Some(command_exec),
                ..Default::default()
            })
        }
        TomlCommand::Platform(platform_block) => {
            compile_platform_command_to_platform_execution(platform_block, true)
        }
    }
}

/// Compiles a `PlatformCommand` struct into a `PlatformExecution` struct.
fn compile_platform_command_to_platform_execution(
    platform_command: PlatformCommand,
    parse_action_prefixes: bool,
) -> Result<PlatformExecution> {
    // DRY Principle: Helper closure to avoid repetition.
    let compile_string = |s: String| -> Result<CommandExecution> {
        compile_string_to_command_execution(&s, parse_action_prefixes)
    };

    Ok(PlatformExecution {
        default: platform_command.default.map(compile_string).transpose()?,
        windows: platform_command.windows.map(compile_string).transpose()?,
        linux: platform_command.linux.map(compile_string).transpose()?,
        macos: platform_command.macos.map(compile_string).transpose()?,
    })
}

/// The lowest-level compilation function.
/// Takes a raw string, tokenizes it, and (optionally) parses `axes` prefixes.
fn compile_string_to_command_execution(
    s: &str,
    parse_action_prefixes: bool,
) -> Result<CommandExecution> {
    let (prefixes, command_text) = if parse_action_prefixes {
        parse_prefixes(s)
    } else {
        (Prefixes::default(), s)
    };

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

// --- CACHE I/O ---

/// Reads and deserializes a `CachedProjectConfig` from a binary file.
fn read_cached_layer(path: &std::path::Path) -> Result<CachedProjectConfig> {
    let bytes = fs::read(path)
        .with_context(|| format!("Failed to read cache file at '{}'", path.display()))?;
    let (cached_layer, _): (CachedProjectConfig, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).context(
            "Failed to deserialize cache file. It might be corrupt or from an older version.",
        )?;
    Ok(cached_layer)
}

/// Serializes and writes a `CachedProjectConfig` to a binary file.
fn write_cached_layer(path: &std::path::Path, layer: &CachedProjectConfig) -> Result<()> {
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
