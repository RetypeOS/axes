// EN: src/cli/handlers/alias.rs

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::*;
use dialoguer::{Confirm, console::measure_text_width, theme::ColorfulTheme};
use std::env;

use crate::CancellationToken;
use crate::cli::handlers::commons::check_for_cancellation;

use crate::core::{context_resolver, index_manager};

#[derive(Parser, Debug)]
#[command(no_binary_name = true)]
struct AliasArgs {
    #[command(subcommand)]
    command: Option<AliasCommand>,
}

#[derive(Subcommand, Debug)]
enum AliasCommand {
    /// Sets a new alias or updates an existing one.
    Set {
        /// The name for the alias (without the '!' suffix).
        name: String,
        /// The project context the alias should point to.
        context: String,
    },
    /// Lists all defined aliases.
    #[command(aliases= ["ls"])]
    List,
    /// Removes an alias.
    #[command(aliases = ["rm"])]
    Remove {
        /// The name of the alias to remove.
        name: String,
    },
}

pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    if env::var("AXES_PROJECT_UUID").is_ok() {
        return Err(anyhow!(t!("alias.error.not_in_session")));
    }

    let alias_args = AliasArgs::try_parse_from(&args)?;
    let mut index = index_manager::load_and_ensure_global_project()?;

    match alias_args.command.unwrap_or(AliasCommand::List) {
        AliasCommand::Set { name, context } => {
            let clean_name = validate_alias_name(&name)?;

            // Special handling for the 'g' alias.
            if clean_name.to_lowercase() == "g" {
                println!("{}", t!("alias.warning.modifying_g").yellow().bold());
                if !Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(t!("common.prompt.are_you_sure"))
                    .default(false)
                    .interact()?
                {
                    println!("\n{}", t!("common.info.operation_cancelled"));
                    return Ok(());
                }
            }
            check_for_cancellation(cancellation_token)?;

            let (target_uuid, target_name) =
                context_resolver::resolve_context(&context, &index, cancellation_token)?;
            index_manager::set_alias(&mut index, clean_name.clone(), target_uuid);
            index_manager::save_global_index(&index)?;

            println!(
                "{} {} '{}!' -> '{}'",
                t!("common.success"),
                t!("alias.success.set"),
                clean_name,
                target_name.cyan()
            );
        }
        AliasCommand::List => {
            if index.aliases.is_empty() {
                println!("\n{}", t!("alias.info.no_aliases"));
                return Ok(());
            }

            println!("\n{}:", t!("alias.info.header"));
            let mut sorted_aliases: Vec<_> = index.aliases.iter().collect();
            sorted_aliases.sort_by_key(|(name, _)| *name);

            let max_len = sorted_aliases
                .iter()
                .map(|(name, _)| measure_text_width(&format!("{}!", name)))
                .max()
                .unwrap_or(0);

            for (name, uuid) in sorted_aliases {
                let target_name = index_manager::build_qualified_name(*uuid, &index)
                    .unwrap_or_else(|| t!("alias.info.broken_link").red().to_string());

                let alias_display_raw = format!("{}!", name);
                let alias_display_colored = format!("{}!", name.cyan());

                let visible_len = measure_text_width(&alias_display_raw);
                let padding = " ".repeat(max_len.saturating_sub(visible_len));

                println!("  {}{} ->  {}", alias_display_colored, padding, target_name);
            }
        }
        AliasCommand::Remove { name } => {
            let clean_name = validate_alias_name(&name)?;

            if clean_name.to_lowercase() == "g" {
                println!("{}", t!("alias.warning.modifying_g").yellow().bold());
                if !Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(t!("common.prompt.are_you_sure"))
                    .default(false)
                    .interact()?
                {
                    println!("\n{}", t!("common.info.operation_cancelled"));
                    return Ok(());
                }
            }
            check_for_cancellation(cancellation_token)?;

            if index_manager::remove_alias(&mut index, &clean_name) {
                index_manager::save_global_index(&index)?;
                println!(
                    "{} {}",
                    t!("common.success"),
                    format_args!(t!("alias.success.removed"), name = clean_name)
                );
            } else {
                return Err(anyhow!(t!("alias.error.not_found"), name = clean_name));
            }
        }
    }

    Ok(())
}

/// Validates an alias name against reserved keywords and syntax rules.
fn validate_alias_name(raw_name: &str) -> Result<String> {
    let name = raw_name.trim().strip_suffix('!').unwrap_or(raw_name.trim());

    if name.is_empty() {
        return Err(anyhow!(t!("alias.error.empty_name")));
    }
    if name.contains(char::is_whitespace) || name.contains('/') || name.contains('\\') {
        return Err(anyhow!(t!("alias.error.invalid_chars"), name = name));
    }

    // These are reserved for context resolution and cannot be aliases.
    let reserved_nav_names = [".", "..", "*", "**", "_"];
    if reserved_nav_names.contains(&name.to_lowercase().as_str()) {
        return Err(anyhow!(t!("alias.error.reserved_name"), name = name));
    }

    Ok(name.to_string())
}
