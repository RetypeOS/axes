// src/system/shell.rs

use crate::CancellationToken;
use crate::models::{ResolvedConfig, ShellConfig, ShellsConfig};
use crate::system::executor;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command};
use std::{env, fs};
use tempfile::NamedTempFile;
use thiserror::Error;

use colored::Colorize;

use crate::core::interpolator::Interpolator;

#[derive(Error, Debug)]
pub enum ShellError {
    #[error("Filesystem Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Error con el archivo temporal: {0}")]
    TempFile(#[from] tempfile::PersistError),
    #[error("Could not find axes config directory.")]
    ConfigDirNotFound,
    #[error("Requested shell '{0}' is not defined in shells.toml.")]
    ShellNotDefined(String),
    #[error("No se pudo determinar una shell por defecto para este sistema operativo.")]
    NoDefaultShell,
    #[error("Error al parsear shells.toml: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("Error serializing shells config to TOML: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("Failed to expand tokens in command: {0}")]
    InterpolationFailed(String),
}

/// Launches an interactive sub-shell for a project.
/// Launches an interactive sub-shell for a project.
pub fn launch_interactive_shell(
    config: &ResolvedConfig,
    cancellation_token: &CancellationToken,
) -> Result<(), ShellError> {
    let shells_config = load_shells_config()?;

    // 1. Determine which shell to use
    let shell_name = config
        .options
        .shell
        .as_deref()
        .unwrap_or(get_default_shell_name());
    let shell_config = shells_config
        .shells
        .get(shell_name)
        .ok_or_else(|| ShellError::ShellNotDefined(shell_name.to_string()))?;

    // 2. Create the interpolator EARLY. FIXED.
    let mut interpolator = Interpolator::new(config);

    // 3. Expand the `at_start` command BEFORE building the script.
    let expanded_at_start_cmd = match &config.options.at_start {
        Some(cmd) if !cmd.trim().is_empty() => Some(
            interpolator
                .expand_string(cmd, cancellation_token)
                .map_err(|e| ShellError::InterpolationFailed(e.to_string()))?,
        ),
        _ => None,
    };

    // 4. Build the temporary initialization script using the EXPANDED command.
    let is_windows_shell = shell_name == "cmd" || shell_name == "powershell";
    let script_extension = if is_windows_shell { ".bat" } else { ".sh" };
    let temp_script_file = NamedTempFile::with_prefix("axes-init-")
        .unwrap()
        .into_temp_path()
        .with_extension(script_extension);

    let script_content = build_init_script(
        config,
        expanded_at_start_cmd.as_deref(),
        is_windows_shell,
    );
    fs::write(&temp_script_file, script_content)?;
    log::debug!("Temporary init script created at: {}", temp_script_file.display());

    // 5. Build and execute the shell command
    let mut cmd = Command::new(&shell_config.path);
    cmd.current_dir(&config.project_root);
    cmd.env("AXES_PROJECT_ROOT", config.project_root.as_os_str());
    cmd.env("AXES_PROJECT_NAME", &config.qualified_name);
    cmd.env("AXES_PROJECT_UUID", config.uuid.to_string());

    if let Some(args) = &shell_config.interactive_args {
        for arg in args {
            cmd.arg(arg);
        }
        cmd.arg(&temp_script_file);
    }
    
    let status = cmd.status()?;
    if !status.success() {
        log::warn!("Interactive shell finished with code: {:?}", status.code());
    }

    // 6. Execute the `at_exit` hook, using the same interpolator instance.
    if let Some(at_exit_command) = &config.options.at_exit {
        if !at_exit_command.trim().is_empty() {
            println!("\n{}", t!("shell.info.executing_at_exit").dimmed());

            let final_command = interpolator
                .expand_string(at_exit_command, cancellation_token)
                .map_err(|e| ShellError::InterpolationFailed(e.to_string()))?;

            if let Err(e) = executor::execute_command(
                &final_command,
                &config.project_root,
                &config.env,
                cancellation_token,
            ) {
                eprintln!("\n{}", format!(t!("shell.warning.at_exit_failed"), error = e).yellow());
            }
        }
    }

    Ok(())
}

/// Builds the content of the initialization script.
fn build_init_script(
    config: &ResolvedConfig,
    expanded_at_start: Option<&str>,
    is_windows: bool,
) -> String {
    let mut script = String::new();

    if is_windows {
        script.push_str("@echo off\n");
    }

    // Add [env] variables
    for (key, value) in &config.env {
        if is_windows {
            script.push_str(&format!("set \"{}={}\"\n", key, value));
        } else {
            script.push_str(&format!("export {}='{}'\n", key, value));
        }
    }

    // Use the pre-expanded `at_start` command
    if let Some(at_start) = expanded_at_start {
        if !at_start.trim().is_empty() {
             if is_windows {
                script.push_str(&format!("call {}\n", at_start));
            } else {
                script.push_str(&format!("source \"{}\" || . \"{}\"\n", at_start, at_start));
            }
        }
    }

    // Welcome message
    let welcome_message = format!(t!("shell.info.welcome"), name = config.qualified_name);
    if is_windows {
        script.push_str(&format!("\necho.\necho {}\n", welcome_message));
    } else {
        script.push_str(&format!("\necho ''\necho '{}'\n", welcome_message));
    }

    script
}

/// Loads shell configuration from disk.
/// If the file does not exist, it generates it with default values and saves it.
fn load_shells_config() -> Result<ShellsConfig, ShellError> {
    let config_dir =
        crate::core::paths::get_axes_config_dir().map_err(|_| ShellError::ConfigDirNotFound)?;
    let shells_path = config_dir.join("shells.toml");

    if !shells_path.exists() {
        log::warn!("'shells.toml' not found. Generating default config file.");
        let default_config = generate_default_shells_config();
        let toml_string = toml::to_string_pretty(&default_config)?;
        fs::write(&shells_path, toml_string)?;
        println!("Shells config file created at: {}", shells_path.display());
        return Ok(default_config);
    }

    let content = fs::read_to_string(shells_path)?;
    Ok(toml::from_str(&content)?)
}

/// Generates a default shell configuration, detecting what is available.
fn generate_default_shells_config() -> ShellsConfig {
    let mut shells = HashMap::new();

    // Always add `cmd` on Windows.
    if cfg!(target_os = "windows") {
        shells.insert(
            "cmd".to_string(),
            ShellConfig {
                path: PathBuf::from("cmd.exe"),
                interactive_args: Some(vec!["/K".to_string()]),
            },
        );

        // Try to detect PowerShell.
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

    // Try to detect `bash` on any system.
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

    // More detectors for zsh, fish, etc. could be added here.

    ShellsConfig { shells }
}

/// Checks if an executable exists in the system's PATH.
fn is_executable_in_path(executable_name: &str) -> bool {
    if let Ok(path_var) = env::var("PATH") {
        for path in env::split_paths(&path_var) {
            let full_path = path.join(executable_name);
            if full_path.is_file() {
                return true;
            }
        }
    }
    false
}

/// Returns the default shell name for the current OS.
fn get_default_shell_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "cmd"
    } else {
        // --- NOTE: This could be improved by reading the SHELL env var, but `bash` is a safe default.
        "bash"
    }
}
