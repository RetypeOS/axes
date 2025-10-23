//! # Compiler
//!
//! This module is responsible for the Ahead-of-Time (AOT) compilation of `axes.toml` files.
//! Its primary goal is to transform the user-friendly, flexible TOML syntax into a
//! platform-agnostic, optimized Abstract Syntax Tree (AST) that can be cached and executed efficiently.

use crate::{
    core::{cache, color, parameters, paths},
    models::{
        CachedOpenWithConfig, CachedOptionsConfig, CachedProjectConfig, CachedVar, CommandAction,
        CommandExecution, GlobalIndex, IndexEntry, IndexUpdate, PlatformCommand, PlatformExecution,
        ProjectConfig, RunSpec, Task, TemplateComponent, TomlCommand, TomlScript, TomlVar,
        TomlVarValue,
    },
};
use anyhow::{Context, Result, anyhow};
use lazy_static::lazy_static;
use lz4_flex;
use regex::Regex;
use std::{collections::HashMap, fs, path::PathBuf};
use thiserror::Error;
use uuid::Uuid;

lazy_static! {
    // Regex to capture potential tokens, with an optional escape character `\`.
    static ref TOKEN_RE: Regex = Regex::new(r"\\?<([^>]+)>").unwrap();
}

/// Represents errors that can occur during the compilation of `axes.toml` files.
#[derive(Error, Debug)]
pub enum CompilerError {
    /// An I/O error occurred while reading the configuration file.
    #[error("I/O error while processing configuration: {0}")]
    Io(#[from] std::io::Error),
    /// The TOML content of `axes.toml` is invalid and could not be parsed.
    #[error("Failed to parse TOML file at '{path}': {source}")]
    TomlParse {
        /// The path to the file that failed to parse.
        path: std::path::PathBuf,
        /// The underlying parsing error from the `toml` crate.
        #[source]
        source: toml::de::Error,
    },
}

// --- PUBLIC COMPILER API ---

/// Compiles a user-facing `TomlScript` into a platform-agnostic `Task` (AST).
/// This is the main entry point for script compilation.
///
/// # Arguments
///
/// * `toml_script` - The `TomlScript` to compile.
///
/// # Returns
///
/// A `Result` containing the compiled `Task` on success, or an error if compilation fails.
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
///
/// # Arguments
///
/// * `toml_var` - The `TomlVar` to compile.
///
/// # Returns
///
/// A `Result` containing the compiled `CachedVar` on success, or an error if compilation fails.
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
///
/// # Arguments
///
/// * `entry` - The `IndexEntry` for the project to compile.
///
/// # Returns
///
/// A `Result` containing the compiled `CachedProjectConfig` on success, or an error if compilation fails.
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
        prompt: project_config.options.prompt,
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
///
/// # Arguments
///
/// * `uuid` - The UUID of the project to load.
/// * `index` - A reference to the `GlobalIndex`.
///
/// # Returns
///
/// A `Result` containing a tuple of the loaded `CachedProjectConfig` and an optional `IndexUpdate`
/// on success, or an error if loading fails.
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

    let cache_dir_to_check = entry
        .cache_dir
        .clone()
        .unwrap_or_else(|| paths::get_default_cache_dir_for_project(uuid).unwrap());

    if let Some(saved_hash) = &entry.config_hash
        && saved_hash == &current_toml_hash
    {
        let cache_file_path = cache_dir_to_check.join(saved_hash);
        if let Ok(cached_layer) = read_cached_layer(&cache_file_path) {
            log::debug!(
                "Cache HIT for layer '{}' at '{}'.",
                entry.name,
                cache_file_path.display()
            );
            return Ok((std::sync::Arc::new(cached_layer), None));
        }
        log::warn!(
            "Cache file for '{}' missing or corrupt. Forcing re-compilation.",
            entry.name
        );
    }

    // --- CACHE MISS: RECOMPILE ---
    log::debug!("Cache MISS for layer '{}'. Recompiling.", entry.name);

    let new_layer = load_and_compile_layer(entry)?;

    // We just return an IndexUpdate with the new hash. The ConfigLoader will handle the rest.
    let update = IndexUpdate {
        uuid,
        new_hash: current_toml_hash,
        // We leave the final cache_dir path to be resolved by the ConfigLoader.
        new_cache_dir: PathBuf::new(),
    };

    // We still need to write the new cache file somewhere. We'll use the path we checked.
    let new_cache_file_path = cache_dir_to_check.join(&update.new_hash);
    write_cached_layer(&new_cache_file_path, &new_layer)?;

    Ok((std::sync::Arc::new(new_layer), Some(update)))
}

// --- HELPER IMPLEMENTATIONS ---

/// A struct to hold the parsed execution prefixes from a command string.
#[derive(Debug, Default)]
pub struct Prefixes {
    /// Ignore errors.
    pub ignore_errors: bool,
    /// Run in parallel.
    pub run_in_parallel: bool,
    /// Silent mode.
    pub silent_mode: bool,
    /// Represents the '#' prefix.
    pub is_echo: bool,
}

/// Parses `axes` execution prefixes (`@`, `-`, `>`, `#`) from the start of a line.
/// Returns the parsed prefixes and a slice of the string containing the actual command.
///
/// # Arguments
///
/// * `line` - The line to parse.
///
/// # Returns
///
/// A tuple containing the parsed `Prefixes` and a slice of the string containing the actual command.
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

/// Transforms a command string into a sequence of `TemplateComponent`s,
/// merging adjacent literal components for optimization.
///
/// # Arguments
///
/// * `text` - The text to tokenize.
///
/// # Returns
///
/// A `Result` containing a vector of `TemplateComponent`s on success, or an error if tokenization fails.
pub fn tokenize_string(text: &str) -> Result<Vec<TemplateComponent>> {
    let mut components = Vec::with_capacity(text.len() / 20);

    // Helper to push literals and handle merging.
    let push_literal = |components: &mut Vec<TemplateComponent>, s: &str| {
        if s.is_empty() {
            return;
        }
        if let Some(TemplateComponent::Literal(last)) = components.last_mut() {
            last.push_str(s);
        } else {
            components.push(TemplateComponent::Literal(s.to_string()));
        }
    };

    let mut last_index = 0;
    for caps in TOKEN_RE.captures_iter(text) {
        let full_match = caps.get(0).unwrap();

        // Push the literal part before the match.
        push_literal(&mut components, &text[last_index..full_match.start()]);

        if full_match.as_str().starts_with('\\') {
            // It's an escaped token. Treat its content (without the '\') as a literal.
            push_literal(&mut components, &full_match.as_str()[1..]);
        } else {
            // It's a real token.
            let content = caps.get(1).unwrap().as_str();
            let component = parse_token_content(content, full_match.as_str())
                .with_context(|| format!("Failed to parse token: '{}'", full_match.as_str()))?;
            components.push(component);
        }
        last_index = full_match.end();
    }

    // Push any remaining literal text after the last token.
    push_literal(&mut components, &text[last_index..]);

    Ok(components)
}

/// Parses the inner content of a token like `<...>`
/// This refactor makes `tokenize_string` cleaner and improves error handling.
///
/// # Arguments
///
/// * `content` - The content of the token to parse.
/// * `full_match` - The full match of the token.
///
/// # Returns
///
/// A `Result` containing the parsed `TemplateComponent` on success, or an error if parsing fails.
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
        Ok(TemplateComponent::Color(color::parse_style_name(
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
///
/// # Arguments
///
/// * `toml_command` - The `TomlCommand` to compile.
///
/// # Returns
///
/// A `Result` containing the compiled `PlatformExecution` on success, or an error if compilation fails.
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
///
/// # Arguments
///
/// * `platform_command` - The `PlatformCommand` to compile.
/// * `parse_action_prefixes` - Whether to parse action prefixes.
///
/// # Returns
///
/// A `Result` containing the compiled `PlatformExecution` on success, or an error if compilation fails.
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
///
/// # Arguments
///
/// * `s` - The string to compile.
/// * `parse_action_prefixes` - Whether to parse action prefixes.
///
/// # Returns
///
/// A `Result` containing the compiled `CommandExecution` on success, or an error if compilation fails.
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

/// Reads and deserializes a `CachedProjectConfig` from a compressed binary file.
/// This function is on the hot path for "cache hit" scenarios.
///
/// # Arguments
///
/// * `path` - The path to the cache file.
///
/// # Returns
///
/// A `Result` containing the deserialized `CachedProjectConfig` on success, or an error if the file
/// cannot be read or deserialized.
fn read_cached_layer(path: &std::path::Path) -> Result<CachedProjectConfig> {
    // 1. Read the compressed bytes from disk.
    let compressed_bytes = fs::read(path)
        .with_context(|| format!("Failed to read cache file at '{}'", path.display()))?;

    // ROBUSTNESS: Handle empty file case gracefully.
    if compressed_bytes.is_empty() {
        return Err(anyhow!("Cache file is empty."));
    }

    // 2. Decompress the data using LZ4.
    // `decompress_size_prepended` is extremely fast and safe. It reads the expected
    // decompressed size from the first few bytes, preventing DoS attacks with "zip bombs".
    log::trace!(
        "Decompressing cache layer from {} bytes.",
        compressed_bytes.len()
    );
    let decompressed_bytes =
        lz4_flex::decompress_size_prepended(&compressed_bytes).map_err(|e| {
            anyhow!(
                "Failed to decompress cache file: {}. It might be corrupt.",
                e
            )
        })?;
    log::trace!("Decompressed to {} bytes.", decompressed_bytes.len());

    // 3. Deserialize the raw bytes using bincode.
    let (cached_layer, _): (CachedProjectConfig, usize) =
        bincode::serde::decode_from_slice(&decompressed_bytes, bincode::config::standard())
            .context("Failed to deserialize cache data after decompression. The cache is likely from an incompatible version of `axes`.")?;

    Ok(cached_layer)
}

/// Serializes and writes a `CachedProjectConfig` to a compressed binary file.
/// This function is on the "cold path" (cache miss).
///
/// # Arguments
///
/// * `path` - The path to the cache file.
/// * `layer` - The `CachedProjectConfig` to serialize.
///
/// # Returns
///
/// A `Result` indicating success or failure.
fn write_cached_layer(path: &std::path::Path, layer: &CachedProjectConfig) -> Result<()> {
    // 1. Ensure the parent directory exists.
    if let Some(parent_dir) = path.parent() {
        fs::create_dir_all(parent_dir).with_context(|| {
            format!(
                "Failed to create cache directory '{}'",
                parent_dir.display()
            )
        })?;
    }

    // 2. Serialize the AST to raw bytes using bincode.
    let decompressed_bytes = bincode::serde::encode_to_vec(layer, bincode::config::standard())
        .context("Failed to serialize cache layer.")?;
    log::trace!(
        "Serialized cache layer to {} bytes.",
        decompressed_bytes.len()
    );

    // 3. Compress the raw bytes using LZ4.
    // `compress_prepend_size` is very fast and adds a small header with the original
    // size, which is used for safe decompression.
    let compressed_bytes = lz4_flex::compress_prepend_size(&decompressed_bytes);
    log::trace!(
        "Compressed cache layer to {} bytes.",
        compressed_bytes.len()
    );

    // 4. Write the final compressed bytes to disk.
    fs::write(path, &compressed_bytes).with_context(|| {
        format!(
            "Failed to write compressed cache file to '{}'",
            path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{TemplateComponent, TomlScriptExtended, TomlVarExtended, TomlVarValue};

    // --- Script Compilation Tests ---

    #[test]
    fn test_compile_simple_script_with_prefixes() {
        let script = TomlScript::Simple("@-echo 'hello'".to_string());
        let task = compile_script(script).unwrap();
        assert_eq!(task.commands.len(), 1);
        let exec = task.commands[0].default.as_ref().unwrap();
        assert!(exec.silent_mode);
        assert!(exec.ignore_errors);
        assert!(!exec.run_in_parallel);
        assert!(matches!(exec.action, CommandAction::Execute(_)));
    }

    #[test]
    fn test_compile_sequence_script_mixed() {
        let script = TomlScript::Sequence(vec![
            TomlCommand::Simple("# Step 1".to_string()),
            TomlCommand::Platform(PlatformCommand {
                windows: Some("dir".to_string()),
                default: Some("ls -la".to_string()),
                ..Default::default()
            }),
        ]);
        let task = compile_script(script).unwrap();
        assert_eq!(task.commands.len(), 2);

        let first_cmd = task.commands[0].default.as_ref().unwrap();
        assert!(matches!(first_cmd.action, CommandAction::Print(_)));

        let second_cmd = &task.commands[1];
        assert!(second_cmd.windows.is_some());
        assert!(second_cmd.default.is_some());
        assert!(second_cmd.linux.is_none());
    }

    #[test]
    fn test_compile_extended_script_with_run_sequence() {
        let script = TomlScript::Extended(TomlScriptExtended {
            desc: Some("A complex script".to_string()),
            run: Box::new(TomlScript::Sequence(vec![
                TomlCommand::Simple("cmd1".to_string()),
                TomlCommand::Simple("cmd2".to_string()),
            ])),
        });
        let task = compile_script(script).unwrap();
        assert_eq!(task.desc.as_deref(), Some("A complex script"));
        assert_eq!(task.commands.len(), 2);
    }

    #[test]
    fn test_compile_platform_direct_script() {
        let script = TomlScript::PlatformDirect(crate::models::TomlScriptPlatformDirect {
            desc: Some("Platform direct".to_string()),
            platform: PlatformCommand {
                windows: Some("echo 'win'".to_string()),
                ..Default::default()
            },
        });
        let task = compile_script(script).unwrap();
        assert_eq!(task.desc.as_deref(), Some("Platform direct"));
        assert_eq!(task.commands.len(), 1);
        assert!(task.commands[0].windows.is_some());
        assert!(task.commands[0].default.is_none());
    }

    // --- Variable Compilation Tests ---

    #[test]
    fn test_compile_simple_var_no_prefixes() {
        let var = TomlVar::Simple("@-my-value".to_string());
        let cached_var = compile_var(var).unwrap();
        let exec = cached_var.value.default.as_ref().unwrap();

        // CRITICAL: Prefixes must be ignored for variables.
        assert!(!exec.silent_mode);
        assert!(!exec.ignore_errors);

        if let CommandAction::Execute(tpl) = &exec.action {
            if let TemplateComponent::Literal(s) = &tpl[0] {
                assert_eq!(s, "@-my-value");
            } else {
                panic!("Expected literal");
            }
        } else {
            panic!("Expected Execute action");
        }
    }

    #[test]
    fn test_compile_extended_var_platform() {
        let var = TomlVar::Extended(TomlVarExtended {
            desc: Some("Path to binary".to_string()),
            value: TomlVarValue::Platform(PlatformCommand {
                windows: Some("bin\\app.exe".to_string()),
                default: Some("bin/app".to_string()),
                ..Default::default()
            }),
        });
        let cached_var = compile_var(var).unwrap();
        assert_eq!(cached_var.desc.as_deref(), Some("Path to binary"));
        assert!(cached_var.value.windows.is_some());
        assert!(cached_var.value.default.is_some());
    }

    // --- Error Handling and Edge Case Tests ---

    #[test]
    fn test_tokenizer_handles_escaped_tokens_and_merges_literals() {
        let text = r"echo '\<hello> world <name>'";
        let components = tokenize_string(text).unwrap();
        assert_eq!(components.len(), 3);
        assert!(
            matches!(&components[0], TemplateComponent::Literal(s) if s == "echo '<hello> world ")
        );
        assert!(matches!(&components[1], TemplateComponent::Name));
        assert!(matches!(&components[2], TemplateComponent::Literal(s) if s == "'"));
    }

    #[test]
    fn test_tokenizer_with_complex_tokens() {
        let text = r"<run('git status')> <params::0(required)> <#red>ERROR<#reset>";
        let components = tokenize_string(text).unwrap();
        assert_eq!(components.len(), 7);
        assert!(matches!(&components[0], TemplateComponent::Run(_)));
        assert!(matches!(&components[1], TemplateComponent::Literal(s) if s == " "));
        assert!(matches!(&components[2], TemplateComponent::Parameter(_)));
        assert!(matches!(&components[3], TemplateComponent::Literal(s) if s == " "));
        assert!(
            matches!(&components[4], TemplateComponent::Color(c) if *c == crate::models::AnsiStyle::Red)
        );
        assert!(matches!(&components[5], TemplateComponent::Literal(s) if s == "ERROR"));
        assert!(
            matches!(&components[6], TemplateComponent::Color(c) if *c == crate::models::AnsiStyle::Reset)
        );
    }

    #[test]
    fn test_empty_script_compiles_to_empty_task() {
        let script = TomlScript::Simple("".to_string());
        let task = compile_script(script).unwrap();
        assert_eq!(task.commands.len(), 1); // An empty command is still a command
        let exec = task.commands[0].default.as_ref().unwrap();
        if let CommandAction::Execute(tpl) = &exec.action {
            assert!(tpl.is_empty());
        } else {
            panic!("Expected Execute action");
        }
    }

    #[test]
    fn test_toml_deserialization_error_for_unknown_field_in_script() {
        let toml_str = r#"
            desc = "A script with a typo"
            runs = "echo 'hello'" # Typo: should be `run`
        "#;
        // FIX: We are testing the `TomlScriptExtended` struct directly.
        let result: Result<TomlScriptExtended, _> = toml::from_str(toml_str);
        assert!(result.is_err(), "Should fail due to unknown field 'runs'");
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("unknown field `runs`"),
            "Error message was: {}",
            error_msg
        );
    }

    #[test]
    fn test_toml_deserialization_error_for_unknown_field_in_var() {
        let toml_str = r#"
            desc = "A var with a typo"
            val = "my-value" # Typo: should be `value`
        "#;
        // FIX: We are testing the `TomlVarExtended` struct directly.
        let result: Result<TomlVarExtended, _> = toml::from_str(toml_str);
        assert!(result.is_err(), "Should fail due to unknown field 'val'");
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("unknown field `val`"),
            "Error message was: {}",
            error_msg
        );
    }
}
