use std::{collections::HashMap, env, fs, path::PathBuf};

use crate::{
    models::{ShellConfig, ShellsConfig},
    system::shell::ShellError,
};

pub fn load_shells_config() -> Result<ShellsConfig, ShellError> {
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

pub fn get_default_shell_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "bash"
    }
}
