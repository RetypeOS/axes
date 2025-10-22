//! # Color
//!
//! This module provides utilities for parsing and converting ANSI color styles.

use crate::models::AnsiStyle;
use anyhow::{anyhow, Result};

/// Parses a style name string (e.g., "red", "bold", "bright-green") into an `AnsiStyle` enum.
/// The parsing is case-insensitive.
///
/// # Returns
///
/// A `Result` containing the `AnsiStyle` on success, or an error if the style name is unknown.
pub fn parse_style_name(name: &str) -> Result<AnsiStyle> {
    match name.to_lowercase().replace('_', "-").as_str() {
        // Attributes
        "reset" => Ok(AnsiStyle::Reset),
        "bold" => Ok(AnsiStyle::Bold),
        "dim" => Ok(AnsiStyle::Dim),
        "italic" => Ok(AnsiStyle::Italic),
        "underline" => Ok(AnsiStyle::Underline),

        // Standard Colors
        "black" => Ok(AnsiStyle::Black),
        "red" => Ok(AnsiStyle::Red),
        "green" => Ok(AnsiStyle::Green),
        "yellow" => Ok(AnsiStyle::Yellow),
        "blue" => Ok(AnsiStyle::Blue),
        "magenta" => Ok(AnsiStyle::Magenta),
        "cyan" => Ok(AnsiStyle::Cyan),
        "white" => Ok(AnsiStyle::White),

        // Bright Colors (with multiple name variants for user convenience)
        "bright-black" | "gray" | "grey" => Ok(AnsiStyle::BrightBlack),
        "bright-red" => Ok(AnsiStyle::BrightRed),
        "bright-green" => Ok(AnsiStyle::BrightGreen),
        "bright-yellow" => Ok(AnsiStyle::BrightYellow),
        "bright-blue" => Ok(AnsiStyle::BrightBlue),
        "bright-magenta" => Ok(AnsiStyle::BrightMagenta),
        "bright-cyan" => Ok(AnsiStyle::BrightCyan),
        "bright-white" => Ok(AnsiStyle::BrightWhite),

        // Handle legacy `AnsiStyle` enum name in error messages if we rename it.
        _ => Err(anyhow!("Unknown style token: '<#{}>'", name)),
    }
}

/// Converts an `AnsiStyle` enum into its raw ANSI escape code representation.
///
/// # Returns
///
/// A `&'static str` containing the ANSI escape code for the given style.
pub fn style_to_ansi_code(style: AnsiStyle) -> &'static str {
    match style {
        // Attributes
        AnsiStyle::Reset => "\x1b[0m",
        AnsiStyle::Bold => "\x1b[1m",
        AnsiStyle::Dim => "\x1b[2m",
        AnsiStyle::Italic => "\x1b[3m",
        AnsiStyle::Underline => "\x1b[4m",

        // Standard Colors
        AnsiStyle::Black => "\x1b[30m",
        AnsiStyle::Red => "\x1b[31m",
        AnsiStyle::Green => "\x1b[32m",
        AnsiStyle::Yellow => "\x1b[33m",
        AnsiStyle::Blue => "\x1b[34m",
        AnsiStyle::Magenta => "\x1b[35m",
        AnsiStyle::Cyan => "\x1b[36m",
        AnsiStyle::White => "\x1b[37m",

        // Bright (Intense) Colors
        AnsiStyle::BrightBlack => "\x1b[90m",
        AnsiStyle::BrightRed => "\x1b[91m",
        AnsiStyle::BrightGreen => "\x1b[92m",
        AnsiStyle::BrightYellow => "\x1b[93m",
        AnsiStyle::BrightBlue => "\x1b[94m",
        AnsiStyle::BrightMagenta => "\x1b[95m",
        AnsiStyle::BrightCyan => "\x1b[96m",
        AnsiStyle::BrightWhite => "\x1b[97m",
    }
}
