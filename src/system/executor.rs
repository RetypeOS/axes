// src/system/executor.rs

use dunce;
use std::collections::HashMap;
use std::io::ErrorKind; // Required for error detection
use std::path::Path;
use std::process::{Command as StdCommand, Stdio};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("El comando no pudo ser parseado: {0}")]
    CommandParse(String),
    #[error("No command specified to run.")]
    EmptyCommand,
    #[error("El comando '{0}' no se pudo ejecutar: {1}")]
    CommandFailed(String, std::io::Error),
    #[error("Command '{0}' exited with a non-zero error code.")]
    NonZeroExitStatus(String),
    #[error("Command '{command}' produced output that was not valid UTF-8")]
    InvalidUtf8Output {
        command: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
}

/// Executes a system command robustly and predictably.
pub fn execute_command(
    command_line: &str,
    cwd: &Path,
    env_vars: &HashMap<String, String>,
) -> Result<(), ExecutionError> {
    let trimmed_command = command_line.trim();
    if trimmed_command.is_empty() {
        return Err(ExecutionError::EmptyCommand);
    }

    let (final_command_line, ignore_errors) = if trimmed_command.starts_with('-') {
        (trimmed_command.strip_prefix('-').unwrap().trim(), true)
    } else {
        (trimmed_command, false)
    };

    log::info!(
        "Ejecutando comando: '{}' en {:?}",
        final_command_line,
        dunce::simplified(cwd).display()
    );

    let parts = shlex::split(final_command_line)
        .ok_or_else(|| ExecutionError::CommandParse(final_command_line.to_string()))?;

    if parts.is_empty() {
        return Err(ExecutionError::EmptyCommand);
    }

    let program = &parts[0];
    let args = &parts[1..];
    let clean_cwd = dunce::simplified(cwd);

    // --- The Unified Approach ---

    // 1. Attempt direct execution
    let mut command = StdCommand::new(program);
    command
        .args(args)
        .current_dir(clean_cwd)
        .envs(env_vars)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    match command.status() {
        Ok(status) => {
            // The program was found and executed.
            if !status.success() {
                if !ignore_errors {
                    return Err(ExecutionError::NonZeroExitStatus(command_line.to_string()));
                } else {
                    log::warn!(
                        "Command finished with a non-zero error code, but was ignored as requested."
                    );
                }
            }
        }
        Err(e) => {
            // The program could not be started.
            if e.kind() == ErrorKind::NotFound && cfg!(target_os = "windows") {
                // 2. FALLBACK: If not found and on Windows, it might be a `builtin`.
                log::debug!("Command '{}' not found. Retrying with cmd /C.", program);

                let mut fallback_command = StdCommand::new("cmd");
                fallback_command
                    .arg("/C")
                    .arg(command_line) // We pass the full line for `cmd` to parse.
                    .current_dir(dunce::simplified(cwd))
                    .envs(env_vars)
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit());

                let fallback_status = fallback_command
                    .status()
                    .map_err(|e| ExecutionError::CommandFailed(command_line.to_string(), e))?;

                if !fallback_status.success() {
                    return Err(ExecutionError::NonZeroExitStatus(command_line.to_string()));
                }
            } else {
                // If the error is different (e.g., permissions) or we are not on Windows, it's a real error.
                return Err(ExecutionError::CommandFailed(command_line.to_string(), e));
            }
        }
    }

    Ok(())
}

/// Executes a command and captures its standard output.
/// Stderr is passed through to the user's terminal.
pub fn execute_and_capture_output(
    command_line: &str,
    cwd: &Path,
    env_vars: &HashMap<String, String>,
) -> Result<String, ExecutionError> {
    let trimmed_command = command_line.trim();
    if trimmed_command.is_empty() {
        return Err(ExecutionError::EmptyCommand);
    }

    let parts = shlex::split(trimmed_command)
        .ok_or_else(|| ExecutionError::CommandParse(trimmed_command.to_string()))?;
    if parts.is_empty() {
        return Err(ExecutionError::EmptyCommand);
    }

    let program = &parts[0];
    let args = &parts[1..];
    let clean_cwd = dunce::simplified(cwd);

    let command_output = StdCommand::new(program)
        .args(args)
        .current_dir(clean_cwd)
        .envs(env_vars)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| ExecutionError::CommandFailed(trimmed_command.to_string(), e))?;

    if !command_output.status.success() {
        return Err(ExecutionError::NonZeroExitStatus(
            trimmed_command.to_string(),
        ));
    }

    String::from_utf8(command_output.stdout).map_err(|e| ExecutionError::InvalidUtf8Output {
        command: trimmed_command.to_string(),
        source: e,
    })
}
