// src/bin/axes.rs

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use clap::Parser;
use std::{env, fs, path::PathBuf};
use uuid::Uuid;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axes::cli::Cli;
use axes::models::Runnable;
use axes::system::shell;

use axes::constants::{AXES_DIR, PROJECT_CONFIG_FILENAME};
use axes::core::graph_display;
use axes::core::{
    config_resolver, context_resolver, index_manager, onboarding_manager,
    onboarding_manager::OnboardingOptions,
};
use axes::models::{Command as ProjectCommand, ProjectConfig, ProjectRef, ResolvedConfig};

use dialoguer::{Confirm, theme::ColorfulTheme};

/// The main entry point of the application.
fn main() {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // This runs on a separate thread when Ctrl+C is pressed.
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        println!("\nPor favor no intente cerrar forzosamente, puede cerrar de forma segura el shell usando `exit`.");
    }).expect("Error al establecer el manejador de Ctrl-C");

    // Initialize the logger.
    env_logger::init();

    // Parse command line arguments.
    let cli = Cli::parse();

    // Execute the main logic and handle any errors.
    if let Err(e) = run_cli(cli) {
        // Do not show the error if it was due to a user interruption.
        if running.load(Ordering::SeqCst) {
            eprintln!("\nError: {:?}", e);
            std::process::exit(1);
        } else {
            // The error was likely caused by the interruption, so we exit silently.
            println!("\nOperation cancelled.");
            std::process::exit(130); // Standard exit code for Ctrl+C
        }
    }
}

/// The main application dispatcher.
fn run_cli(cli: Cli) -> Result<()> {
    log::debug!("CLI args parsed: {:?}", cli);

    const SYSTEM_PROJECT_ACTIONS: &[&str] = &[
        "tree",
        "info",
        "open",
        "rename",
        "link",
        "unregister",
        "delete",
        "run",
        "start",
    ];
    const SYSTEM_GLOBAL_ACTIONS: &[&str] = &["init", "register", "alias"];

    // 1. Initial Parsing
    let arg1 = match cli.context_or_action {
        Some(a) => a,
        None => {
            println!("TODO: Lanzar la TUI.");
            return Ok(());
        }
    };

    let mut remaining_args = Vec::new();
    if let Some(arg2) = cli.action_or_context_or_arg {
        remaining_args.push(arg2);
    }
    remaining_args.extend(cli.args);

    // 2. Global Actions Filter
    if SYSTEM_GLOBAL_ACTIONS.contains(&arg1.as_str()) {
        let action = arg1;
        let sub_command_or_context = remaining_args.first().cloned();
        let final_args = remaining_args.into_iter().skip(1).collect();

        return match action.as_str() {
            "init" => handle_init(sub_command_or_context, final_args),
            "register" => handle_register(sub_command_or_context, final_args),
            "alias" => handle_alias(sub_command_or_context, final_args),
            _ => unreachable!(),
        };
    }

    // 3. Mode Detection and Execution
    if let Ok(project_uuid_str) = std::env::var("AXES_PROJECT_UUID") {
        // --- SESSION MODE ---
        let action = arg1;
        let args = remaining_args;

        let project_uuid = Uuid::parse_str(&project_uuid_str)?;
        let index = index_manager::load_and_ensure_global_project()?;
        let qualified_name = index_manager::build_qualified_name(project_uuid, &index)
            .ok_or_else(|| anyhow!("Could not reconstruct project name from session."))?;
        let config =
            config_resolver::resolve_config_for_uuid(project_uuid, qualified_name, &index)?;

        // Call the project action dispatcher
        execute_project_action(config, action, args, SYSTEM_PROJECT_ACTIONS)?;
    } else {
        // --- SCRIPT MODE ---
        let arg2 = remaining_args.first();

        let (context_str, action_str, final_args) =
            if SYSTEM_PROJECT_ACTIONS.contains(&arg1.as_str()) {
                // Format: `axes <action> <context> [args...]`
                let context = arg2
                    .cloned()
                    .ok_or_else(|| anyhow!("Action '{}' requires a project context.", arg1))?;
                (context, arg1, remaining_args.into_iter().skip(1).collect())
            } else {
                // Format: `axes <context> [action?] [args...]`
                let context = arg1;
                let action = arg2.cloned().unwrap_or_else(|| "start".to_string());
                let args = remaining_args.into_iter().skip(1).collect();
                (context, action, args)
            };

        // `tree` without context (or with `global`) is a special case
        if action_str == "tree" && (context_str == "global" || context_str.is_empty()) {
            return handle_tree(None);
        }

        let index = index_manager::load_and_ensure_global_project()?;
        let (uuid, qualified_name) = context_resolver::resolve_context(&context_str, &index)?;
        let config = config_resolver::resolve_config_for_uuid(uuid, qualified_name, &index)?;

        execute_project_action(config, action_str, final_args, SYSTEM_PROJECT_ACTIONS)?;
    }

    Ok(())
}

/// Executes an action that operates on an already resolved project configuration.
fn execute_project_action(
    config: ResolvedConfig,
    action: String,
    args: Vec<String>,
    system_actions: &[&str],
) -> Result<()> {
    log::debug!(
        "Executing action '{}' for project '{}'",
        action,
        config.qualified_name
    );

    match action.as_str() {
        "tree" => handle_tree(Some(config)),
        "start" => handle_start(&config),
        "info" => handle_info(&config),
        "open" => handle_open(&config, args),
        "rename" => handle_rename(&config, args),
        "link" => handle_link(&config, args),
        "unregister" => handle_unregister(&config, args),
        "delete" => handle_delete(&config, args),
        "run" => {
            let script_name = args.first().cloned();
            let params = args.into_iter().skip(1).collect();
            handle_run(&config, script_name, params)
        }
        // Shortcut for `run`
        script_name if !system_actions.contains(&script_name) => {
            handle_run(&config, Some(action), args)
        }
        _ => {
            anyhow::bail!(
                "System action '{}' is valid but has no implemented handler.",
                action
            );
        }
    }
}

// --- ACTION HANDLERS (Implementations) ---

///Allows creating and registering new projects to axes.
fn handle_init(name_arg: Option<String>, args: Vec<String>) -> Result<()> {
    let project_name = name_arg
        .ok_or_else(|| anyhow!("El comando 'init' requiere un nombre para el nuevo proyecto."))?;

    // Simple argument parsing for --parent
    let mut parent_context: Option<String> = None;
    if let Some(pos) = args.iter().position(|r| r == "--parent") {
        parent_context = args.get(pos + 1).cloned();
    }

    let current_dir = env::current_dir()?;
    println!(
        "Inicializando proyecto '{}' en {}",
        project_name,
        current_dir.display()
    );

    // 1. Validate that a .axes directory does not already exist in the current directory
    let axes_dir = current_dir.join(AXES_DIR);
    if axes_dir.exists() {
        return Err(anyhow!(
            "An '.axes' directory already exists at this location."
        ));
    }

    // 2. Load index and resolve parent (if specified)
    let mut index = index_manager::load_and_ensure_global_project()?;
    let final_parent_uuid: Uuid = match parent_context {
        Some(context) => {
            println!("Resolviendo padre '{}'...", context);
            let (uuid, qualified_name) = context_resolver::resolve_context(&context, &index)?;
            println!(
                "Proyecto padre '{}' encontrado (UUID: {}).",
                qualified_name, uuid
            );
            uuid
        }
        None => {
            println!(
                "No parent specified. Linking to 'global' project. (UUID: {})",
                index_manager::GLOBAL_PROJECT_UUID
            );
            index_manager::GLOBAL_PROJECT_UUID
        }
    };

    // 3. Add the new project to the index
    let canonical_path = current_dir.canonicalize()?;
    let (new_uuid, _) = index_manager::add_project_to_index(&mut index, project_name.clone(), canonical_path.clone(), Some(final_parent_uuid))
        .context("Could not add project to global index. There might be a sibling project with the same name.")?;

    // 4. Create the project file structure on disk
    fs::create_dir_all(&axes_dir)?;
    let config_path = axes_dir.join(PROJECT_CONFIG_FILENAME);
    let default_config = ProjectConfig::new();
    let toml_string = toml::to_string_pretty(&default_config)?;
    fs::write(&config_path, toml_string)?;

    // 5. Create and save the local reference file (`project_ref.bin`)
    let project_ref = ProjectRef {
        self_uuid: new_uuid,
        parent_uuid: Some(final_parent_uuid), // The definitive parent
        name: project_name.clone(),
    };
    index_manager::write_project_ref(&canonical_path, &project_ref)
        .context("No se pudo escribir el archivo de referencia del proyecto (project_ref.bin).")?;

    // 6. Save the updated global index
    index_manager::save_global_index(&index).context("Could not save updated global index.")?;

    println!("\n✔ Success!");
    println!(
        "  Proyecto '{}' creado con UUID: {}",
        project_name, new_uuid
    );
    println!("  Configuration created at: {}", config_path.display());
    println!(
        "  Identidad local guardada en: .axes/{}",
        axes::constants::PROJECT_REF_FILENAME
    );
    println!("  Successfully registered in global index.");

    Ok(())
}

fn handle_link(config: &ResolvedConfig, args: Vec<String>) -> Result<()> {
    // 1. Get the context of the new parent.
    let new_parent_context = args
        .first()
        .ok_or_else(|| anyhow!("El comando 'link' requiere el contexto del nuevo padre."))?
        .trim();

    if new_parent_context.is_empty() {
        return Err(anyhow!("New parent context cannot be empty."));
    }
    // We do not validate path characters here because it is a context, not a direct name.

    println!(
        "Intentando mover '{}' a ser hijo de '{}'...",
        config.qualified_name, new_parent_context
    );

    // 2. Load the global index and resolve the UUID of the new parent.
    let mut index = index_manager::load_and_ensure_global_project()?;
    let (new_parent_uuid, new_parent_qualified_name) =
        context_resolver::resolve_context(new_parent_context, &index).context(format!(
            "No se pudo resolver el contexto del nuevo padre '{}'.",
            new_parent_context
        ))?;

    // 3. Critical validations (in the `index_manager`):
    //    a. Anti-Cycles
    //    b. Anti-Sibling Name Collision
    index_manager::link_project(&mut index, config.uuid, new_parent_uuid).context(format!(
        "No se pudo establecer el enlace para el proyecto '{}'.",
        config.qualified_name
    ))?;

    // 4. Save the modified global index.
    index_manager::save_global_index(&index).context("Could not save updated global index.")?;

    // 5. Update the local `project_ref.bin` (using `get_or_create_project_ref`)
    let mut project_ref =
        index_manager::get_or_create_project_ref(&config.project_root, config.uuid, &index)
            .context(
                "No se pudo obtener o crear la referencia local del proyecto (`project_ref.bin`).",
            )?;

    project_ref.parent_uuid = Some(new_parent_uuid);
    if let Err(e) = index_manager::write_project_ref(&config.project_root, &project_ref) {
        eprintln!(
            "\nWarning: Project was linked in global index, but local reference file `project_ref.bin` could not be updated: {}",
            e
        );
    }

    println!("\n✔ Success!");
    println!(
        "El proyecto '{}' ahora es hijo de '{}'.",
        config.qualified_name, new_parent_qualified_name
    );
    println!("Note: Caches will be automatically regenerated on next resolve.");

    Ok(())
}

/// Starts an interactive terminal session for the project.
fn handle_start(config: &ResolvedConfig) -> Result<()> {
    println!("\nStarting session for '{}'...", config.qualified_name);

    // We simply call our new function.
    // We use `with_context` to add useful information to the error if it occurs.
    shell::launch_interactive_shell(config).with_context(|| {
        format!(
            "Could not start session for project '{}'",
            config.qualified_name
        )
    })
}

/// Executes a command defined in the project's `axes.toml`.
fn handle_run(
    config: &ResolvedConfig,
    script_name: Option<String>,
    params: Vec<String>,
) -> Result<()> {
    let script_key = script_name
        .ok_or_else(|| anyhow!("Debe especificar un script para ejecutar con 'run'."))?;

    let command_def = config.commands.get(&script_key).ok_or_else(|| {
        anyhow!(
            "Script '{}' not found in project configuration.",
            script_key
        )
    })?;

    // 1. Get the `Runnable` from the command definition.
    let runnable_template = match command_def {
        ProjectCommand::Sequence(s) => Runnable::Sequence(s.clone()),
        ProjectCommand::Simple(s) => Runnable::Single(s.clone()),
        ProjectCommand::Extended(ext) => ext.run.clone(),
        ProjectCommand::Platform(pc) => {
            let os_specific_runnable = if cfg!(target_os = "windows") {
                pc.windows.as_ref()
            } else if cfg!(target_os = "linux") {
                pc.linux.as_ref()
            } else if cfg!(target_os = "macos") {
                pc.macos.as_ref()
            } else {
                None
            };

            os_specific_runnable
                .or(pc.default.as_ref())
                .ok_or_else(|| {
                    anyhow!(
                        "Script '{}' has no implementation for the current OS and no 'default'.",
                        script_key
                    )
                })?
                .clone()
        }
    };

    // 2. Execute the `Runnable`.
    let interpolator = axes::core::interpolator::Interpolator::new(config, &params);

    match runnable_template {
        Runnable::Single(command_template) => {
            let final_command = interpolator.interpolate(&command_template);
            println!("\n> {}", final_command);
            axes::system::executor::execute_command(
                &final_command,
                &config.project_root,
                &config.env,
            )
            .map_err(|e| anyhow!(e))?;
        }
        Runnable::Sequence(command_templates) => {
            println!(
                "\nEjecutando secuencia de comandos para '{}'...",
                script_key
            );
            for (i, command_template) in command_templates.iter().enumerate() {
                let final_command = interpolator.interpolate(command_template);
                println!(
                    "\n[{}/{}]> {}",
                    i + 1,
                    command_templates.len(),
                    final_command
                );

                // If any step fails, `?` will stop execution and propagate the error.
                axes::system::executor::execute_command(
                    &final_command,
                    &config.project_root,
                    &config.env,
                )
                .map_err(|e| anyhow!(e))?;
            }
            println!("\n✔ Sequence completed successfully.");
        }
    }

    Ok(())
}

/// Displays detailed information about the resolved project configuration.
fn handle_info(config: &ResolvedConfig) -> Result<()> {
    let config_file_path = config
        .project_root
        .join(AXES_DIR)
        .join(PROJECT_CONFIG_FILENAME);

    println!("\n--- Information for '{}' ---", config.qualified_name);
    println!("  UUID:           {}", config.uuid);
    println!("  Root Path:    {}", config.project_root.display());
    println!("  Archivo Conf:   {}", config_file_path.display());

    if let Some(v) = &config.version {
        println!("  Version:        {}", v);
    }
    if let Some(d) = &config.description {
        println!("  Description:    {}", d);
    }

    if !config.commands.is_empty() {
        println!("\n  Comandos Disponibles:");
        let mut cmd_names: Vec<_> = config.commands.keys().collect();
        cmd_names.sort();
        for cmd_name in cmd_names {
            if let Some(command_def) = config.commands.get(cmd_name) {
                // **FIX**: The match now extracts the internal `ext` struct.
                match command_def {
                    ProjectCommand::Sequence(_) => {
                        println!("    - {} (secuencia de comandos)", cmd_name)
                    }
                    ProjectCommand::Extended(ext) => {
                        if let Some(d) = &ext.desc {
                            println!("    - {} : {}", cmd_name, d);
                        } else {
                            println!("    - {}", cmd_name);
                        }
                    }
                    ProjectCommand::Simple(_) => {
                        println!("    - {}", cmd_name)
                    }
                    ProjectCommand::Platform(pc) => {
                        if let Some(d) = &pc.desc {
                            println!("    - {} : {}", cmd_name, d);
                        } else {
                            println!("    - {} (multi-plataforma)", cmd_name);
                        }
                    }
                }
            }
        }
    } else {
        println!("\n  No hay comandos definidos.");
    }

    if !config.vars.is_empty() {
        println!("\n  Variables (fusionadas):");
        for (key, val) in &config.vars {
            println!("    - {} = \"{}\"", key, val);
        }
    }

    if !config.env.is_empty() {
        println!("\n  Variables de Entorno (fusionadas):");
        for (key, val) in &config.env {
            println!("    - {} = \"{}\"", key, val);
        }
    }

    println!("\n--------------------------");
    Ok(())
}

/// Opens the project with a configured application.
fn handle_open(config: &ResolvedConfig, args: Vec<String>) -> Result<()> {
    // 1. Determine the key for the open action.
    let open_key = if !args.is_empty() && args[0] == "with" {
        // Case: `axes ... open with vsc`
        args.get(1) // Get the app name
            .map(|s| s.as_str())
            .ok_or_else(|| {
                anyhow!(
                    "'open with' command requires an application name (e.g. 'vsc', 'explorer')."
                )
            })?
    } else if !args.is_empty() {
        // Case: `axes ... open vsc` (shortcut)
        args[0].as_str()
    } else {
        // Case: `axes ... open` (use the default)
        config
            .options
            .open_with
            .get("default")
            .ok_or_else(|| {
                anyhow!("No application specified and no 'default' key in [options.open_with].")
            })?
            .as_str()
    };

    // 2. Search for the command in the configuration.
    // If the key is "default", the user made a mistake, as "default" must point to another key.
    if open_key == "default" {
        return Err(anyhow!(
            "The 'default' key must point to the name of another open action (e.g. default = \"vsc\")."
        ));
    }

    let command_template = config.options.open_with.get(open_key).ok_or_else(|| {
        anyhow!(
            "No open action found for '{}' in [options.open_with].",
            open_key
        )
    })?;

    // 3. Interpolate and execute. For now, {root} and {path} are the same.
    let interpolator = axes::core::interpolator::Interpolator::new(config, &[]);
    let final_command = interpolator.interpolate(command_template);

    println!("\n> {}", final_command);

    axes::system::executor::execute_command(&final_command, &config.project_root, &config.env)
        .map_err(|e| anyhow!(e))
}

fn handle_rename(config: &ResolvedConfig, args: Vec<String>) -> Result<()> {
    let new_name = args
        .first()
        .ok_or_else(|| anyhow!("El comando 'rename' requiere un nuevo nombre para el proyecto."))?
        .trim();

    if new_name.is_empty() {
        return Err(anyhow!("New name cannot be empty."));
    }
    // Validate that the new name does not contain path characters ('/' or '\')
    if new_name.contains('/') || new_name.contains('\\') {
        return Err(anyhow!("El nuevo nombre no puede contener '/' o '\\'."));
    }
    // Validate that it is not a reserved name
    if ["global", ".", "..", "*", "_", "**"].contains(&new_name.to_lowercase().as_str()) {
        return Err(anyhow!(
            "El nombre '{}' es reservado y no puede usarse para un proyecto.",
            new_name
        ));
    }

    println!(
        "Renombrando '{}' a '{}'...",
        config.qualified_name, new_name
    );

    // 1. Load the global index for modification (critical operation)
    let mut index = index_manager::load_and_ensure_global_project()?;

    // 2. Rename the project in the in-memory index (this includes sibling validation)
    index_manager::rename_project(&mut index, config.uuid, new_name).with_context(|| {
        format!(
            "Could not rename project '{}' in global index.",
            config.qualified_name
        )
    })?;

    // 3. Save the modified global index to disk
    index_manager::save_global_index(&index).context("Could not save updated global index.")?;

    // 4. Get and update the local project reference (project_ref.bin)
    //    This logic is encapsulated in `get_or_create_project_ref` for self-healing.
    let mut project_ref = index_manager::get_or_create_project_ref(&config.project_root, config.uuid, &index)
        .with_context(|| format!("No se pudo obtener o crear la referencia local del proyecto `project_ref.bin` para '{}'.", config.qualified_name))?;

    // 5. Update the name in the reference and save it.
    project_ref.name = new_name.to_string();
    if let Err(e) = index_manager::write_project_ref(&config.project_root, &project_ref) {
        eprintln!(
            "\nWarning: Project was renamed in global index, but local reference file `project_ref.bin` at `{}` could not be updated: {}",
            config.project_root.display(),
            e
        );
    }

    println!("\n✔ Success!");
    println!(
        "El proyecto '{}' ha sido renombrado a '{}'.",
        config.qualified_name, new_name
    );
    println!(
        "Note: The full qualified name might have changed. Caches will be automatically regenerated on next resolve."
    );

    Ok(())
}

///Register existing project.
fn handle_unregister(config: &ResolvedConfig, args: Vec<String>) -> Result<()> {
    let unregister_children = args.iter().any(|arg| arg == "--children");
    let mut index = index_manager::load_and_ensure_global_project()?;

    let mut uuids_to_unregister = vec![config.uuid];
    if unregister_children {
        println!(
            "Recolectando todos los descendientes de '{}'...",
            config.qualified_name
        );
        uuids_to_unregister.extend(index_manager::get_all_descendants(&index, config.uuid));
    }

    println!("\nThe following `axes` entries will be unregistered (files will not be modified):");
    for uuid in &uuids_to_unregister {
        if let Some(entry) = index.projects.get(uuid) {
            println!("  - {} (en {})", entry.name, entry.path.display());
        }
    }

    if !unregister_children
        && index
            .projects
            .values()
            .any(|e| e.parent == Some(config.uuid))
    {
        println!(
            "\nNote: Direct children of '{}' will become children of 'global'.",
            config.qualified_name
        );
    }

    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Continue?")
        .default(false)
        .interact()?
    {
        println!("Operation cancelled.");
        return Ok(());
    }

    let should_reparent = !unregister_children;
    let removed_count =
        index_manager::remove_from_index(&mut index, &uuids_to_unregister, should_reparent);

    index_manager::save_global_index(&index)?;

    println!("\n✔ Success! {} projects unregistered.", removed_count);
    Ok(())
}

/// Deletes a project from the index.
fn handle_delete(config: &ResolvedConfig, args: Vec<String>) -> Result<()> {
    let delete_children = args.iter().any(|arg| arg == "--children");
    let mut index = index_manager::load_and_ensure_global_project()?;

    let mut uuids_to_process = vec![config.uuid];
    if delete_children {
        uuids_to_process.extend(index_manager::get_all_descendants(&index, config.uuid));
    }

    println!("\n**WARNING: DESTRUCTIVE OPERATION!**");
    println!(
        "The `.axes` directories will be deleted AND the following projects will be unregistered:"
    );

    let mut paths_to_purge = Vec::new();
    for uuid in &uuids_to_process {
        if let Some(entry) = index.projects.get(uuid) {
            println!("  - {} (en {})", entry.name, entry.path.display());
            paths_to_purge.push(entry.path.join(AXES_DIR));
        }
    }

    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("ARE YOU SURE?")
        .default(false)
        .interact()?
    {
        println!("Operation cancelled.");
        return Ok(());
    }

    // 1. Purge files (we do this first, in case of failure, we don't leave the index inconsistent)
    let mut purged_count = 0;
    for path in paths_to_purge {
        if path.exists() {
            if fs::remove_dir_all(&path).is_ok() {
                purged_count += 1;
            } else {
                eprintln!("Advertencia: no se pudo purgar {}", path.display());
            }
        }
    }

    // 2. Unregister from the index (we never re-parent in a recursive delete)
    let removed_count = index_manager::remove_from_index(&mut index, &uuids_to_process, false);

    index_manager::save_global_index(&index)?;

    println!("\n✔ Success!");
    println!(
        "Se eliminaron {} directorios `.axes` y se desregistraron {} proyectos.",
        purged_count, removed_count
    );
    Ok(())
}

/// Registers an existing project in the current directory or at a specified path.
fn handle_register(path_arg: Option<String>, args: Vec<String>) -> Result<()> {
    // 1. Determine the path and flags robustly.
    // `path_arg` is the context passed to us by the dispatcher.
    // `args` are the additional arguments.

    let mut path_to_register = PathBuf::from("."); // By default, the current directory
    let mut autosolve = false;

    // Join all possible arguments into a single list for parsing.
    let mut all_args = Vec::new();
    if let Some(p) = path_arg {
        all_args.push(p);
    }
    all_args.extend(args);

    // Iterate to find the path and the flag.
    let mut path_found = false;
    for arg in all_args {
        if arg == "--autosolve" {
            autosolve = true;
        } else if !path_found {
            // The first argument that is not a flag is the path.
            path_to_register = PathBuf::from(arg);
            path_found = true;
        } else {
            // We already found a path, any other positional argument is an error.
            return Err(anyhow!(
                "Argumento inesperado '{}' para el comando 'register'.",
                arg
            ));
        }
    }

    if !path_to_register.exists() {
        return Err(anyhow!(
            "La ruta especificada no existe: {}",
            path_to_register.display()
        ));
    }

    // 2. Load the index
    let mut index = index_manager::load_and_ensure_global_project()?;

    // 3. Configure options and call the state machine
    let options = OnboardingOptions {
        autosolve,
        suggested_parent_uuid: None,
    };

    onboarding_manager::register_project(&path_to_register, &mut index, &options).context(
        format!(
            "No se pudo registrar el proyecto en '{}'.",
            path_to_register.display()
        ),
    )?;

    // 4. Save changes
    index_manager::save_global_index(&index)?;

    println!("\nRegistration operation finished.");
    Ok(())
}

fn handle_tree(config: Option<ResolvedConfig>) -> Result<()> {
    let index = index_manager::load_and_ensure_global_project()?;
    match config {
        Some(conf) => {
            println!("\nShowing tree from: '{}'", conf.qualified_name);
            let start_node = if conf.uuid == index_manager::GLOBAL_PROJECT_UUID {
                None
            } else {
                Some(conf.uuid)
            };
            graph_display::display_project_tree(&index, start_node);
        }
        None => {
            // Global Case
            graph_display::display_project_tree(&index, None);
        }
    }
    Ok(())
}

/// Manages project aliases.
fn handle_alias(subcommand: Option<String>, args: Vec<String>) -> Result<()> {
    // If there is no subcommand, the default is `list`.
    let subcommand = subcommand.as_deref().unwrap_or("list");

    let mut index = index_manager::load_and_ensure_global_project()?;

    let reserved_names = ["g", ".", "..", "*", "_", "**"];

    match subcommand {
        "set" => {
            let alias_name = args
                .first()
                .ok_or_else(|| anyhow!("El subcomando 'set' requiere un nombre de alias."))?;
            let context = args
                .get(1)
                .ok_or_else(|| anyhow!("El subcomando 'set' requiere un contexto de proyecto."))?;

            // Validate alias name
            let clean_alias_name = alias_name.strip_suffix('!').unwrap_or(alias_name);
            if reserved_names
                .iter()
                .any(|&rn| rn == clean_alias_name.to_lowercase().as_str())
            {
                return Err(anyhow!(
                    "El alias '{}' usa un nombre reservado.",
                    alias_name
                ));
            }
            if clean_alias_name.is_empty() || clean_alias_name.contains('/') {
                return Err(anyhow!("Alias name '{}' is invalid.", alias_name));
            }
            // Resolve the context to get the UUID
            let (target_uuid, target_name) = context_resolver::resolve_context(context, &index)?;

            index_manager::set_alias(&mut index, clean_alias_name.to_string(), target_uuid);
            index_manager::save_global_index(&index)?;

            println!(
                "✔ Alias '{}!' set to point to '{}'.",
                clean_alias_name, target_name
            );
        }
        "list" | "ls" => {
            if index.aliases.is_empty() {
                println!(
                    "No hay alias definidos. Usa `axes alias set <nombre> <contexto>` para crear uno."
                );
                return Ok(());
            }

            println!("Alias definidos:");
            // For a nice table, we could use `prettytable-rs`, but for now this is sufficient.
            let mut sorted_aliases: Vec<_> = index.aliases.iter().collect();
            sorted_aliases.sort_by_key(|(name, _)| *name);

            for (name, uuid) in sorted_aliases {
                let target_name = index_manager::build_qualified_name(*uuid, &index)
                    .unwrap_or_else(|| "<enlace roto>".to_string());
                println!("  {}!  ->  {}", name, target_name);
            }
        }
        "rm" | "remove" | "delete" => {
            let alias_name = args
                .first()
                .ok_or_else(|| anyhow!("Se requiere un nombre de alias para eliminar."))?;
            let clean_alias_name = alias_name.strip_suffix('!').unwrap_or(alias_name);
            if reserved_names
                .iter()
                .any(|&rn| rn == clean_alias_name.to_lowercase().as_str())
            {
                return Err(anyhow!(
                    "El alias '{}' usa un nombre reservado.",
                    alias_name
                ));
            }
            if index_manager::remove_alias(&mut index, clean_alias_name) {
                index_manager::save_global_index(&index)?;
                println!("✔ Alias '{}!' removed.", clean_alias_name);
            } else {
                return Err(anyhow!(
                    "El alias '{}!' no fue encontrado o no se puede eliminar.",
                    clean_alias_name
                ));
            }
        }
        _ => {
            return Err(anyhow!(
                "Unknown subcommand for 'alias'. Valid options: set, list, rm."
            ));
        }
    }

    Ok(())
}
