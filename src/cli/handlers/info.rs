// EN: src/cli/handlers/info.rs

use anyhow::{Result, anyhow};
use colored::*;

use crate::{
    constants::{AXES_DIR, PROJECT_CONFIG_FILENAME}, core::index_manager, models::{CacheableValue, Command as ProjectCommand, ResolvedConfig}, CancellationToken
};

use clap::Parser;

use super::commons;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct InfoArgs {
    /// The project context to display information for.
    context: Option<String>,
}

/// The main handler for the `info` command.
/// Displays detailed information about the resolved project configuration.
pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    // 1. Parsear los argumentos específicos de `info`.
    let info_args = InfoArgs::try_parse_from(&args)?;
    if args.len() > 1 {
        return Err(anyhow!(t!("info.error.unexpected_args")));
    }
    let index = index_manager::load_and_ensure_global_project()?;

    // 2. Resolver la configuración usando el `context` parseado.
    let config =
        commons::resolve_config_from_context_or_session(info_args.context, &index, cancellation_token)?;

    // 3. El resto de la lógica de impresión no cambia.
    print_metadata(&config);
    print_scripts(&config);
    print_variables(&config, "vars", t!("info.label.vars"));
    print_variables(&config, "env", t!("info.label.env"));

    println!("\n---------------------------------");
    Ok(())
}

/// Prints the core metadata of the project.
fn print_metadata(config: &ResolvedConfig) {
    let config_file_path = config
        .project_root
        .join(AXES_DIR)
        .join(PROJECT_CONFIG_FILENAME);

    println!(
        "\n--- {} '{}' ---",
        t!("info.header"),
        config.qualified_name.yellow()
    );

    println!("  {:<15} {}", t!("info.label.uuid").blue(), config.uuid);
    println!(
        "  {:<15} {}",
        t!("info.label.root_path").blue(),
        config.project_root.display()
    );
    println!(
        "  {:<15} {}",
        t!("info.label.config_file").blue(),
        config_file_path.display()
    );

    if let Some(v) = &config.version {
        println!("  {:<15} {}", t!("info.label.version").blue(), v);
    }
    if let Some(d) = &config.description {
        println!("  {:<15} {}", t!("info.label.description").blue(), d);
    }
}

/// Prints the list of available scripts, including their descriptions if available.
fn print_scripts(config: &ResolvedConfig) {
    if config.scripts.is_empty() {
        println!("\n  {}", t!("info.label.no_scripts").dimmed());
        return;
    }

    println!("\n  {}:", t!("info.label.available_scripts").blue());
    let mut cmd_names: Vec<_> = config.scripts.keys().collect();
    cmd_names.sort();

    for cmd_name in cmd_names {
        if let Some(cacheable_value) = config.scripts.get(cmd_name) {
            print!("    - {}", cmd_name.cyan());

            // Extraemos la descripción del CacheableValue.
            let description = match cacheable_value {
                CacheableValue::Raw { desc, .. } => desc.as_deref(),
                CacheableValue::Expanded(task) => task.desc.as_deref(),
            };
            
            if let Some(d) = description {
                // Si la descripción no está vacía, la mostramos.
                if !d.trim().is_empty() {
                    print!(": {}", d.dimmed());
                }
            }
            
            // Opcional: Podríamos añadir un indicador visual del tipo de script,
            // pero por ahora, la descripción es lo más importante.
            // Ejemplo:
            // match cacheable_value {
            //     CacheableValue::Raw { value, .. } if value.contains("&&") => print!(" {}", "(sequence)".dimmed()),
            //     _ => {}
            // }

            println!();
        }
    }
}

/// A generic function to print key-value maps like [vars] and [env].
fn print_variables(config: &ResolvedConfig, key: &str, title: &str) {
    if key == "vars" {
        if config.vars.is_empty() {
            return;
        }
        println!("\n  {}:", title.blue());
        let mut sorted_keys: Vec<_> = config.vars.keys().collect();
        sorted_keys.sort();
        for k in sorted_keys {
            if let Some(val) = config.vars.get(k) {
                // Mostramos una representación del valor cacheable
                let display_val = match val {
                    CacheableValue::Raw { command, .. } => {
                        // Un 'var' es un Simple(string)
                        if let ProjectCommand::Simple(s) = command {
                            format!("\"{}\" (raw)", s)
                        } else {
                            "[complex raw value]".to_string()
                        }
                    },
                    CacheableValue::Expanded { .. } => "[expanded]".to_string(),
                };
                println!("    - {} = {}", k.cyan(), display_val);
            }
        }
    } else if key == "env" {
        if config.env.is_empty() {
            return;
        }
        println!("\n  {}:", title.blue());
        let mut sorted_keys: Vec<_> = config.env.keys().collect();
        sorted_keys.sort();
        for k in sorted_keys {
            if let Some(val) = config.env.get(k) {
                println!("    - {} = {}", k.cyan(), format!("\"{}\"", val));
            }
        }
    }
}
