//! # Command-Line Interface (CLI) Module
//!
//! This module serves as the main entry point for parsing and defining the application's
//! command-line interface. It uses the `clap` crate to define the top-level `Cli` struct
//! and orchestrates the custom help message generation.
//!
//! ## Modules
//!
//! - **`dispatcher`**: Contains the core logic for parsing the application's "universal grammar"
//!   and routing commands to the appropriate handlers.
//! - **`handlers`**: A collection of sub-modules, each responsible for the logic of a specific
//!   `axes` command (e.g., `run`, `init`, `tree`).
//!
//! ## Custom Help Message
//!
//! This module is responsible for building a dynamic, color-aware, and richly formatted
//! help message. The `build_help_string()` function replaces simple placeholders in a
//! template with ANSI color codes, providing a more user-friendly and readable output than
//! the default `clap` help screen.

use clap::Parser;

pub mod dispatcher;
pub mod handlers;

/// Builds the dynamic, color-aware full help string at runtime.
///
/// This function acts as a mini-renderer for our semantic help template, which is defined
/// in the localization files (e.g., `locales/en.toml`). It replaces placeholders like
/// `<title>` and `<cmd>` with the appropriate ANSI escape codes for color and style, but
/// only if color output is enabled for the terminal. This ensures a beautiful help
/// message on supported terminals and a clean, readable one on others.
fn build_help_string() -> &'static str {
    // This function acts as a mini-renderer for our semantic help template.
    // It replaces placeholders like `<title>` with colored/styled text.

    let use_colors = colored::control::SHOULD_COLORIZE.should_colorize();

    let template = t!("cli.help.template");

    // Define styles. If colors are disabled, they are empty strings.
    let title = if use_colors { "\x1b[1;33m" } else { "" }; // Bold Yellow
    let hl = if use_colors { "\x1b[1;36m" } else { "" }; // Bold Cyan (for highlights)
    let hi = if use_colors { "\x1b[1;20m" } else { "" }; // Bold Cyan (for highlights)
    let cmd = if use_colors { "\x1b[36m" } else { "" }; // Cyan (for commands)
    let group = if use_colors { "\x1b[1;32m" } else { "" }; // Bold Green
    let err = if use_colors { "\x1b[91m" } else { "" }; // Bright Red (for destructive)
    let dim = if use_colors { "\x1b[2m" } else { "" }; // Dim
    let reset = if use_colors { "\x1b[0m" } else { "" };

    // Perform replacements in a single, chained expression.
    let formatted_string = template
        .replace("<title>", title)
        .replace("</title>", reset)
        .replace("<hl>", hl)
        .replace("</hl>", reset)
        .replace("<hi>", hi)
        .replace("</hi>", reset)
        .replace("<cmd>", cmd)
        .replace("</cmd>", reset)
        .replace("<group>", group)
        .replace("</group>", reset)
        .replace("<err>", err)
        .replace("</err>", reset)
        .replace("<dim>", dim)
        .replace("</dim>", reset);

    Box::leak(formatted_string.into_boxed_str())
}

/// The root of the command-line interface, defined using `clap`.
///
/// This struct captures all arguments passed to `axes` into a single `Vec<String>`.
/// It intentionally avoids defining subcommands at this top level. Instead, the raw
/// arguments are passed to the `dispatcher` module, which implements a more flexible,
/// universal grammar for command parsing. This approach allows for context-sensitive
/// commands and implicit actions (like `axes my-script` being a shortcut for `axes . run my-script`).
#[derive(Parser, Debug)]
#[command(
    //author,
    version,
    about,
    // Use `help_template` to take full control of the output.
    help_template = { build_help_string() },
    // Custom styles for auto-generated parts (not used now, but good to keep for future)
    styles = clap::builder::Styles::styled()
        .header(clap::builder::styling::AnsiColor::Yellow.on_default().bold())
        .usage(clap::builder::styling::AnsiColor::Yellow.on_default().bold())
        .literal(clap::builder::styling::AnsiColor::Cyan.on_default().bold())
        .placeholder(clap::builder::styling::AnsiColor::Green.on_default()),
)]
#[command(disable_help_subcommand = true)]
#[command(trailing_var_arg = true)]
pub struct Cli {
    /// A catch-all for the entire sequence of arguments passed to `axes`.
    /// This vector is passed directly to the dispatcher for parsing according to the
    /// application's universal grammar.
    #[arg()]
    pub args: Vec<String>,
}
