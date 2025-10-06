// EN: src/cli/handlers/info.rs (REBUILT FOR LAZY `ResolvedConfig`)

use crate::{
    cli::handlers::commons,
    constants::{AXES_DIR, PROJECT_CONFIG_FILENAME},
    models::{CommandAction, GlobalIndex, ResolvedConfig, RunSpec, TemplateComponent},
};
use anyhow::Result;
use clap::Parser;
use colored::*;
//use std::sync::Arc;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct InfoArgs {}

/// The main handler for the `info` command.
/// Displays detailed information about the lazily resolved project configuration.
pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    let _info_args = InfoArgs::try_parse_from(&args)?;

    // `resolve_config_for_context` now returns our lazy facade.
    let config = commons::resolve_config_for_context(context, index)?;

    // Print all sections, passing the index for lazy resolution.
    print_metadata(&config, index)?;
    print_scripts(&config, index)?;
    print_variables(&config, "vars", t!("info.label.vars"), index)?;
    print_variables(&config, "env", t!("info.label.env"), index)?;

    println!("\n---------------------------------");
    Ok(())
}

/// Prints the core metadata of the project.
fn print_metadata(config: &ResolvedConfig, index: &mut GlobalIndex) -> Result<()> {
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

    // Use accessor methods to get lazily resolved data.
    if let Some(v) = config.get_version(index)? {
        println!("  {:<15} {}", t!("info.label.version").blue(), v);
    }
    if let Some(d) = config.get_description(index)? {
        println!("  {:<15} {}", t!("info.label.description").blue(), d);
    }
    Ok(())
}

/// Prints the list of available scripts, including their descriptions.
fn print_scripts(config: &ResolvedConfig, index: &mut GlobalIndex) -> Result<()> {
    // To get all available scripts, we merge them from all layers.
    let scripts = config.get_all_scripts(index)?;

    if scripts.is_empty() {
        println!("\n  {}", t!("info.label.no_scripts").dimmed());
        return Ok(());
    }

    println!("\n  {}:", t!("info.label.available_scripts").blue());
    let mut cmd_names: Vec<_> = scripts.keys().cloned().collect();
    cmd_names.sort();

    for cmd_name in cmd_names {
        // `get` is guaranteed to return Some within this loop.
        let task = scripts.get(&cmd_name).unwrap();

        print!("    - {}", cmd_name.cyan());

        if let Some(d) = &task.desc {
            if !d.trim().is_empty() {
                print!(": {}", d.dimmed());
            }
        }
        println!();
    }
    Ok(())
}

/// Renders a template AST back into a representative string for display.
fn render_template_to_string(template: &[TemplateComponent]) -> String {
    template
        .iter()
        .map(|c| match c {
            TemplateComponent::Literal(s) => s.clone(),
            TemplateComponent::Parameter(p) => p.original_token.clone(),
            TemplateComponent::GenericParams => "<axes::params>".to_string(),
            TemplateComponent::Run(spec) => match spec {
                RunSpec::Literal(cmd) => format!("<axes::run('{}')>", cmd),
            },
            TemplateComponent::Path => "<axes::path>".to_string(),
            TemplateComponent::Name => "<axes::name>".to_string(),
            TemplateComponent::Uuid => "<axes::uuid>".to_string(),
            TemplateComponent::Version => "<axes::version>".to_string(),
            TemplateComponent::Script(s) => format!("<axes::scripts::{}>", s),
            TemplateComponent::Var(v) => format!("<axes::vars::{}>", v),
        })
        .collect::<String>()
}

/// A generic function to print key-value maps like [vars] and [env].
fn print_variables(
    config: &ResolvedConfig,
    key: &str,
    title: &str,
    index: &mut GlobalIndex,
) -> Result<()> {
    if key == "vars" {
        // Merge all vars from all layers to display a complete view.
        let vars = config.get_all_vars(index)?;
        if vars.is_empty() {
            return Ok(());
        }

        println!("\n  {}:", title.blue());
        let mut sorted_keys: Vec<_> = vars.keys().cloned().collect();
        sorted_keys.sort();

        for k in sorted_keys {
            if let Some(task) = vars.get(&k) {
                // Render the task's AST back into a representative string.
                let display_val = task
                    .commands
                    .iter()
                    .map(|cmd| {
                        let template = match &cmd.action {
                            CommandAction::Execute(t) | CommandAction::Print(t) => t,
                        };
                        render_template_to_string(template)
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
        // Use the accessor method for the fully merged env.
        let env = config.get_env(index)?;
        if env.is_empty() {
            return Ok(());
        }

        println!("\n  {}:", title.blue());
        let mut sorted_keys: Vec<_> = env.keys().cloned().collect();
        sorted_keys.sort();

        for k in sorted_keys {
            if let Some(val) = env.get(&k) {
                println!("    - {} = {}", k.cyan(), format_args!("\"{}\"", val));
            }
        }
    }
    Ok(())
}
