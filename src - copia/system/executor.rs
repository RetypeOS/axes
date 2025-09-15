// src/system/executor.rs

use dunce;
use std::collections::HashMap;
use std::io::ErrorKind; // Necesario para la detección de errores
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
}

/// Ejecuta un comando de sistema de forma robusta y predecible.
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

    // --- El Enfoque Unificado ---

    // 1. Intentar la ejecución directa
    let mut command = StdCommand::new(program);
    command
        .args(args)
        .current_dir(clean_cwd)
        .envs(env_vars)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    match command.status() {
        Ok(status) => {
            // El programa se encontró y se ejecutó.
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
            // El programa no se pudo iniciar.
            if e.kind() == ErrorKind::NotFound && cfg!(target_os = "windows") {
                // 2. FALLBACK: Si no se encontró y estamos en Windows, podría ser un `builtin`.
                log::debug!(
                    "Command '{}' not found. Retrying with cmd /C.",
                    program
                );

                let mut fallback_command = StdCommand::new("cmd");
                fallback_command
                    .arg("/C")
                    .arg(command_line) // Pasamos la línea completa para que `cmd` la parsee.
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
                // Si el error es otro (ej. permisos) o no estamos en Windows, es un error real.
                return Err(ExecutionError::CommandFailed(command_line.to_string(), e));
            }
        }
    }

    Ok(())
}

// La función `is_windows_shell_builtin` ya no es necesaria y ha sido eliminada.
