// src/cli/handlers/rename.rs

use anyhow::{Context, Result, anyhow};
use colored::*;
use dialoguer::{Confirm, theme::ColorfulTheme};

use super::commons;
use crate::{core::index_manager, models::GlobalIndex};

use clap::Parser;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct RenameArgs {
    /// The new name for the project.
    new_name: String,
}

/// The main handler for the `rename` command.
pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. Parse args.
    let rename_args = RenameArgs::try_parse_from(&args)?;
    let context_str =
        context.ok_or_else(|| anyhow!(t!("error.context_required"), command = "delete"))?;

    // 2. Solve config.
    let mut index = index_manager::load_and_ensure_global_project()?;
    let config = commons::resolve_config_and_update_index_if_needed(Some(context_str), &mut index)?;
    let old_qualified_name = config.qualified_name.clone();

    let simple_name = config
        .qualified_name
        .split('/')
        .next_back()
        .unwrap_or(&config.qualified_name);

    let new_name = commons::validate_project_name(&rename_args.new_name)?;

    if config.uuid == index_manager::GLOBAL_PROJECT_UUID {
        println!(
            "{}",
            t!("rename.warning.renaming_global_header").yellow().bold()
        );
        println!("  - {}", t!("rename.warning.renaming_global_docs"));
        println!("  - {}", t!("rename.warning.renaming_global_community"));

        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(t!("common.prompt.are_you_sure"))
            .default(false)
            .interact()?
        {
            println!("\n{}", t!("common.info.operation_cancelled"));
            return Ok(());
        }
    }

    println!(
        t!("rename.info.renaming"),
        old_name = simple_name.yellow(),
        new_name = new_name.cyan()
    );

    index_manager::rename_project(&mut index, config.uuid, &new_name)
        .with_context(|| anyhow!(t!("rename.error.rename_failed"), name = old_qualified_name))?;

    index_manager::save_global_index(&index).with_context(|| t!("error.saving_global_index"))?;

    let mut project_ref =
        index_manager::get_or_create_project_ref(&config.project_root, config.uuid, &index)
            .with_context(|| t!("error.local_ref_failed"))?;

    project_ref.name = new_name.clone();
    if let Err(e) = index_manager::write_project_ref(&config.project_root, &project_ref) {
        eprintln!(
            "\n{}",
            format!(
                t!("rename.warning.local_ref_update_failed"),
                path = config.project_root.display(),
                error = e
            )
            .yellow()
        );
    }

    println!("\n{}", t!("common.success"));
    println!(
        "  {}",
        format_args!(
            t!("rename.success.header"),
            old_name = simple_name,
            new_name = new_name
        )
    );
    println!("  {}", t!("common.info.caches_will_regenerate").dimmed());

    Ok(())
}
