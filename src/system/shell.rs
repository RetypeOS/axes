// src/system/shell.rs

use crate::{
    CancellationToken,
    core::{parameters::ArgResolver, task_executor},
    models::{ResolvedConfig, ShellConfig, ShellsConfig, Task},
};
use anyhow::Result;
use colored::Colorize;
use std::{collections::HashMap, env, fs, path::PathBuf, process::Command};
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

///
/// Launches an interactive session shell, executing `at_start` within the new shell's
/// context and `at_exit` after the shell terminates.
///
pub fn launch_session(
    config: &ResolvedConfig,
    task_start: Option<Task>,
    task_exit: Option<Task>,
    resolver: &ArgResolver,
    cancellation_token: &CancellationToken,
) -> Result<(), ShellError> {
    let shells_config = load_shells_config()?;
    let shell_name = config
        .options
        .shell
        .as_deref()
        .unwrap_or(get_default_shell_name());
    let shell_config = shells_config
        .shells
        .get(shell_name)
        .ok_or_else(|| ShellError::ShellNotDefined(shell_name.to_string()))?;

    // 1. Assemble the commands for the `at_start` task. These will be written to a temporary script.
    let at_start_final_commands = if let Some(task) = &task_start {
        println!("\n{}", "Preparing `at_start` hook...".dimmed());
        task.commands
            .iter()
            .map(|cmd| task_executor::assemble_final_command(&cmd.template, resolver))
            .collect::<Result<Vec<String>>>()?
    } else {
        Vec::new()
    };

    // 2. Build and launch the interactive shell with the temporary init script.
    let is_windows_shell = shell_name == "cmd" || shell_name == "powershell";
    let script_extension = if is_windows_shell { ".bat" } else { ".sh" };
    let temp_script_file = NamedTempFile::with_prefix("axes-init-")?.into_temp_path();
    let temp_script_path = temp_script_file.with_extension(script_extension);

    let script_content = build_init_script(config, &at_start_final_commands, is_windows_shell);
    fs::write(&temp_script_path, script_content)?;
    log::debug!(
        "Temporary init script created at: {}",
        temp_script_path.display()
    );

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

    cmd.envs(&config.env);

    if let Some(args) = &shell_config.interactive_args {
        for arg in args {
            cmd.arg(arg);
        }
        cmd.arg(&temp_script_path);
    }

    let status = cmd.status()?;
    if !status.success() {
        log::warn!("Interactive shell finished with code: {:?}", status.code());
    }

    let _ = fs::remove_file(&temp_script_path);

    // 3. Execute the `at_exit` task if it exists.
    if let Some(task) = &task_exit {
        println!("\n{}", "\nExecuting `at_exit` hook...".dimmed());
        task_executor::execute_task(task, config, resolver, cancellation_token)?;
    }

    Ok(())
}

fn build_init_script(
    config: &ResolvedConfig,
    at_start_commands: &[String],
    is_windows: bool,
) -> String {
    let mut script = String::new();
    if is_windows {
        script.push_str("@echo off\n");
    }

    for (key, value) in &config.env {
        if is_windows {
            script.push_str(&format!("set \"{}={}\"\n", key, value));
        } else {
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
    script
}

// --- Shells.toml Management ---

fn load_shells_config() -> Result<ShellsConfig, ShellError> {
    let config_dir =
        crate::core::paths::get_axes_config_dir().map_err(|_| ShellError::ConfigDirNotFound)?;
    let shells_path = config_dir.join("shells.toml");
    if !shells_path.exists() {
        let default_config = generate_default_shells_config();
        let toml_string = toml::to_string_pretty(&default_config)?;
        fs::write(&shells_path, toml_string)?;
        return Ok(default_config);
    }
    let content = fs::read_to_string(shells_path)?;
    Ok(toml::from_str(&content)?)
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
