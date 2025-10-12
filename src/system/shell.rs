// EN: src/system/shell.rs (REBUILT FOR LAZY ARCHITECTURE)

use crate::{
    core::{parameters::ArgResolver, task_executor},
    // FIX: `Task` is now passed as `Arc<Task>`, `GlobalIndex` is needed for lazy resolution.
    models::{CommandAction, GlobalIndex, ResolvedConfig, ShellConfig, ShellsConfig, Task},
};
use anyhow::Result;
use colored::Colorize;
use std::{collections::HashMap, env, fs, path::PathBuf, process::Command, sync::Arc};
use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShellError {
    #[error("Filesystem Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Error with temporary file: {0}")]
    TempFile(#[from] tempfile::PersistError),
    #[error("Could not find axes config directory.")]
    ConfigDirNotFound,
    #[error("Requested shell '{0}' is not defined in shells.toml.")]
    ShellNotDefined(String),
    #[error("Could not determine a default shell for this operating system.")]
    NoDefaultShell,
    #[error("Failed to parse shells.toml: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("Failed to serialize shells config to TOML: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("Task execution failed: {0}")]
    TaskExecution(#[from] anyhow::Error),
}

/// Launches an interactive shell session for a project.
///
/// This function orchestrates the entire session lifecycle:
/// 1.  It lazily resolves the project's configuration to determine the correct shell and environment.
/// 2.  It renders the `at_start` hook commands into a temporary initialization script.
/// 3.  It spawns a new interactive shell process, configured to run the init script upon startup.
///     This process inherits the project's environment variables.
/// 4.  After the user exits the shell, it executes the `at_exit` hook.
///
/// # Arguments
/// * `config` - The lazy `ResolvedConfig` facade for the project.
/// * `task_start` - An `Option<Arc<Task>>` for the `at_start` hook, obtained lazily by the `start` handler.
/// * `task_exit` - An `Option<Arc<Task>>` for the `at_exit` hook.
/// * `resolver` - The `ArgResolver` for any parameters passed to the `axes start` command.
/// * `index` - A mutable reference to the `GlobalIndex` for further lazy resolutions.
pub fn launch_session(
    config: &ResolvedConfig,
    task_start: Option<Arc<Task>>,
    task_exit: Option<Arc<Task>>,
    resolver: &ArgResolver,
    index: &mut GlobalIndex,
) -> Result<(), ShellError> {
    // 1. Determine which shell to use.
    let shells_config = load_shells_config()?;
    // Lazily get options.
    let options = config.get_options()?;
    let shell_name: String = options
        .shell
        .clone()
        .unwrap_or_else(|| get_default_shell_name().to_string());
    let shell_config = shells_config
        .shells
        .get(&shell_name)
        .ok_or_else(|| ShellError::ShellNotDefined(shell_name.clone()))?;

    // 2. If an `at_start` task exists, render its commands into a script.
    let at_start_final_commands = if let Some(task) = &task_start {
        println!(
            "\n{}",
            format!(t!("start.info.preparing_hook"), hook = "at_start").dimmed()
        );
        // Start recursion depth at 0 for rendering.
        task.commands
            .iter()
            .map(|cmd| match &cmd.action {
                CommandAction::Execute(template) => {
                    task_executor::assemble_final_command(template, config, resolver, index, 0)
                }
                // Print actions in `at_start` are rendered as `echo` commands.
                CommandAction::Print(template) => {
                    let text = task_executor::assemble_final_command(
                        template, config, resolver, index, 0,
                    )?;
                    Ok(format!("echo \"{}\"", text.replace('"', "\\\"")))
                }
            })
            .collect::<Result<Vec<String>>>()?
    } else {
        Vec::new()
    };

    // 3. Create the temporary initialization script.
    let is_windows_shell = shell_name == "cmd" || shell_name == "powershell";
    let script_extension = if is_windows_shell { "bat" } else { "sh" };
    let temp_script_file = NamedTempFile::with_prefix("axes-init-")?.into_temp_path();
    let temp_script_path = temp_script_file.with_extension(script_extension);

    let script_content = build_init_script(config, &at_start_final_commands, is_windows_shell)?;
    fs::write(&temp_script_path, &script_content)?;
    log::debug!(
        "Temporary init script created at: {}",
        temp_script_path.display()
    );

    // 4. Spawn the interactive shell process.
    println!(
        "\n--- {} '{}' {}. ---",
        "axes session for".green(),
        config.qualified_name.yellow().bold(),
        "started".green()
    );

    let mut cmd = Command::new(&shell_config.path);
    cmd.current_dir(&config.project_root);
    cmd.env("AXES_PROJECT_ROOT", config.project_root.as_os_str());
    cmd.env("AXES_PROJECT_NAME", &config.qualified_name);
    cmd.env("AXES_PROJECT_UUID", config.uuid.to_string());

    let env_vars = config.get_env()?;
    cmd.envs(&*env_vars);

    if let Some(args) = &shell_config.interactive_args {
        cmd.args(args);
        cmd.arg(&temp_script_path);
    }

    let status = cmd.status()?;
    if !status.success() {
        log::warn!("Interactive shell exited with code: {:?}", status.code());
    }

    let _ = fs::remove_file(&temp_script_path);

    // 5. Execute the `at_exit` task if it exists.
    if let Some(task) = &task_exit {
        println!(
            "\n{}",
            format!(t!("start.info.executing_hook"), hook = "at_exit").dimmed()
        );
        task_executor::execute_task(task, config, resolver, index)?;
    }

    Ok(())
}

/// Helper function to escape a string for safe use within a `cmd.exe` `set "KEY=VALUE"` command.
/// It handles special characters that could otherwise terminate the command or be interpreted by the shell.
fn escape_for_cmd_set(value: &str) -> String {
    value
        .replace('%', "%%") // Escape percent signs
        .replace('^', "^^") // Escape carets (escape character itself)
        .replace('&', "^&") // Escape ampersands (command separator)
        .replace('<', "^<") // Escape less than (redirection)
        .replace('>', "^>") // Escape greater than (redirection)
        .replace('|', "^|") // Escape pipes (command piping)
}

/// Builds the content of the temporary shell initialization script.
/// This script sets environment variables and then runs the `at_start` commands.
fn build_init_script(
    config: &ResolvedConfig,
    at_start_commands: &[String],
    is_windows: bool,
    //index: &mut GlobalIndex,
) -> Result<String> {
    let mut script = String::new();
    if is_windows {
        script.push_str("@echo off\n");
    }

    // Lazily resolve the fully merged environment and write export/set commands.
    for (key, value) in &*config.get_env()? {
        if is_windows {
            // Basic escaping for cmd.exe
            let escaped_value = escape_for_cmd_set(value);
            script.push_str(&format!("set \"{}={}\"\n", key, escaped_value));
        } else {
            // Escaping for POSIX shells
            let escaped_value = value.replace('\'', "'\\''");
            script.push_str(&format!("export {}='{}'\n", key, escaped_value));
        }
    }
    script.push('\n');

    for command in at_start_commands {
        if !command.trim().is_empty() {
            script.push_str(command);
            script.push('\n');
        }
    }

    let exit_message = "--- Type 'exit' to leave. ---";
    if is_windows {
        script.push_str(&format!("\necho.\necho {}\necho.", exit_message));
    } else {
        script.push_str(&format!("\necho ''\necho '{}'\necho ''\n", exit_message));
    }
    Ok(script)
}

// --- Shells.toml Management (No changes needed in this section) ---

fn load_shells_config() -> Result<ShellsConfig, ShellError> {
    let config_dir =
        crate::core::paths::get_axes_config_dir().map_err(|_| ShellError::ConfigDirNotFound)?;
    let shells_path = config_dir.join("shells.toml");
    if !shells_path.exists() {
        let default_config = generate_default_shells_config();
        let toml_string = toml::to_string_pretty(&default_config)?;
        fs::write(&shells_path, toml_string)?;
        Ok(default_config)
    } else {
        let content = fs::read_to_string(shells_path)?;
        Ok(toml::from_str(&content)?)
    }
}

fn generate_default_shells_config() -> ShellsConfig {
    let mut shells = HashMap::new();
    if cfg!(target_os = "windows") {
        shells.insert(
            "cmd".to_string(),
            ShellConfig {
                path: PathBuf::from("cmd.exe"),
                interactive_args: Some(vec!["/K".to_string()]),
            },
        );
        if is_executable_in_path("powershell.exe") {
            shells.insert(
                "powershell".to_string(),
                ShellConfig {
                    path: PathBuf::from("powershell.exe"),
                    interactive_args: Some(vec!["-NoExit".to_string(), "-File".to_string()]),
                },
            );
        }
    }
    let bash_path_str = if cfg!(target_os = "windows") {
        "bash.exe"
    } else {
        "bash"
    };
    if is_executable_in_path(bash_path_str) {
        shells.insert(
            "bash".to_string(),
            ShellConfig {
                path: PathBuf::from(bash_path_str),
                interactive_args: Some(vec!["--rcfile".to_string()]),
            },
        );
    }
    ShellsConfig { shells }
}

fn is_executable_in_path(executable_name: &str) -> bool {
    if let Ok(path_var) = env::var("PATH") {
        for path in env::split_paths(&path_var) {
            if path.join(executable_name).is_file() {
                return true;
            }
        }
    }
    false
}

fn get_default_shell_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "bash"
    }
}
