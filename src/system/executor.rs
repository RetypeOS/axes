// EN: src/system/executor.rs

use crate::{CancellationToken, cli::handlers::commons};
use dunce;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Command could not be parsed: {0}")]
    CommandParse(String),
    #[error("No command specified to run.")]
    EmptyCommand,
    #[error("Command '{0}' could not be executed: {1}")]
    CommandFailed(String, std::io::Error),
    #[error("Command '{0}' exited with a non-zero error code.")]
    NonZeroExitStatus(String),
    #[error("Command '{command}' produced output that was not valid UTF-8")]
    InvalidUtf8Output {
        command: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("Operation was cancelled by the user.")]
    Cancelled,
}

/// Executes a system command robustly and predictably, with support for graceful cancellation.
/// This function will not return until the command has finished, but it can be
/// interrupted by the CancellationToken.
pub fn execute_command(
    command_line: &str,
    cwd: &Path,
    env_vars: &HashMap<String, String>,
    cancellation_token: &CancellationToken,
) -> Result<(), ExecutionError> {
    let trimmed_command = command_line.trim();
    if trimmed_command.is_empty() {
        return Ok(()); // An empty command is a success, not an error.
    }

    let (final_command_line, ignore_errors) = if trimmed_command.starts_with('-') {
        (trimmed_command.strip_prefix('-').unwrap().trim(), true)
    } else {
        (trimmed_command, false)
    };

    if final_command_line.is_empty() {
        return Ok(());
    }

    let parts = shlex::split(final_command_line)
        .ok_or_else(|| ExecutionError::CommandParse(final_command_line.to_string()))?;
    if parts.is_empty() {
        return Ok(());
    }

    let program = &parts[0];
    let args = &parts[1..];
    let clean_cwd = dunce::simplified(cwd);

    let mut command = StdCommand::new(program);
    command
        .args(args)
        .current_dir(clean_cwd)
        .envs(env_vars)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // Fallback logic for Windows built-in commands like `echo`.
    // We try to spawn directly first. If it fails with `NotFound`, we try with `cmd /C`.
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) if e.kind() == ErrorKind::NotFound && cfg!(target_os = "windows") => {
            log::debug!("Command '{}' not found. Retrying with cmd /C.", program);
            StdCommand::new("cmd")
                .arg("/C")
                .arg(final_command_line) // Pass the full, unparsed line to cmd
                .current_dir(clean_cwd)
                .envs(env_vars)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| ExecutionError::CommandFailed(final_command_line.to_string(), e))?
        }
        Err(e) => {
            return Err(ExecutionError::CommandFailed(
                final_command_line.to_string(),
                e,
            ));
        }
    };

    // Non-blocking wait loop to allow for cancellation.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process has finished.
                if !status.success() && !ignore_errors {
                    return Err(ExecutionError::NonZeroExitStatus(
                        final_command_line.to_string(),
                    ));
                }
                return Ok(());
            }
            Ok(None) => {
                // Process is still running. Check for cancellation signal.
                if commons::check_for_cancellation(cancellation_token).is_err() {
                    log::debug!(
                        "Cancellation requested, killing child process (PID: {})...",
                        child.id()
                    );
                    if let Err(e) = child.kill() {
                        log::warn!("Failed to kill child process {}: {}", child.id(), e);
                    }
                    // Wait briefly for the process to die after being killed.
                    child.wait().ok();
                    return Err(ExecutionError::Cancelled);
                }
                // Wait briefly to avoid a tight loop consuming CPU.
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                // Error while trying to get the process status.
                return Err(ExecutionError::CommandFailed(
                    final_command_line.to_string(),
                    e,
                ));
            }
        }
    }
}

/// Executes a command and captures its standard output.
/// Stderr is passed through to the user's terminal.
/// NOTE: This operation is blocking and only checks for cancellation *before* starting.
/// It is intended for short-running commands used for text substitution.
pub fn execute_and_capture_output(
    command_line: &str,
    cwd: &Path,
    env_vars: &HashMap<String, String>,
    cancellation_token: &CancellationToken,
) -> Result<String, ExecutionError> {
    // Pre-flight cancellation check.
    if commons::check_for_cancellation(cancellation_token).is_err() {
        return Err(ExecutionError::Cancelled);
    }

    let trimmed_command = command_line.trim();
    if trimmed_command.is_empty() {
        return Ok(String::new());
    }

    let parts = shlex::split(trimmed_command)
        .ok_or_else(|| ExecutionError::CommandParse(trimmed_command.to_string()))?;
    if parts.is_empty() {
        return Ok(String::new());
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
