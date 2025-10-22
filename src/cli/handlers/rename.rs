use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::{Confirm, theme::ColorfulTheme};

use crate::{
    core::{context_resolver, index_manager},
    state::AppStateGuard,
};

// --- Command Argument Parsing ---
#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true, about = "Renames a registered project.")]
struct RenameArgs {
    /// The new name for the project.
    new_name: String,
}

// --- Main Handler ---
pub fn handle(
    context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard,
) -> Result<()> {
    // 1. Parse and validate arguments.
    let rename_args = RenameArgs::try_parse_from(&args)?;
    let new_name = crate::cli::handlers::commons::validate_project_name(&rename_args.new_name)?;
    let context_str =
        context.ok_or_else(|| anyhow!(t!("error.context_required"), command = "rename"))?;

    // 2. Resolve target and perform pre-flight checks.
    let (uuid_to_rename, old_qualified_name) =
        context_resolver::resolve_context(&context_str, state_guard)?;
    let old_simple_name = &state_guard
        .index()
        .projects
        .get(&uuid_to_rename)
        .unwrap()
        .name;

    // This function now handles no-op and global project confirmation.
    if !pre_rename_validation(uuid_to_rename, old_simple_name, &new_name)? {
        return Ok(()); // Validation failed or user cancelled, exit gracefully.
    }

    println!(
        "\n{}",
        format_args!(
            t!("rename.info.renaming"),
            old_name = old_simple_name.yellow(),
            new_name = new_name.cyan()
        )
    );

    // 3. Perform the rename operation.
    log::info!(
        "Renaming project {} ('{}') to '{}'",
        uuid_to_rename,
        old_qualified_name,
        new_name
    );
    index_manager::rename_project(state_guard.index_mut(), uuid_to_rename, &new_name)
        .with_context(|| anyhow!(t!("rename.error.rename_failed"), name = old_qualified_name))?;

    // 4. Provide clear feedback.
    let new_qualified_name =
        index_manager::build_qualified_name(uuid_to_rename, state_guard.index())
            .unwrap_or_default();

    println!("\n{}", t!("common.success").green().bold());
    println!("  {:<18} {}", "Old Full Path:".blue(), old_qualified_name);
    println!(
        "  {:<18} {}",
        "New Full Path:".blue(),
        new_qualified_name.cyan()
    );
    println!("\n  {}", t!("rename.info.caches_remain_valid").dimmed());

    Ok(())
}

fn pre_rename_validation(uuid: uuid::Uuid, old_name: &str, new_name: &str) -> Result<bool> {
    // Check for no-op renames.
    if old_name == new_name {
        println!(
            "\n{}",
            format!(t!("rename.info.no_change"), name = new_name).yellow()
        );
        return Ok(false);
    }

    // Handle special case for 'global' project.
    if uuid == index_manager::GLOBAL_PROJECT_UUID {
        return confirm_global_rename();
    }

    Ok(true)
}

fn confirm_global_rename() -> Result<bool> {
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
        return Ok(false);
    }
    Ok(true)
}
