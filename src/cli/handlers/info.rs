// EN: src/cli/handlers/info.rs (REFACTORED FOR CLARITY AND COMPLETENESS)

use crate::{
    cli::handlers::commons,
    constants::{AXES_DIR, PROJECT_CONFIG_FILENAME},
    core::index_manager,
    models::{CommandAction, GlobalIndex, ResolvedConfig, RunSpec, Task, TemplateComponent},
};
use anyhow::Result;
use clap::Parser;
use colored::*;
use std::sync::Arc;

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Displays detailed information about a project's configuration."
)]
struct InfoArgs {}

/// The main handler for the `info` command.
/// Displays detailed, lazily-resolved information about a project, including its inheritance.
pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    let _info_args = InfoArgs::try_parse_from(&args)?;
    let config = commons::resolve_config_for_context(context, index)?;

    print_metadata(&config, index)?;
    print_options(&config)?;
    print_task_map(
        "scripts",
        t!("info.label.available_scripts"),
        &config,
        index,
    )?;
    print_task_map("vars", t!("info.label.vars"), &config, index)?;
    print_env(&config)?;

    println!("\n---------------------------------");
    Ok(())
}

/// Prints the core metadata of the project, including its inheritance chain.
fn print_metadata(config: &ResolvedConfig, index: &GlobalIndex) -> Result<()> {
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

    // [NEW] Display the full inheritance hierarchy for clarity.
    let hierarchy_names: Vec<String> = config
        .hierarchy
        .iter()
        .map(|uuid| {
            index
                .projects
                .get(uuid)
                .map(|e| e.name.clone())
                .unwrap_or_else(|| "unknown".to_string())
        })
        .collect();
    println!(
        "  {:<15} {}",
        t!("info.label.inheritance").blue(),
        hierarchy_names.join(" -> ").dimmed()
    );

    if let Some(v) = config.get_version()? {
        println!("  {:<15} {}", t!("info.label.version").blue(), v);
    }
    if let Some(d) = config.get_description()? {
        println!("  {:<15} {}", t!("info.label.description").blue(), d);
    }
    Ok(())
}

/// [NEW] Prints the resolved [options] section.
fn print_options(config: &ResolvedConfig) -> Result<()> {
    let options = config.get_options()?;
    let mut entries = Vec::new();

    if let Some(shell) = options.shell {
        entries.push(format!("{}: '{}'", "shell".cyan(), shell));
    }
    if let Some(cache_dir) = options.cache_dir {
        entries.push(format!("{}: '{}'", "cache_dir".cyan(), cache_dir));
    }
    if let Some(default_open) = options.open_with.default {
        entries.push(format!(
            "{}: '{}'",
            "open_with.default".cyan(),
            default_open
        ));
    }

    if entries.is_empty() {
        return Ok(());
    }

    println!("\n  {}:", t!("info.label.options").blue());
    for entry in entries {
        println!("    - {}", entry);
    }
    Ok(())
}

/// [REFACTORED] A generic function to print a map of Tasks (like `scripts` or `vars`).
/// It now indicates the source of inherited items.
fn print_task_map(
    key: &str,
    title: &str,
    config: &ResolvedConfig,
    index: &GlobalIndex,
) -> Result<()> {
    let tasks = if key == "scripts" {
        config.get_all_scripts()?
    } else {
        config.get_all_vars()?
    };

    if tasks.is_empty() {
        if key == "scripts" {
            println!("\n  {}", t!("info.label.no_scripts").dimmed());
        }
        return Ok(());
    }

    println!("\n  {}:", title.blue());
    let mut sorted_keys: Vec<_> = tasks.keys().cloned().collect();
    sorted_keys.sort();

    for task_name in sorted_keys {
        let task = tasks.get(&task_name).unwrap(); // Safe
        print!("    - {}", task_name.cyan());

        // [NEW] Find and display the source of the task for inherited items.
        let source_project_name = find_task_source(key, &task_name, config, index)?;
        if source_project_name != config.qualified_name {
            print!(
                " {}",
                format!(
                    "[{}]",
                    format_args!(t!("common.label.inherited"), from = source_project_name)
                )
                .dimmed()
            );
        }

        if let Some(d) = &task.desc
            && !d.trim().is_empty()
        {
            print!(": {}", d.dimmed());
        }

        // For `vars`, we also display the rendered value.
        if key == "vars" {
            let display_val = render_task_to_string(task);
            print!(" = {}", format!("\"{}\"", display_val));
        }
        println!();
    }
    Ok(())
}

/// Prints all merged environment variables.
fn print_env(config: &ResolvedConfig) -> Result<()> {
    let env = config.get_env()?;
    if env.is_empty() {
        return Ok(());
    }

    println!("\n  {}:", t!("info.label.env").blue());
    let mut sorted_keys: Vec<_> = env.keys().cloned().collect();
    sorted_keys.sort();

    for k in sorted_keys {
        if let Some(val) = env.get(&k) {
            println!("    - {} = {}", k.cyan(), format!("\"{}\"", val));
        }
    }
    Ok(())
}

// --- Helper Functions ---

/// Renders a template AST back into a representative string for display.
fn render_template_to_string(template: &[TemplateComponent]) -> String {
    template.iter().map(render_component_to_string).collect()
}

/// Renders a single Task back to its most representative string form.
fn render_task_to_string(task: &Arc<Task>) -> String {
    task.commands
        .iter()
        .map(|cmd| {
            let template = match &cmd.action {
                CommandAction::Execute(t) | CommandAction::Print(t) => t,
            };
            render_template_to_string(template)
        })
        .collect::<Vec<_>>()
        .join(" && ")
}

/// Renders a single TemplateComponent to its string representation.
fn render_component_to_string(component: &TemplateComponent) -> String {
    match component {
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
    }
}

/// [NEW] Traverses the hierarchy to find which project a script/var originates from.
pub(crate) fn find_task_source(
    key: &str,
    task_name: &str,
    config: &ResolvedConfig,
    index: &GlobalIndex,
) -> Result<String> {
    for uuid in config.hierarchy.iter() {
        let layer = config.get_layer(*uuid)?;
        let tasks = if key == "scripts" {
            &layer.scripts
        } else {
            &layer.vars
        };
        if tasks.contains_key(task_name) {
            return Ok(index_manager::build_qualified_name(*uuid, index)
                .unwrap_or_else(|| "unknown".to_string()));
        }
    }
    Ok(config.qualified_name.clone()) // Fallback to current project name
}
