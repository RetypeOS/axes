//! # Commons
//!
//! This module contains common utility functions that are used throughout the application.

/// Wraps a string in quotes and escapes any internal quotes.
///
/// # Arguments
///
/// * `value` - The string to wrap in quotes.
///
/// # Returns
///
/// A new string that is wrapped in quotes, with any internal quotes escaped.
pub fn wrap_value(value: &str) -> String {
    // Escape any existing double quotes and then wrap the whole string in double quotes.
    format!("\"{}\"", value.replace('"', "\\\""))
}
