// src/system/shell.rs

use crate::models::{ResolvedConfig, ShellConfig, ShellsConfig};
use crate::system::executor;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{env, fs};
use tempfile::NamedTempFile;
use thiserror::Error;

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
}

/// Lanza una sub-shell interactiva para un proyecto.
pub fn launch_interactive_shell(config: &ResolvedConfig) -> Result<(), ShellError> {
    let shells_config = load_shells_config()?;

    // 1. Determinar qué shell usar
    let shell_name = match &config.options.shell {
        Some(shell_from_config) => shell_from_config.clone(), // Usar el valor del config
        None => get_default_shell_name() // Si no hay nada, usar el default del sistema
            .ok_or(ShellError::NoDefaultShell)?
            .to_string(),
    };

    let shell_config = shells_config
        .shells
        .get(&shell_name)
        .ok_or_else(|| ShellError::ShellNotDefined(shell_name.clone()))?;

    // 2. Crear el script de inicialización temporal
    let is_windows_shell = shell_name == "cmd" || shell_name == "powershell";
    let script_extension = if is_windows_shell { ".bat" } else { ".sh" };
    let temp_script_file = NamedTempFile::with_prefix("axes-init-")?
        .into_temp_path()
        .with_extension(script_extension);

    let script_content = build_init_script(config, is_windows_shell);

    fs::write(&temp_script_file, script_content)?;

    log::debug!(
        "Temporary initialization script created at: {}",
        temp_script_file.display()
    );

    // 3. Construir y ejecutar el comando
    let mut cmd = Command::new(&shell_config.path);
    cmd.current_dir(&config.project_root);

    // Inyectar variables de sesión de axes
    cmd.env("AXES_PROJECT_ROOT", config.project_root.as_os_str());
    cmd.env("AXES_PROJECT_NAME", &config.qualified_name);
    cmd.env("AXES_PROJECT_UUID", config.uuid.to_string());

    if let Some(args) = &shell_config.interactive_args {
        for arg in args {
            cmd.arg(arg);
        }
        cmd.arg(&temp_script_file);
    }

    // 4. Lanzar la shell y esperar a que termine
    let status = cmd
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if !status.success() {
        log::warn!(
            "Interactive shell finished with error code: {:?}",
            status.code()
        );
    }

    // 5. **NUEVA LÓGICA**: Ejecutar el hook `at_exit`
    if let Some(at_exit_command) = &config.options.at_exit
        && !at_exit_command.trim().is_empty()
    {
        //println!("\nEjecutando hook 'at_exit'...");

        // Usamos nuestro ejecutor de comandos estándar.
        // No pasamos parámetros, pero sí el entorno del proyecto.
        let interpolator = crate::core::interpolator::Interpolator::new(config, &[]);
        let final_command = interpolator.interpolate(at_exit_command);

        if let Err(e) = executor::execute_command(&final_command, &config.project_root, &config.env)
        {
            // Si `at_exit` falla, no queremos que toda la operación de `axes` falle.
            // Es una operación de limpieza, por lo que solo mostramos una advertencia.
            eprintln!(
                "\nWarning: 'at_exit' hook failed to execute: {}",
                e
            );
        }
    }

    // 6. Limpieza del archivo temporal (manejada por `tempfile`)

    Ok(())
}

/// Construye el contenido del script de inicialización.
fn build_init_script(config: &ResolvedConfig, is_windows: bool) -> String {
    let mut script = String::new();

    // Silenciar comandos
    if is_windows {
        script.push_str("@echo off\n");
    } else {
        // Podríamos usar `set +v` o simplemente no poner nada para shells POSIX
    }

    // Añadir variables de [env]
    for (key, value) in &config.env {
        if is_windows {
            script.push_str(&format!("set \"{}={}\"\n", key, value));
        } else {
            script.push_str(&format!("export {}='{}'\n", key, value));
        }
    }

    // Añadir hook at_start
    if let Some(at_start) = &config.options.at_start
        && !at_start.trim().is_empty()
    {
        if is_windows {
            script.push_str(&format!("call {}\n", at_start));
        } else {
            // `source` es más robusto que `.`
            script.push_str(&format!("source \"{}\" || . \"{}\"\n", at_start, at_start));
        }
    }

    // Mensaje de bienvenida
    let welcome_message = format!(
        "--- axes session for '{}' started. Type 'exit' to exit. ---",
        config.qualified_name
    );
    if is_windows {
        script.push_str(&format!("\necho.\necho {}\n", welcome_message));
    } else {
        script.push_str(&format!("\necho ''\necho '{}'\n", welcome_message));
    }

    script
}

/// Carga la configuración de shells desde el disco.
/// Si el archivo no existe, lo genera con valores por defecto y lo guarda.
fn load_shells_config() -> Result<ShellsConfig, ShellError> {
    let config_dir =
        crate::core::paths::get_axes_config_dir().map_err(|_| ShellError::ConfigDirNotFound)?;
    let shells_path = config_dir.join("shells.toml");

    if !shells_path.exists() {
        log::warn!("'shells.toml' not found. Generating default config file.");
        let default_config = generate_default_shells_config();
        let toml_string = toml::to_string_pretty(&default_config)?;
        fs::write(&shells_path, toml_string)?;
        println!(
            "Shells config file created at: {}",
            shells_path.display()
        );
        return Ok(default_config);
    }

    let content = fs::read_to_string(shells_path)?;
    Ok(toml::from_str(&content)?)
}

/// Genera una configuración de shells por defecto, detectando lo que está disponible.
fn generate_default_shells_config() -> ShellsConfig {
    let mut shells = HashMap::new();

    // Siempre añadir `cmd` en Windows.
    if cfg!(target_os = "windows") {
        shells.insert(
            "cmd".to_string(),
            ShellConfig {
                path: PathBuf::from("cmd.exe"),
                interactive_args: Some(vec!["/K".to_string()]),
            },
        );

        // Intentar detectar PowerShell.
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

    // Intentar detectar `bash` en cualquier sistema.
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

    // Se podrían añadir más detectores para zsh, fish, etc. aquí.

    ShellsConfig { shells }
}

/// Comprueba si un ejecutable existe en las rutas del PATH del sistema.
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

/// Devuelve el nombre de la shell por defecto para el SO actual.
fn get_default_shell_name() -> Option<&'static str> {
    if cfg!(target_os = "windows") {
        Some("cmd")
    } else {
        // En sistemas no-Windows, `bash` es una suposición segura, pero
        // se podría mejorar leyendo la variable de entorno SHELL.
        env::var("SHELL")
            .ok()
            .and_then(|s| {
                s.split('/').next_back().map(|name| {
                    if name == "zsh" {
                        "zsh"
                    } else if name == "fish" {
                        "fish"
                    } else {
                        "bash"
                    } // Fallback
                })
            })
            .or(Some("bash"))
    }
}
