// src/cli/handlers/info.rs

use crate::{
    cli::handlers::commons,
    constants::{AXES_DIR, PROJECT_CONFIG_FILENAME},
    models::{CacheableValue, ResolvedConfig, TemplateComponent},
};
use anyhow::Result;
use clap::Parser;
use colored::*;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct InfoArgs {
    /// The project context to display information for.
    context: String,
}

/// The main handler for the `info` command.
/// Displays detailed information about the resolved project configuration.
pub fn handle(args: Vec<String>) -> Result<()> {
    // 1. Parse arguments.
    let info_args = InfoArgs::try_parse_from(&args)?;

    // 2. Load index and resolve configuration.
    let index = crate::core::index_manager::load_and_ensure_global_project()?;
    let config = commons::resolve_config_from_context_or_session(Some(info_args.context), &index)?;

    // 3. Print all sections.
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

/// Prints the list of available scripts, including their descriptions.
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

            let description = match cacheable_value {
                CacheableValue::Raw(fc) => fc.desc.as_deref(),
                CacheableValue::Expanded(task) => task.desc.as_deref(),
            };

            if let Some(d) = description
                && !d.trim().is_empty()
            {
                print!(": {}", d.dimmed());
            }
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
                let display_val = match val {
                    CacheableValue::Raw(fc) => {
                        // For a var, `command_lines` will have a single entry.
                        fc.command_lines.join(" && ")
                    }
                    CacheableValue::Expanded(task) => {
                        // This case is unlikely for a var, but we handle it.
                        // We show a flattened representation.
                        task.commands
                            .iter()
                            .map(|cmd| {
                                cmd.template
                                    .iter()
                                    .map(|c| match c {
                                        TemplateComponent::Literal(s) => s.clone(),
                                        TemplateComponent::Parameter(p) => p.original_token.clone(),
                                        TemplateComponent::GenericParams => {
                                            "<axes::params>".to_string()
                                        }
                                    })
                                    .collect::<String>()
                            })
                            .collect::<Vec<_>>()
                            .join(" && ")
                    }
                };
                println!(
                    "    - {} = {}",
                    k.cyan(),
                    format_args!("\"{}\"", display_val)
                );
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
                println!("    - {} = {}", k.cyan(), format_args!("\"{}\"", val));
            }
        }
    }
}
