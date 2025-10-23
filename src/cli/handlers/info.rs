//! # Handler for the `info` command
//!
//! This module provides the logic for the `axes info` command, which displays a comprehensive,
//! human-readable summary of a project's fully resolved configuration.
//!
//! ## Core Logic
//!
//! 1.  **Configuration Resolution**: It uses `commons::resolve_config_for_context` to obtain
//!     the `ResolvedConfig` for the target project. This is a potentially mutable operation,
//!     as it can trigger cache updates in the `GlobalIndex`.
//! 2.  **Sectioned Display**: The output is broken down into logical sections (Metadata, Options,
//!     Scripts, Variables, Environment), each handled by a dedicated `print_*` function.
//! 3.  **Inheritance Indication**: For inherited items like scripts and variables, it uses the
//!     `find_task_source` helper to determine which project in the hierarchy originally defined
//!     the item, adding a `[inherited from ...]` label to the output for clarity.
//! 4.  **Data Rendering**: It includes logic to render complex data structures, like variable
//!     values, into a representative string format for display, respecting platform-specific
//!     definitions.

use crate::{
    cli::handlers::commons,
    constants::{AXES_DIR, PROJECT_CONFIG_FILENAME},
    core::index_manager,
    models::{
        CachedVar, CommandAction, GlobalIndex, PlatformExecution, ResolvedConfig, RunSpec,
        TemplateComponent,
    },
    state::AppStateGuard,
};
use anyhow::Result;
use clap::Parser;
use colored::*;
use std::sync::Arc;

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Displays detailed information about a project's configuration."
)]
struct InfoArgs {
    /// The project context to display information about. Defaults to the current project.
    context: Option<String>,
}

// --- Main Handler ---

/// The main handler for the `info` command.
///
/// It resolves the full configuration for a project and then calls a series of
/// helper functions to print each section of the resolved data in a formatted way.
///
/// # Arguments
/// * `dispatcher_context` - The context from the dispatcher (e.g., the project part of `axes my-app info`).
/// * `args` - Command-specific arguments, which may include an overriding context.
/// * `state_guard` - A mutable guard to the application state, needed for configuration resolution.
pub fn handle(
    dispatcher_context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard<'_>,
) -> Result<()> {
    // 1. Parse all handler-specific arguments.
    let info_args = InfoArgs::try_parse_from(&args)?;

    // 2. Determine the definitive context with clear priority: cli arg > dispatcher context > cwd.
    let final_context = info_args
        .context
        .or(dispatcher_context)
        .unwrap_or_else(|| ".".to_string());

    // 3. Lazily resolve the full configuration for the context.
    let config = commons::resolve_config_for_context(Some(final_context), state_guard)?;
    let index = state_guard.index();

    // 4. Print each section of the configuration.
    print_metadata(&config, index)?;
    print_options(&config)?;
    print_scripts_map(&config, index)?;
    print_vars_map(&config, index)?;
    print_env(&config)?;

    println!("\n---------------------------------");
    Ok(())
}

// --- Display Functions for Each Section ---

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

    // Display the full inheritance hierarchy for clarity.
    let hierarchy_names: Vec<String> = config
        .hierarchy
        .iter()
        .map(|uuid| {
            index_manager::build_qualified_name(*uuid, index)
                .unwrap_or_else(|| format!("<unknown: {}>", uuid))
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

/// Prints the resolved `[options]` section.
fn print_options(config: &ResolvedConfig) -> Result<()> {
    let options = config.get_options()?;
    let mut entries = Vec::new();

    if let Some(shell) = &options.shell {
        entries.push(format!("{}: '{}'", "shell".cyan(), shell));
    }
    if let Some(prompt) = &options.prompt {
        entries.push(format!("{}: '{}'", "prompt".cyan(), prompt));
    }
    if let Some(cache_dir) = &options.cache_dir {
        entries.push(format!("{}: '{}'", "cache_dir".cyan(), cache_dir));
    }
    if let Some(default_open) = &options.open_with.default {
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

/// Prints a map of available scripts, indicating their source.
fn print_scripts_map(config: &ResolvedConfig, index: &GlobalIndex) -> Result<()> {
    let scripts = config.get_all_scripts()?;
    if scripts.is_empty() {
        println!("\n  {}", t!("info.label.no_scripts").dimmed());
        return Ok(());
    }

    println!("\n  {}:", t!("info.label.available_scripts").blue());
    let mut sorted_keys: Vec<_> = scripts.keys().collect();
    sorted_keys.sort();

    for script_name in sorted_keys {
        let task = scripts.get(script_name).unwrap();
        print!("    - {}", script_name.cyan());

        let source_project_name = find_task_source("scripts", script_name, config, index)?;
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
        println!();
    }
    Ok(())
}

/// Prints a map of available variables, indicating their source and description or value.
fn print_vars_map(config: &ResolvedConfig, index: &GlobalIndex) -> Result<()> {
    let vars = config.get_all_vars()?;
    if vars.is_empty() {
        return Ok(());
    }

    println!("\n  {}:", t!("info.label.vars").blue());
    let mut sorted_keys: Vec<_> = vars.keys().collect();
    sorted_keys.sort();

    for var_name in sorted_keys {
        let var = vars.get(var_name).unwrap();
        print!("    - {}", var_name.cyan());

        let source_project_name = find_task_source("vars", var_name, config, index)?;
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

        // --- CORRECTED LOGIC ---
        // Priority: 1. Description, 2. Rendered value.
        if let Some(d) = &var.desc {
            if !d.trim().is_empty() {
                print!(": {}", d.dimmed());
            } else {
                // Description is present but empty, show the value instead.
                let display_val = render_var_to_string(var, config);
                print!(" = {}", format_args!("\"{}\"", display_val));
            }
        } else {
            // No description, show the value.
            let display_val = render_var_to_string(var, config);
            print!(" = {}", format_args!("\"{}\"", display_val));
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
    let mut sorted_keys: Vec<_> = env.keys().collect();
    sorted_keys.sort();

    for k in sorted_keys {
        if let Some(val) = env.get(k) {
            println!("    - {} = {}", k.cyan(), format_args!("\"{}\"", val));
        }
    }
    Ok(())
}

// --- Helper Functions ---

/// Traverses the project's inheritance hierarchy to find which ancestor project
/// a specific script or variable was originally defined in.
///
/// # Arguments
/// * `key` - The type of item to look for ("scripts" or "vars").
/// * `task_name` - The name of the script or variable.
/// * `config` - The resolved configuration of the starting project.
/// * `index` - An immutable reference to the global index.
///
/// # Returns
/// The qualified name of the project where the item was found.
pub(crate) fn find_task_source(
    key: &str,
    task_name: &str,
    config: &ResolvedConfig,
    index: &GlobalIndex,
) -> Result<String> {
    for uuid in config.hierarchy.iter() {
        let layer = config.get_layer(*uuid)?;
        let task_exists = if key == "scripts" {
            layer.scripts.contains_key(task_name)
        } else {
            layer.vars.contains_key(task_name)
        };

        if task_exists {
            return Ok(index_manager::build_qualified_name(*uuid, index)
                .unwrap_or_else(|| format!("<unknown: {}>", uuid)));
        }
    }
    Ok(config.qualified_name.clone()) // Fallback to current project name
}

/// Selects the appropriate command template for the current platform from a `PlatformExecution` block.
fn get_template_for_platform<'a>(
    plat_exec: &'a PlatformExecution,
    config: &'a ResolvedConfig,
) -> Option<&'a [TemplateComponent]> {
    config
        .select_platform_exec(plat_exec)
        .map(|cmd_exec| match &cmd_exec.action {
            CommandAction::Execute(t) | CommandAction::Print(t) => t.as_slice(),
        })
}

/// Renders a template AST back into a representative string for display.
fn render_template_to_string(template: &[TemplateComponent]) -> String {
    template.iter().map(render_component_to_string).collect()
}

/// A helper that renders an abstract `TemplateComponent` into its original string representation
/// (e.g., `<params::0>`, `<#red>`). This is used for display purposes in `info` and `start --dry-run`.
fn render_component_to_string(component: &TemplateComponent) -> String {
    match component {
        TemplateComponent::Literal(s) => s.clone(),
        TemplateComponent::Parameter(p) => p.original_token.clone(),
        TemplateComponent::GenericParams { literal } => {
            if *literal {
                "<params(literal)>".to_string()
            } else {
                "<params>".to_string()
            }
        }
        TemplateComponent::Color(color) => format!("<#{:?}>", color).to_lowercase(),
        TemplateComponent::Run(spec) => match spec {
            RunSpec::Literal(cmd) => format!("<run('{}')>", cmd),
        },
        TemplateComponent::Path => "<path>".to_string(),
        TemplateComponent::Name => "<name>".to_string(),
        TemplateComponent::Uuid => "<uuid>".to_string(),
        TemplateComponent::Version => "<version>".to_string(),
        TemplateComponent::Script(s) => format!("<scripts::{}>", s),
        TemplateComponent::Var(v) => format!("<vars::{}>", v),
    }
}

/// A helper that renders the value of a `CachedVar` to a string for display.
/// It correctly selects the value for the current platform before rendering.
fn render_var_to_string(var: &Arc<CachedVar>, config: &ResolvedConfig) -> String {
    get_template_for_platform(&var.value, config)
        .map(render_template_to_string)
        .unwrap_or_else(|| "<no value for this platform>".dimmed().to_string())
}
