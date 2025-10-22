use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use colored::*;
use dialoguer::{Confirm, console::measure_text_width, theme::ColorfulTheme};
use std::env;

use crate::{
    core::{context_resolver, index_manager},
    models::GlobalIndex,
    state::AppStateGuard,
};

// --- Command Argument Parsing ---

#[derive(Parser, Debug)]
#[command(no_binary_name = true, about = "Manage project shortcuts (aliases).")]
struct AliasArgs {
    #[command(subcommand)]
    command: Option<AliasCommand>,
}

#[derive(Subcommand, Debug)]
enum AliasCommand {
    /// Sets a new alias or updates an existing one.
    Set {
        /// The name for the alias (e.g., 'backend').
        name: String,
        /// The project context the alias should point to (e.g., 'my-app/api', 'g!', '.').
        context: String,
    },
    /// Lists all defined aliases.
    #[command(name = "list", aliases = ["ls"])]
    List,
    /// Removes an alias.
    #[command(name = "remove", aliases = ["rm"])]
    Remove {
        /// The name of the alias to remove.
        name: String,
    },
    /// Verifies all aliases, reporting any broken links.
    Check,
}

// --- Main Handler ---

/// The main handler for the `alias` command.
/// It dispatches to sub-handlers for set, list, remove, and check operations.
pub fn handle(
    _context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard,
) -> Result<()> {
    // Aliases are a global concept and cannot be managed from within a project session.
    if env::var("AXES_PROJECT_UUID").is_ok() {
        return Err(anyhow!(t!("alias.error.not_in_session")));
    }
    // The `alias` command itself is global and does not depend on a context argument.
    // The `_context` parameter is ignored.

    let alias_args = AliasArgs::try_parse_from(&args)?;

    match alias_args.command.unwrap_or(AliasCommand::List) {
        AliasCommand::Set { name, context } => set_alias(&name, &context, state_guard),
        AliasCommand::List => list_aliases(state_guard.index()),
        AliasCommand::Remove { name } => remove_alias(&name, state_guard.index_mut()),
        AliasCommand::Check => check_aliases(state_guard.index()),
    }
}

// --- Subcommand Logic ---

/// Handles the logic for creating or updating an alias.
fn set_alias(name: &str, context: &str, state_guard: &mut AppStateGuard) -> Result<()> {
    let clean_name = validate_alias_name(name)?;

    // Proceed only if user confirms modifying the special 'g' alias.
    if !confirm_g_alias_modification(&clean_name)? {
        return Ok(());
    }

    // Check if the alias already exists to provide better user feedback.
    let is_update = state_guard.index().aliases.contains_key(&clean_name);
    if is_update {
        let old_uuid = state_guard.index().aliases.get(&clean_name).unwrap(); // Safe to unwrap here.
        let old_target = index_manager::build_qualified_name(*old_uuid, state_guard.index())
            .unwrap_or_else(|| t!("alias.info.broken_link").red().to_string());
        println!(
            "{}",
            format!(
                t!("alias.warning.overwriting"),
                name = clean_name,
                old_target = old_target
            )
            .yellow()
        );
    }

    let (target_uuid, target_name) = context_resolver::resolve_context(context, state_guard)
        .with_context(|| {
            format!(
                "The provided context '{}' for the alias could not be resolved.",
                context
            )
        })?;
    index_manager::set_alias(state_guard.index_mut(), clean_name.clone(), target_uuid);

    println!(
        "{} {} '{}!' -> '{}'",
        t!("common.success"),
        if is_update {
            t!("alias.success.updated")
        } else {
            t!("alias.success.set")
        },
        clean_name,
        target_name.cyan()
    );
    Ok(())
}

/// Handles the logic for listing all aliases in a formatted table.
fn list_aliases(index: &GlobalIndex) -> Result<()> {
    if index.aliases.is_empty() {
        println!("\n{}", t!("alias.info.no_aliases"));
        return Ok(());
    }

    println!("\n{}:", t!("alias.info.header"));

    // Collect references, not owned values. Avoids cloning Strings and Uuids.
    let mut sorted_aliases: Vec<_> = index.aliases.iter().collect();
    sorted_aliases.sort_by_key(|(name, _)| *name);

    let max_len = sorted_aliases
        .iter()
        .map(|(name, _)| measure_text_width(&format!("{}!", name)))
        .max()
        .unwrap_or(0);

    for (name, uuid) in sorted_aliases {
        let target_name = index_manager::build_qualified_name(*uuid, index)
            .unwrap_or_else(|| t!("alias.info.broken_link").red().to_string());

        let alias_display_raw = format!("{}!", name);
        let alias_display_colored = format!("{}!", name.cyan());

        let visible_len = measure_text_width(&alias_display_raw);
        let padding = " ".repeat(max_len.saturating_sub(visible_len));

        println!("  {}{} ->  {}", alias_display_colored, padding, target_name);
    }
    Ok(())
}

/// Handles the logic for removing an alias.
fn remove_alias(name: &str, index: &mut GlobalIndex) -> Result<()> {
    let clean_name = validate_alias_name(name)?;

    if !confirm_g_alias_modification(&clean_name)? {
        return Ok(());
    }

    if index_manager::remove_alias(index, &clean_name) {
        // CRITICAL: Do NOT save the index here.
        println!(
            "{} {}",
            t!("common.success"),
            format_args!(t!("alias.success.removed"), name = clean_name)
        );
    } else {
        return Err(anyhow!(t!("alias.error.not_found"), name = clean_name));
    }
    Ok(())
}

/// Handles the logic for checking the health of all aliases.
fn check_aliases(index: &GlobalIndex) -> Result<()> {
    if index.aliases.is_empty() {
        println!("\n{}", t!("alias.info.no_aliases"));
        return Ok(());
    }

    println!("\n{}", t!("alias.info.checking_header"));
    let mut broken_count = 0;

    let mut sorted_aliases: Vec<_> = index.aliases.iter().collect();
    sorted_aliases.sort_by_key(|(name, _)| *name);

    for (name, uuid) in sorted_aliases {
        match index_manager::build_qualified_name(*uuid, index) {
            Some(target_name) => {
                println!("  {} {}! -> {}", "✔".green(), name.cyan(), target_name);
            }
            None => {
                broken_count += 1;
                println!(
                    "  {} {}! -> {}",
                    "✖".red(),
                    name.cyan(),
                    t!("alias.info.broken_link").red()
                );
            }
        }
    }

    println!("---");
    if broken_count == 0 {
        println!("{} {}", t!("common.success"), t!("alias.success.all_ok"));
    } else {
        println!(
            "{}",
            format!(t!("alias.warning.found_broken"), count = broken_count).yellow()
        );
    }

    Ok(())
}

// --- Helper Functions ---

/// Validates an alias name against reserved keywords and syntax rules.
fn validate_alias_name(raw_name: &str) -> Result<String> {
    // Allow user to conveniently type the alias with or without the '!' suffix.
    let name = raw_name.trim().strip_suffix('!').unwrap_or(raw_name.trim());

    if name.is_empty() {
        return Err(anyhow!(t!("alias.error.empty_name")));
    }
    if name.contains(char::is_whitespace) || name.contains('/') || name.contains('\\') {
        return Err(anyhow!(t!("alias.error.invalid_chars"), name = name));
    }

    // These are reserved for context resolution and cannot be used for aliases.
    let reserved_nav_names = [".", "..", "*", "**", "_"];
    if reserved_nav_names.contains(&name.to_lowercase().as_str()) {
        return Err(anyhow!(t!("alias.error.reserved_name"), name = name));
    }

    Ok(name.to_string())
}

/// [DRY] Asks for confirmation if the user is trying to modify the special 'g' alias.
/// Returns `Ok(false)` if the operation should be cancelled.
fn confirm_g_alias_modification(name: &str) -> Result<bool> {
    if name.to_lowercase() == "g" {
        println!("{}", t!("alias.warning.modifying_g").yellow().bold());
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(t!("common.prompt.are_you_sure"))
            .default(false)
            .interact()?
        {
            println!("\n{}", t!("common.info.operation_cancelled"));
            return Ok(false); // User cancelled the operation.
        }
    }
    Ok(true) // Proceed with the operation.
}
