// EN: src/cli/handlers/info.rs

use crate::{
    cli::handlers::commons,
    constants::{AXES_DIR, PROJECT_CONFIG_FILENAME},
    // FIX: Removed `CacheableValue` and added `GlobalIndex` to the import list.
    models::{CommandAction, GlobalIndex, ResolvedConfig, TemplateComponent},
};
use anyhow::Result;
use clap::Parser;
use colored::*;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct InfoArgs {
    // Example future flag:
    // #[arg(long)]
    // raw: bool,
}

/// The main handler for the `info` command.
/// Displays detailed information about the resolved project configuration.
// FIX: The signature now correctly uses the passed `index` mutable reference.
pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. Parse this handler's specific arguments.
    let _info_args = InfoArgs::try_parse_from(&args)?;

    // 2. Resolve configuration using the new central helper function.
    //    `context` is passed directly. If it's `None`, the helper defaults to `.`
    let config = commons::resolve_config_for_context(context, index)?;

    // 3. Print all sections using the resolved config.
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
    // Note: This path might be misleading if the project has no axes.toml,
    // but it's consistent behavior. We can refine this later if needed.
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
        // `get` is guaranteed to return Some within this loop.
        let task = config.scripts.get(cmd_name).unwrap();
        
        print!("    - {}", cmd_name.cyan());

        if let Some(d) = &task.desc {
            if !d.trim().is_empty() {
                print!(": {}", d.dimmed());
            }
        }
        println!();
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
            if let Some(task) = config.vars.get(k) {
                // Render the task's AST back into a representative string.
                let display_val = task
                    .commands
                    .iter()
                    .map(|cmd| {
                        let template = match &cmd.action {
                            CommandAction::Execute(t) | CommandAction::Print(t) => t,
                        };
                        template
                            .iter()
                            .map(|c| match c {
                                TemplateComponent::Literal(s) => s.clone(),
                                TemplateComponent::Parameter(p) => p.original_token.clone(),
                                TemplateComponent::GenericParams => "<axes::params>".to_string(),
                                TemplateComponent::Run(spec) => match spec {
                                    crate::models::RunSpec::Literal(cmd) => format!("<axes::run('{}')>", cmd),
                                },
                                TemplateComponent::Path => "<axes::path>".to_string(),
                                TemplateComponent::Name => "<axes::name>".to_string(),
                                TemplateComponent::Uuid => "<axes::uuid>".to_string(),
                                TemplateComponent::Version => "<axes::version>".to_string(),
                            })
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join(" && ");
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