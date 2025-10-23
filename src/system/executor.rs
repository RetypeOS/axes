//! # System Command Execution
//!
//! This module provides a robust, high-performance, and platform-aware abstraction for
//! executing external system commands. It is built on top of `tokio::process` to handle
//! asynchronous operations efficiently, while providing a simple synchronous API to the
//! rest of the application.
//!
//! ## Key Features
//!
//! - **Async Backend, Sync API**: Uses a global `tokio` runtime to manage child processes
//!   without blocking the main `axes` thread, which is crucial for features like parallel
//!   execution.
//! - **Graceful Cancellation**: `execute_command` properly handles `Ctrl+C` signals, ensuring
//!   that child processes are terminated correctly.
//! - **Platform Compatibility**: Includes a fallback for Windows to execute shell built-ins
//!   (like `echo` or `dir`) via `cmd.exe /C`, increasing compatibility.
//! - **Output Capturing**: Provides `execute_and_capture_output` for use in substitutions like
//!   `<run(...)>`, allowing dynamic command generation.
//! - **Robust Error Handling**: The `ExecutionError` enum provides clear, contextual information
//!   about why a command failed.

use dunce;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Stdio;
use thiserror::Error;
use tokio::process::Command as TokioCommand;
use tokio::runtime::Runtime;

lazy_static! {
    /// A global multi-threaded Tokio runtime for executing commands.
    /// We use a multi-threaded runtime to allow for true parallelism
    /// if `axes` ever needs to spawn multiple blocking tasks concurrently.
    static ref TOKIO_RT: Runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime for command execution");
}

/// Represents errors that can occur during the execution of an external command.
#[derive(Error, Debug)]
pub enum ExecutionError {
    /// The command string could not be parsed into a program and arguments (e.g., due to mismatched quotes).
    #[error("Command could not be parsed: {0}")]
    CommandParse(String),
    /// The command failed to spawn, often because the executable was not found in the system's `PATH`.
    #[error("Command '{0}' could not be executed: {1}")]
    CommandFailed(String, std::io::Error),
    /// The command executed successfully but returned a non-zero exit code, indicating failure.
    #[error("Command '{0}' exited with a non-zero error code.")]
    NonZeroExitStatus(String),
    /// The command was terminated prematurely by a signal, typically `Ctrl+C`.
    #[error("Command '{command}' was interrupted by a signal (e.g., Ctrl+C).")]
    Interrupted {
        /// The command that was interrupted.
        command: String,
    },
    /// The command produced standard output that was not valid UTF-8, preventing it from being captured as a String.
    #[error("Command '{command}' produced output that was not valid UTF-8")]
    InvalidUtf8Output {
        /// The command that produced invalid output.
        command: String,
        /// The underlying UTF-8 conversion error.
        #[source]
        source: std::string::FromUtf8Error,
    },
}

/// Executes a system command, inheriting standard input, output, and error streams.
///
/// This function is the primary way to run interactive or long-running commands. It blocks
/// until the command completes but uses an asynchronous runtime internally to handle I/O
/// and process signals gracefully. It includes a `tokio::select!` block to race the
/// child process against a `Ctrl+C` signal, ensuring proper cleanup.
///
/// # Arguments
/// * `command` - The full command string to execute (e.g., `"npm install --save-dev"`).
/// * `ignore_errors` - If `true`, a non-zero exit code will not be considered an error.
/// * `cwd` - The working directory in which to execute the command.
/// * `env_vars` - A map of environment variables to set for the child process.
pub fn execute_command(
    command: &str,
    ignore_errors: bool,
    cwd: &Path,
    env_vars: &HashMap<String, String>, // Currently unused, but kept for API consistency.
) -> Result<(), ExecutionError> {
    TOKIO_RT.block_on(async {
        let trimmed_command = command.trim();
        if trimmed_command.is_empty() {
            return Ok(());
        }

        if trimmed_command.is_empty() {
            return Ok(());
        }

        let parts = shlex::split(trimmed_command)
            .ok_or_else(|| ExecutionError::CommandParse(trimmed_command.to_string()))?;

        if parts.is_empty() {
            return Ok(());
        }

        let program = &parts[0];
        let args = &parts[1..];
        let clean_cwd = dunce::simplified(cwd);

        // We need to create the command inside the async block.
        let mut command = TokioCommand::new(program);
        command
            .args(args)
            .current_dir(clean_cwd)
            .envs(env_vars)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) if e.kind() == ErrorKind::NotFound && cfg!(target_os = "windows") => {
                TokioCommand::new("cmd")
                    .arg("/C")
                    .arg(trimmed_command)
                    .current_dir(clean_cwd)
                    .envs(env_vars)
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .map_err(|e| ExecutionError::CommandFailed(trimmed_command.to_string(), e))?
            }
            Err(e) => {
                return Err(ExecutionError::CommandFailed(
                    trimmed_command.to_string(),
                    e,
                ));
            }
        };

        // Asynchronously wait for either the child to exit or for a Ctrl+C signal.
        tokio::select! {
            // Biased select to prefer checking for cancellation first.
            biased;

            _ = tokio::signal::ctrl_c() => {
                log::debug!("Ctrl+C signal received. Attempting to kill child process...");
                if let Some(pid) = child.id() {
                    log::debug!("Child PID is {}", pid);
                }
                // Attempt to gracefully kill the process.
                child.kill().await.map_err(|e| {
                    log::warn!("Failed to kill child process: {}", e);
                    ExecutionError::CommandFailed(trimmed_command.to_string(), e)
                })?;
                log::debug!("Child process killed.");
                Err(ExecutionError::Interrupted { command: trimmed_command.to_string() })
            }

            status_result = child.wait() => {
                match status_result {
                    Ok(status) if !status.success() && !ignore_errors => {
                        Err(ExecutionError::NonZeroExitStatus(trimmed_command.to_string()))
                    }
                    Ok(_) => Ok(()), // Success
                    Err(e) => Err(ExecutionError::CommandFailed(trimmed_command.to_string(), e)),
                }
            }
        }
    })
}

/// Executes a command and captures its standard output as a `String`.
///
/// This function is intended for short-lived commands used for dynamic substitutions, such as
/// in `<run('git rev-parse HEAD')>`. Standard error is passed through to the user's terminal,
/// but standard output is captured. This function does *not* handle `Ctrl+C`, as it's
/// expected to complete quickly.
///
/// # Arguments
/// * `command_line` - The full command string to execute.
/// * `cwd` - The working directory for the command.
/// * `env_vars` - A map of environment variables to set for the child process.
///
/// # Returns
/// A `Result` containing the captured standard output, trimmed of whitespace, on success.
pub fn execute_and_capture_output(
    command_line: &str,
    cwd: &Path,
    env_vars: &HashMap<String, String>,
) -> Result<String, ExecutionError> {
    TOKIO_RT.block_on(async {
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

        let mut command = TokioCommand::new(program);
        command
            .args(args)
            .current_dir(clean_cwd)
            .envs(env_vars)
            .stdin(Stdio::null()) // Don't inherit stdin
            .stdout(Stdio::piped()) // Capture stdout
            .stderr(Stdio::inherit()); // Pass through stderr

        let output = match command.output().await {
            Ok(out) => out,
            Err(e) if e.kind() == ErrorKind::NotFound && cfg!(target_os = "windows") => {
                TokioCommand::new("cmd")
                    .arg("/C")
                    .arg(trimmed_command)
                    .current_dir(clean_cwd)
                    .envs(env_vars)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .output()
                    .await
                    .map_err(|e| ExecutionError::CommandFailed(trimmed_command.to_string(), e))?
            }
            Err(e) => {
                return Err(ExecutionError::CommandFailed(
                    trimmed_command.to_string(),
                    e,
                ));
            }
        };

        if !output.status.success() {
            return Err(ExecutionError::NonZeroExitStatus(
                trimmed_command.to_string(),
            ));
        }

        String::from_utf8(output.stdout).map_err(|e| ExecutionError::InvalidUtf8Output {
            command: trimmed_command.to_string(),
            source: e,
        })
    })
}
