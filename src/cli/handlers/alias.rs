//! # Handler for the `alias` command
//!
//! This module provides the logic for managing project shortcuts (aliases), allowing users
//! to create, list, remove, and check aliases that point to specific project contexts.
//!
//! ## Core Logic
//!
//! - **Subcommand Dispatch**: The main `handle` function parses the arguments into subcommands
//!   (`set`, `list`, `remove`, `check`) and calls the appropriate function.
//! - **State Management**: It correctly distinguishes between read operations (`list`, `check`)
//!   which use `.index()`, and write operations (`set`, `remove`) which use `.index_mut()`
//!   to ensure the application state is marked as dirty only when necessary.
//! - **Context Resolution**: The `set` command utilizes the `context_resolver` to find the
//!   target project's UUID, ensuring that aliases always point to valid, registered projects.
//! - **User Experience**: Includes user-friendly features like confirmation prompts for potentially
//!   sensitive operations (overwriting an alias, modifying the special 'g' alias) and formatted,
//!   sorted output for lists.

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
///
/// It dispatches to sub-handlers for set, list, remove, and check operations. It also
/// ensures that alias commands cannot be run from within an active project session.
///
/// # Arguments
/// * `_context` - The context from the dispatcher, which is ignored by this global command.
/// * `args` - The command-specific arguments (e.g., `set my-alias .`).
/// * `state_guard` - A mutable guard to the application state.
pub fn handle(
    _context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard<'_>,
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
///
/// This is a write operation. It resolves the target context to a UUID and then
/// inserts or updates the alias in the `GlobalIndex`. It needs the full `AppStateGuard`
/// because it calls `context_resolver`, which may need to update `last_used` metadata.
///
/// # Arguments
/// * `name` - The name for the new alias.
/// * `context` - The context string the alias should point to.
/// * `state_guard` - A mutable guard to the application state.
fn set_alias(name: &str, context: &str, state_guard: &mut AppStateGuard<'_>) -> Result<()> {
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
///
/// This is a read-only operation. It iterates over the aliases in the `GlobalIndex`
/// and prints them in a sorted, aligned format.
///
/// # Arguments
/// * `index` - An immutable reference to the `GlobalIndex`.
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
///
/// This is a write operation that removes an entry from the aliases map in the `GlobalIndex`.
///
/// # Arguments
/// * `name` - The name of the alias to remove.
/// * `index` - A mutable reference to the `GlobalIndex`.
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
///
/// This is a read-only operation. It iterates through all defined aliases and verifies
/// that their target UUIDs still correspond to existing projects in the index, reporting
/// any broken links.
///
/// # Arguments
/// * `index` - An immutable reference to the `GlobalIndex`.
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

/// A helper function to validate an alias name against reserved keywords and syntax rules.
/// It also conveniently strips the optional `!` suffix from user input.
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

/// A shared helper to ask for user confirmation if they are attempting to modify the
/// special 'g' alias, which is conventionally used for the global project.
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
    Ok(true)
}
