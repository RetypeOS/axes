use crate::{
    core::{parameters::ArgResolver, task_executor},
    models::{CommandAction, GlobalIndex, ResolvedConfig, Task},
    system::shells_config,
};
use anyhow::Result;
use colored::Colorize;
use std::fmt::Write;
use std::{fs, process::Command, sync::Arc};
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
    let shells_config = shells_config::load_shells_config()?;
    let options = config.get_options()?;
    let shell_name: String = options
        .shell
        .clone()
        .unwrap_or_else(|| shells_config::get_default_shell_name().to_string());
    let shell_config = shells_config
        .shells
        .get(&shell_name)
        .ok_or_else(|| ShellError::ShellNotDefined(shell_name.clone()))?;

    // 2. If an `at_start` task exists, render its commands into a script.
    // Esta lógica es especial y no usa `execute_task`, por lo que se mantiene.
    let at_start_final_commands = if let Some(task) = &task_start {
        println!(
            "\n{}",
            format!(t!("start.info.preparing_hook"), hook = "at_start").dimmed()
        );

        let mut commands = Vec::new();
        for plat_exec in &task.commands {
            if let Some(cmd_exec) = config.select_platform_exec(plat_exec) {
                let rendered_command = match &cmd_exec.action {
                    CommandAction::Execute(template) => {
                        task_executor::assemble_final_command(template, config, resolver, index, 0)?
                    }
                    CommandAction::Print(template) => {
                        let text = task_executor::assemble_final_command(
                            template, config, resolver, index, 0,
                        )?;
                        format!("echo \"{}\"", text.replace('"', "\\\""))
                    }
                };
                commands.push(rendered_command);
            }
        }
        commands
    } else {
        Vec::new()
    };

    let rendered_prompt = if let Some(prompt_template) = &options.prompt {
        // We can create a temporary template to pass to our powerful assembler.
        let template = crate::core::compiler::tokenize_string(prompt_template)?;
        Some(task_executor::assemble_final_command(
            &template, config, resolver, index, 0,
        )?)
    } else {
        None
    };

    // 3. Create the temporary initialization script using a scope guard for robust cleanup.
    let is_windows_shell = shell_name == "cmd" || shell_name == "powershell";
    let script_extension = if is_windows_shell { "bat" } else { "sh" };

    // Create a named temp file that will persist.
    let temp_script_file = NamedTempFile::with_prefix("axes-init-")?;
    let temp_script_path = temp_script_file.path().with_extension(script_extension);
    let script_content = build_init_script(
        config,
        &at_start_final_commands,
        &shell_name,
        rendered_prompt.as_deref(),
    )?;
    fs::write(&temp_script_path, &script_content)?;

    // This guard ensures the temp file is deleted when `launch_session` returns,
    // even in case of an error or panic in the code below.
    let _guard = scopeguard::guard((), |_| {
        let _ = fs::remove_file(&temp_script_path);
        log::trace!(
            "Cleaned up temporary script file: {}",
            temp_script_path.display()
        );
    });

    //// 3. Create the temporary initialization script.
    //let is_windows_shell = shell_name == "cmd" || shell_name == "powershell";
    //let script_extension = if is_windows_shell { "bat" } else { "sh" };
    //let temp_script_file = NamedTempFile::with_prefix("axes-init-")?.into_temp_path();
    //let temp_script_path = temp_script_file.with_extension(script_extension);
    //
    //let script_content = build_init_script(config, &at_start_final_commands, is_windows_shell)?;
    //fs::write(&temp_script_path, &script_content)?;

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
    cmd.envs(env_vars.as_ref());

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
    if let Some(task_universal) = &task_exit {
        println!(
            "\n{}",
            format!(t!("start.info.executing_hook"), hook = "at_exit").dimmed()
        );

        // Specialize the `at_exit` task for the current platform before execution.
        let task_specialized = config.specialize_task_for_platform(task_universal);

        // Pass the optimized, specialized task to the executor.
        task_executor::execute_task(&task_specialized, config, resolver, index)?;
    }

    Ok(())
}

/// Helper function to escape a string for safe use within a `cmd.exe` `set "KEY=VALUE"` command.
/// It handles special characters that could otherwise terminate the command or be interpreted by the shell.
fn escape_for_cmd_set(value: &str) -> String {
    value
        .replace('%', "%%")
        .replace('^', "^^")
        .replace('&', "^&")
        .replace('<', "^<")
        .replace('>', "^>")
        .replace('|', "^|")
}
fn build_init_script(
    config: &ResolvedConfig,
    at_start_commands: &[String],
    shell_name: &str,
    prompt: Option<&str>,
) -> Result<String> {
    // Determine the shell family for easier logic branching.
    let is_cmd = shell_name == "cmd";
    let is_posix = shell_name == "bash" || shell_name == "zsh";

    let mut script = String::with_capacity(256 + at_start_commands.len() * 128);

    if is_cmd {
        writeln!(script, "@echo off")?;
    }

    for (key, value) in &*config.get_env()? {
        if is_cmd {
            let escaped_value = escape_for_cmd_set(value);
            writeln!(script, "set \"{}={}\"", key, escaped_value)?;
        } else {
            // Assume POSIX-like escaping for PowerShell, Bash, Zsh etc.
            let escaped_value = value.replace('\'', "'\\''");
            writeln!(script, "export {}='{}'", key, escaped_value)?;
        }
    }
    writeln!(script)?;

    for command in at_start_commands {
        if !command.trim().is_empty() {
            writeln!(script, "{}", command)?;
        }
    }

    if let Some(prompt_str) = prompt {
        match shell_name {
            "bash" | "zsh" => {
                let escaped = prompt_str
                    .replace('\\', "\\\\")
                    .replace('$', "\\$")
                    .replace('`', "\\`");
                writeln!(script, "\nexport PS1='{}'", escaped)?;
            }
            "cmd" => {
                let escaped = prompt_str.replace('$', "$$");
                writeln!(script, "\nPROMPT {}", escaped)?;
            }
            "powershell" => {
                let escaped = prompt_str.replace('\'', "''");
                writeln!(
                    script,
                    "\nfunction prompt {{ Write-Host -NoNewline '{}'; return ' ' }}",
                    escaped
                )?;
            }
            _ => {
                log::warn!(
                    "Prompt customization is not supported for shell '{}'.",
                    shell_name
                );
            }
        }
    }

    let exit_message = "--- Type 'exit' to leave. ---";
    if is_cmd {
        writeln!(script, "\necho.\necho {}\necho.", exit_message)?;
    } else if is_posix {
        // POSIX shells can use echo with single quotes.
        writeln!(script, "\necho ''\necho '{}'\necho ''", exit_message)?;
    } else {
        // PowerShell handles echo differently, but simple echo works.
        writeln!(script, "\necho ''\necho '{}'\necho ''", exit_message)?;
    }
    Ok(script)
}
