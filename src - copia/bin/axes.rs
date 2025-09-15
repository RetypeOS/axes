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

/// El punto de entrada principal de la aplicación.
fn main() {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Esto se ejecuta en un hilo separado cuando se presiona Ctrl+C.
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        println!("\nPor favor no intente cerrar forzosamente, puede cerrar de forma segura el shell usando `exit`.");
    }).expect("Error al establecer el manejador de Ctrl-C");

    // Inicializar el logger.
    env_logger::init();

    // Parsear los argumentos de la línea de comandos.
    let cli = Cli::parse();

    // Ejecutar la lógica principal y manejar cualquier error.
    if let Err(e) = run_cli(cli) {
        // No mostrar el error si fue por una interrupción del usuario.
        if running.load(Ordering::SeqCst) {
            eprintln!("\nError: {:?}", e);
            std::process::exit(1);
        } else {
            // El error fue probablemente causado por la interrupción, así que salimos silenciosamente.
            println!("\nOperation cancelled.");
            std::process::exit(130); // Código de salida estándar para Ctrl+C
        }
    }
}

/// El despachador principal de la aplicación.
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

    // 1. Parseo Inicial
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

    // 2. Filtro de Acciones Globales
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

    // 3. Detección de Modo y Ejecución
    if let Ok(project_uuid_str) = std::env::var("AXES_PROJECT_UUID") {
        // --- MODO SESIÓN ---
        let action = arg1;
        let args = remaining_args;

        let project_uuid = Uuid::parse_str(&project_uuid_str)?;
        let index = index_manager::load_and_ensure_global_project()?;
        let qualified_name =
            index_manager::build_qualified_name(project_uuid, &index).ok_or_else(|| {
                anyhow!("Could not reconstruct project name from session.")
            })?;
        let config =
            config_resolver::resolve_config_for_uuid(project_uuid, qualified_name, &index)?;

        // Llamar al despachador de acciones de proyecto
        execute_project_action(config, action, args, SYSTEM_PROJECT_ACTIONS)?;
    } else {
        // --- MODO SCRIPT ---
        let arg2 = remaining_args.first();

        let (context_str, action_str, final_args) =
            if SYSTEM_PROJECT_ACTIONS.contains(&arg1.as_str()) {
                // Formato: `axes <acción> <contexto> [args...]`
                let context = arg2.cloned().ok_or_else(|| {
                    anyhow!("Action '{}' requires a project context.", arg1)
                })?;
                (context, arg1, remaining_args.into_iter().skip(1).collect())
            } else {
                // Formato: `axes <contexto> [acción?] [args...]`
                let context = arg1;
                let action = arg2.cloned().unwrap_or_else(|| "start".to_string());
                let args = remaining_args.into_iter().skip(1).collect();
                (context, action, args)
            };

        // `tree` sin contexto (o con `global`) es un caso especial
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

/// Ejecuta una acción que opera sobre una configuración de proyecto ya resuelta.
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
        // Atajo para `run`
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

// --- MANEJADORES DE ACCIONES (Implementaciones) ---

///Permite crear y registrar nuevos proyectos a axes.
fn handle_init(name_arg: Option<String>, args: Vec<String>) -> Result<()> {
    let project_name = name_arg
        .ok_or_else(|| anyhow!("El comando 'init' requiere un nombre para el nuevo proyecto."))?;

    // Parseo simple de argumentos para --parent
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

    // 1. Validar que no exista ya un directorio .axes en el directorio actual
    let axes_dir = current_dir.join(AXES_DIR);
    if axes_dir.exists() {
        return Err(anyhow!(
            "An '.axes' directory already exists at this location."
        ));
    }

    // 2. Cargar índice y resolver el padre (si se especificó)
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

    // 3. Añadir el nuevo proyecto al índice
    let canonical_path = current_dir.canonicalize()?;
    let (new_uuid, _) = index_manager::add_project_to_index(&mut index, project_name.clone(), canonical_path.clone(), Some(final_parent_uuid))
        .context("Could not add project to global index. There might be a sibling project with the same name.")?;

    // 4. Crear la estructura de archivos del proyecto en el disco
    fs::create_dir_all(&axes_dir)?;
    let config_path = axes_dir.join(PROJECT_CONFIG_FILENAME);
    let default_config = ProjectConfig::new();
    let toml_string = toml::to_string_pretty(&default_config)?;
    fs::write(&config_path, toml_string)?;

    // 5. Crear y guardar el archivo de referencia local (`project_ref.bin`)
    let project_ref = ProjectRef {
        self_uuid: new_uuid,
        parent_uuid: Some(final_parent_uuid), // El padre definitivo
        name: project_name.clone(),
    };
    index_manager::write_project_ref(&canonical_path, &project_ref)
        .context("No se pudo escribir el archivo de referencia del proyecto (project_ref.bin).")?;

    // 6. Guardar el índice global actualizado
    index_manager::save_global_index(&index)
        .context("Could not save updated global index.")?;

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
    // 1. Obtener el contexto del nuevo padre.
    let new_parent_context = args
        .first()
        .ok_or_else(|| anyhow!("El comando 'link' requiere el contexto del nuevo padre."))?
        .trim();

    if new_parent_context.is_empty() {
        return Err(anyhow!("New parent context cannot be empty."));
    }
    // No validamos caracteres de ruta aquí porque es un contexto, no un nombre directo.

    println!(
        "Intentando mover '{}' a ser hijo de '{}'...",
        config.qualified_name, new_parent_context
    );

    // 2. Cargar el índice global y resolver el UUID del nuevo padre.
    let mut index = index_manager::load_and_ensure_global_project()?;
    let (new_parent_uuid, new_parent_qualified_name) =
        context_resolver::resolve_context(new_parent_context, &index).context(format!(
            "No se pudo resolver el contexto del nuevo padre '{}'.",
            new_parent_context
        ))?;

    // 3. Validaciones críticas (en el `index_manager`):
    //    a. Anti-Ciclos
    //    b. Anti-Colisión de Nombres de Hermano
    index_manager::link_project(&mut index, config.uuid, new_parent_uuid).context(format!(
        "No se pudo establecer el enlace para el proyecto '{}'.",
        config.qualified_name
    ))?;

    // 4. Guardar el índice global modificado.
    index_manager::save_global_index(&index)
        .context("Could not save updated global index.")?;

    // 5. Actualizar el `project_ref.bin` local (usando `get_or_create_project_ref`)
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

/// Inicia una sesión de terminal interactiva para el proyecto.
fn handle_start(config: &ResolvedConfig) -> Result<()> {
    println!("\nStarting session for '{}'...", config.qualified_name);

    // Simplemente llamamos a nuestra nueva función.
    // Usamos `with_context` para añadir información útil al error si ocurre.
    shell::launch_interactive_shell(config).with_context(|| {
        format!(
            "Could not start session for project '{}'",
            config.qualified_name
        )
    })
}

/// Ejecuta un comando definido en el `axes.toml` del proyecto.
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

    // 1. Obtener el `Runnable` de la definición del comando.
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

            os_specific_runnable.or(pc.default.as_ref())
                .ok_or_else(|| anyhow!("Script '{}' has no implementation for the current OS and no 'default'.", script_key))?
                .clone()
        }
    };

    // 2. Ejecutar el `Runnable`.
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

                // Si cualquier paso falla, `?` detendrá la ejecución y propagará el error.
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

/// Muestra información detallada sobre la configuración resuelta del proyecto.
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
                // **CORRECCIÓN**: El match ahora extrae la struct interna `ext`.
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

/// Abre el proyecto con una aplicación configurada.
fn handle_open(config: &ResolvedConfig, args: Vec<String>) -> Result<()> {
    // 1. Determinar la clave de la acción de apertura.
    let open_key = if !args.is_empty() && args[0] == "with" {
        // Caso: `axes ... open with vsc`
        args.get(1) // Tomar el nombre de la app
            .map(|s| s.as_str())
            .ok_or_else(|| anyhow!("'open with' command requires an application name (e.g. 'vsc', 'explorer')."))?
    } else if !args.is_empty() {
        // Caso: `axes ... open vsc` (atajo)
        args[0].as_str()
    } else {
        // Caso: `axes ... open` (usar el default)
        config.options.open_with.get("default")
            .ok_or_else(|| anyhow!("No application specified and no 'default' key in [options.open_with]."))?
            .as_str()
    };

    // 2. Buscar el comando en la configuración.
    // Si la clave es "default", el usuario cometió un error, ya que "default" debe apuntar a otra clave.
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

    // 3. Interpolar y ejecutar. Por ahora, {root} y {path} son iguales.
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
    // Validar que el nuevo nombre no contenga caracteres de ruta ('/' o '\')
    if new_name.contains('/') || new_name.contains('\\') {
        return Err(anyhow!("El nuevo nombre no puede contener '/' o '\\'."));
    }
    // Validar que no sea un nombre reservado
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

    // 1. Cargar el índice global para modificarlo (operación crítica)
    let mut index = index_manager::load_and_ensure_global_project()?;

    // 2. Renombrar el proyecto en el índice en memoria (esto incluye la validación de hermanos)
    index_manager::rename_project(&mut index, config.uuid, new_name).with_context(|| {
        format!(
            "Could not rename project '{}' in global index.",
            config.qualified_name
        )
    })?;

    // 3. Guardar el índice global modificado en disco
    index_manager::save_global_index(&index)
        .context("Could not save updated global index.")?;

    // 4. Obtener y actualizar la referencia local del proyecto (project_ref.bin)
    //    Esta lógica está encapsulada en `get_or_create_project_ref` para auto-reparación.
    let mut project_ref = index_manager::get_or_create_project_ref(&config.project_root, config.uuid, &index)
        .with_context(|| format!("No se pudo obtener o crear la referencia local del proyecto `project_ref.bin` para '{}'.", config.qualified_name))?;

    // 5. Actualizar el nombre en la referencia y guardarla.
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

///Registrar proyecto existente.
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

    println!(
        "\nThe following `axes` entries will be unregistered (files will not be modified):"
    );
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

/// Elimina un proyecto del índice.
fn handle_delete(config: &ResolvedConfig, args: Vec<String>) -> Result<()> {
    let delete_children = args.iter().any(|arg| arg == "--children");
    let mut index = index_manager::load_and_ensure_global_project()?;

    let mut uuids_to_process = vec![config.uuid];
    if delete_children {
        uuids_to_process.extend(index_manager::get_all_descendants(&index, config.uuid));
    }

    println!("\n**WARNING: DESTRUCTIVE OPERATION!**");
    println!("The `.axes` directories will be deleted AND the following projects will be unregistered:");

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

    // 1. Purgar archivos (lo hacemos primero, por si falla, no dejamos el índice inconsistente)
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

    // 2. Desregistrar del índice (nunca re-parentamos en un delete recursivo)
    let removed_count = index_manager::remove_from_index(&mut index, &uuids_to_process, false);

    index_manager::save_global_index(&index)?;

    println!("\n✔ Success!");
    println!(
        "Se eliminaron {} directorios `.axes` y se desregistraron {} proyectos.",
        purged_count, removed_count
    );
    Ok(())
}

/// Registra un proyecto existente en el directorio actual o en una ruta especificada.
fn handle_register(path_arg: Option<String>, args: Vec<String>) -> Result<()> {
    // 1. Determinar la ruta y los flags de forma robusta.
    // `path_arg` es el contexto que nos pasa el despachador.
    // `args` son los argumentos adicionales.

    let mut path_to_register = PathBuf::from("."); // Por defecto, el directorio actual
    let mut autosolve = false;

    // Juntar todos los posibles argumentos en una sola lista para el parseo.
    let mut all_args = Vec::new();
    if let Some(p) = path_arg {
        all_args.push(p);
    }
    all_args.extend(args);

    // Iterar para encontrar la ruta y el flag.
    let mut path_found = false;
    for arg in all_args {
        if arg == "--autosolve" {
            autosolve = true;
        } else if !path_found {
            // El primer argumento que no es un flag es la ruta.
            path_to_register = PathBuf::from(arg);
            path_found = true;
        } else {
            // Ya encontramos una ruta, cualquier otro argumento posicional es un error.
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

    // 2. Cargar el índice
    let mut index = index_manager::load_and_ensure_global_project()?;

    // 3. Configurar opciones y llamar a la máquina de estados
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

    // 4. Guardar los cambios
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
            // Caso Global
            graph_display::display_project_tree(&index, None);
        }
    }
    Ok(())
}

/// Gestiona los alias de proyectos.
fn handle_alias(subcommand: Option<String>, args: Vec<String>) -> Result<()> {
    // Si no hay subcomando, el default es `list`.
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

            // Validar nombre del alias
            let clean_alias_name = alias_name.strip_suffix('!').unwrap_or(alias_name);
            if reserved_names.iter().any(|&rn| rn == clean_alias_name.to_lowercase().as_str()) {
                return Err(anyhow!("El alias '{}' usa un nombre reservado.", alias_name));
            }
            if clean_alias_name.is_empty() || clean_alias_name.contains('/') {
                return Err(anyhow!("Alias name '{}' is invalid.", alias_name));
            }
            // Resolver el contexto para obtener el UUID
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
            // Para una tabla bonita, podríamos usar `prettytable-rs`, pero por ahora esto es suficiente.
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
            if reserved_names.iter().any(|&rn| rn == clean_alias_name.to_lowercase().as_str()) {
                return Err(anyhow!("El alias '{}' usa un nombre reservado.", alias_name));
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
