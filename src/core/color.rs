// src/core/color.rs (NEW MODULE)

use crate::models::AnsiColor;
use anyhow::{Result, anyhow};

/// Parses a color name string (e.g., "red", "green") into an `AnsiColor` enum.
pub fn parse_color_name(name: &str) -> Result<AnsiColor> {
    match name.to_lowercase().as_str() {
        "reset" => Ok(AnsiColor::Reset),
        "black" => Ok(AnsiColor::Black),
        "red" => Ok(AnsiColor::Red),
        "green" => Ok(AnsiColor::Green),
        "yellow" => Ok(AnsiColor::Yellow),
        "blue" => Ok(AnsiColor::Blue),
        "magenta" => Ok(AnsiColor::Magenta),
        "cyan" => Ok(AnsiColor::Cyan),
        "white" => Ok(AnsiColor::White),
        _ => Err(anyhow!("Unknown color token: '<#{}>'", name)),
    }
}

/// Converts an `AnsiColor` enum into its raw ANSI escape code representation.
pub fn ansi_color_to_code(color: AnsiColor) -> &'static str {
    match color {
        AnsiColor::Reset => "\x1b[0m",
        AnsiColor::Black => "\x1b[30m",
        AnsiColor::Red => "\x1b[31m",
        AnsiColor::Green => "\x1b[32m",
        AnsiColor::Yellow => "\x1b[33m",
        AnsiColor::Blue => "\x1b[34m",
        AnsiColor::Magenta => "\x1b[35m",
        AnsiColor::Cyan => "\x1b[36m",
        AnsiColor::White => "\x1b[37m",
    }
}
