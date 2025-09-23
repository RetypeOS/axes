// EN: src/cli/handlers/info.rs

use anyhow::{Result, anyhow};
use colored::*;

use crate::{
    CancellationToken,
    constants::{AXES_DIR, PROJECT_CONFIG_FILENAME},
    models::{Command as ProjectCommand, ResolvedConfig},
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

    // 2. Resolver la configuración usando el `context` parseado.
    let config =
        commons::resolve_config_from_context_or_session(info_args.context, cancellation_token)?;

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

/// Prints the list of available scripts (scripts).
fn print_scripts(config: &ResolvedConfig) {
    if config.scripts.is_empty() {
        println!("\n  {}", t!("info.label.no_scripts").dimmed());
        return;
    }

    println!("\n  {}:", t!("info.label.available_scripts").blue());
    let mut cmd_names: Vec<_> = config.scripts.keys().collect();
    cmd_names.sort();

    for cmd_name in cmd_names {
        if let Some(command_def) = config.scripts.get(cmd_name) {
            print!("    - {}", cmd_name.cyan());

            match command_def {
                ProjectCommand::Extended(ext) => {
                    if let Some(d) = &ext.desc {
                        print!(": {}", d.dimmed());
                    }
                }
                ProjectCommand::Platform(pc) => {
                    let type_info = format!("({})", t!("info.script_type.platform"));
                    if let Some(d) = &pc.desc {
                        print!(": {} {}", d.dimmed(), type_info.dimmed());
                    } else {
                        print!(" {}", type_info.dimmed());
                    }
                }
                ProjectCommand::Sequence(_) => {
                    print!(" ({})", t!("info.script_type.sequence").dimmed());
                }
                ProjectCommand::Simple(_) => { /* No extra info */ }
            }
            println!();
        }
    }
}

/// A generic function to print key-value maps like [vars] and [env].
fn print_variables(config: &ResolvedConfig, key: &str, title: &str) {
    let map = match key {
        "vars" => &config.vars,
        "env" => &config.env,
        _ => return,
    };

    if map.is_empty() {
        return;
    }

    println!("\n  {}:", title.blue());
    let mut sorted_keys: Vec<_> = map.keys().collect();
    sorted_keys.sort();

    for k in sorted_keys {
        if let Some(val) = map.get(k) {
            println!("    - {} = {}", k.cyan(), format_args!("\"{}\"", val));
        }
    }
}
