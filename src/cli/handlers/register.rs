// src/cli/handlers/rename.rs

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use colored::*;
use dialoguer::{Confirm, theme::ColorfulTheme};

use crate::{
    core::{context_resolver, index_manager},
    models::GlobalIndex,
};

// --- Command Argument Parsing ---

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true, about = "Renames a registered project.")]
struct RenameArgs {
    /// The new name for the project.
    new_name: String,
}

// --- Main Handler ---

pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. Parse arguments and validate the new name.
    let rename_args = RenameArgs::try_parse_from(&args)?;
    let new_name = crate::cli::handlers::commons::validate_project_name(&rename_args.new_name)?;

    // 2. Resolve the target project's UUID and current name directly from the index.
    // This is much faster as it avoids any filesystem I/O for config files.
    let context_str =
        context.ok_or_else(|| anyhow!(t!("error.context_required"), command = "rename"))?;

    let (uuid_to_rename, old_qualified_name) =
        context_resolver::resolve_context(&context_str, index)?;
    let old_simple_name = old_qualified_name
        .split('/')
        .last()
        .unwrap_or(&old_qualified_name);

    // 3. Handle the special case of renaming the 'global' project.
    if uuid_to_rename == index_manager::GLOBAL_PROJECT_UUID {
        if !confirm_global_rename()? {
            return Ok(());
        }
    }

    println!(
        "\n{}",
        format_args!(
            t!("rename.info.renaming"),
            old_name = old_simple_name.yellow(),
            new_name = new_name.cyan()
        )
    );

    // 4. Perform the rename operation directly on the index.
    index_manager::rename_project(index, uuid_to_rename, &new_name)
        .with_context(|| anyhow!(t!("rename.error.rename_failed"), name = old_qualified_name))?;

    // 5. Update the local project reference file (`project_ref.bin`).
    // We need to get the project's root path from the index entry.
    let project_entry = index.projects.get(&uuid_to_rename).unwrap(); // Safe
    let mut project_ref =
        index_manager::get_or_create_project_ref(&project_entry.path, uuid_to_rename, index)?;

    project_ref.name = new_name.clone();
    if let Err(e) = index_manager::write_project_ref(&project_entry.path, &project_ref) {
        eprintln!(
            "\n{}",
            format!(
                t!("rename.warning.local_ref_update_failed"),
                path = project_entry.path.display(),
                error = e
            )
            .yellow()
        );
    }

    // CRITICAL: `main` will handle saving the updated global index.

    // 6. Provide clear feedback, including the new qualified name.
    let new_qualified_name =
        index_manager::build_qualified_name(uuid_to_rename, index).unwrap_or_default();

    println!("\n{}", t!("common.success").green().bold());
    println!("  {:<18} {}", "Old Full Path:".blue(), old_qualified_name);
    println!(
        "  {:<18} {}",
        "New Full Path:".blue(),
        new_qualified_name.cyan()
    );

    // [OPTIMIZATION] Acknowledge that caches remain valid.
    println!("\n  {}", t!("rename.info.caches_remain_valid").dimmed());

    Ok(())
}

// --- Helper Functions ---

/// [DRY] Handles the warning and confirmation flow for renaming the 'global' project.
/// Returns `Ok(false)` if the user cancels the operation.
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
