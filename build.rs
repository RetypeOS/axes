//! # Application State Management
//!
//! This module provides a high-performance, journaling-based state management system for the application.
//! It is designed to minimize expensive clone and write operations by intelligently tracking changes
//! to the `GlobalIndex`.
//!
//! ## Key Components
//!
//! - **`AppState`**: The core struct that holds the state, transitioning from `Pristine` to `Dirty`
//!   on the first mutation.
//! - **`AppStateGuard`**: A custom `MutexGuard` that provides a safe and explicit API for accessing
//!   and modifying the state via `.index()` and `.index_mut()` methods.
//! - **`get_app_state()` & `lock_app_state()`**: Singleton accessors to the global state, ensuring that
//!   all interactions are thread-safe and centralized.
//!
//! ## Design Philosophy
//!
//! The state is loaded once at the beginning of the application's lifecycle. A snapshot of the
//! original state is taken only when a mutable operation is first requested (a "copy-on-write"
//! strategy). This ensures that read-only commands are extremely fast, as they never trigger
//! a clone.
//!
//! For mutable operations, intelligent `update_*` methods on the `AppStateGuard` perform
//! read-only checks to determine if a write is truly necessary before triggering the copy,
//! further optimizing performance for idempotent updates. At the end of the run, the application
//! saves the state back to disk only if a deep comparison shows that actual changes have occurred.

#![allow(clippy::panic, clippy::unwrap_used)]

use std::collections::HashMap;
use std::env;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{self, Write as IoWrite};
use std::path::Path;

/// Defines the languages supported by the application.
///
/// By defining this list explicitly, we make the build script much faster and more
/// deterministic, as we no longer need to scan all environment variables.
/// To add a new language, add its code here and create the corresponding `locales/xx.toml` file.
const SUPPORTED_LANGS: &[&str] = &["en", "es"];

/// A simple error handler for the build script.
/// Panicking is an acceptable way to fail a build script, but this function
/// provides a cleaner, more informative error message than a raw `unwrap()` or `expect()`.
fn exit_with_error<T: std::fmt::Display>(message: T) -> ! {
    // Flush stdout to ensure cargo messages are seen before the panic message.
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
    panic!("Build script failed: {}", message);
}

fn main() {
    // --- 1. Set up rerun triggers ---
    // This tells Cargo when to re-run this build script.
    println!("cargo:rerun-if-env-changed=AXES_LANG");
    println!("cargo:rerun-if-changed=build.rs");
    // Rerun if any file in the locales directory changes.
    println!("cargo:rerun-if-changed=locales/");

    // --- 2. Determine the effective language and load translations ---
    let lang = determine_language();
    println!("cargo:rustc-env=AXES_LANG_EFFECTIVE={}", lang);
    let translations = load_translations(&lang);

    // --- 3. Generate the macro file from the loaded translations ---
    generate_macro_file(&translations);
}

/// Determines the language to use based on a clear priority order.
/// 1. Cargo feature flags (e.g., `lang_es`).
/// 2. `AXES_LANG` environment variable.
/// 3. Default to "en".
fn determine_language() -> String {
    // Priority 1: Check for `CARGO_FEATURE_LANG_*` environment variables.
    // This is much more efficient than iterating through `env::vars()`.
    let mut active_langs = Vec::new();
    for code in SUPPORTED_LANGS {
        let feature_name = format!("CARGO_FEATURE_LANG_{}", code.to_uppercase());
        if env::var(feature_name).is_ok() {
            active_langs.push(code.to_string());
        }
    }

    if let Some(first_lang) = active_langs.first() {
        if active_langs.len() > 1 {
            println!(
                "cargo:warning=Multiple language features enabled ({:?}). Using the first one found: '{}'.",
                active_langs, first_lang
            );
        }
        return first_lang.clone();
    }

    // Priority 2 & 3: Fall back to environment variable or default.
    env::var("AXES_LANG").unwrap_or_else(|_| "en".to_string())
}

/// Loads the translation key-value pairs from the appropriate TOML files.
/// It always loads the fallback language (`en`) first, then merges the specific
/// language on top, ensuring all keys are present.
fn load_translations(lang: &str) -> HashMap<String, String> {
    let fallback_path = "locales/en.toml";
    let fallback_content = fs::read_to_string(fallback_path)
        .expect("Build failed: Could not read fallback language file 'locales/en.toml'");
    let mut translations: HashMap<String, String> =
        toml::from_str(&fallback_content).expect("Build failed: Could not parse 'locales/en.toml'");

    if lang != "en" {
        let lang_file_path = format!("locales/{}.toml", lang);
        match fs::read_to_string(&lang_file_path) {
            Ok(content) => {
                let specific_translations: HashMap<String, String> = toml::from_str(&content)
                    .unwrap_or_else(|e| {
                        exit_with_error(format!(
                            "Failed to parse TOML from '{}': {}",
                            lang_file_path, e
                        ))
                    });
                // Merge the specific language translations, overwriting fallback keys.
                translations.extend(specific_translations);
            }
            Err(_) => {
                println!(
                    "cargo:warning=Language file '{}' not found. Falling back to 'en'.",
                    lang_file_path
                );
            }
        }
    }
    translations
}

/// Generates the `translations.rs` file containing the `t!` macro.
/// The macro provides compile-time checks for translation keys.
fn generate_macro_file(translations: &HashMap<String, String>) {
    let out_dir = env::var("OUT_DIR").unwrap_or_else(|_| {
        exit_with_error("The OUT_DIR environment variable was not set by Cargo.")
    });
    let dest_path = Path::new(&out_dir).join("translations.rs");

    // Use a String as a buffer for efficiency.
    let mut macro_code = String::with_capacity(translations.len() * 50); // Pre-allocate memory

    // Generate auto-documentation to respect clippy necesary data.
    writeln!(
        macro_code,
        "/// # Translation Macro (`t!`)\n/// \n\
         /// This file is auto-generated by `build.rs`. Do not edit it manually.\n/// \n\
         /// It contains the `t!` macro, which provides a compile-time safe mechanism\n\
         /// for accessing localized strings. If a key is not found, a compile-time\n\
         /// error is generated, preventing missing translations from reaching production."
    )
    .unwrap();

    // Start of the macro definition
    writeln!(
        macro_code,
        "#[macro_export]\n\
         #[allow(clippy::wildcard_imports, macro_use_extern_crate)]\n\
         macro_rules! t {{"
    )
    .expect("Failed to write to string buffer");

    // --- Deterministic Ordering ---
    // Sort the keys to ensure the generated file is identical across builds.
    // This prevents unnecessary rebuilds and makes version control diffs clean.
    let mut sorted_keys: Vec<_> = translations.keys().collect();
    sorted_keys.sort_unstable();

    for key in sorted_keys {
        let value = translations.get(key).unwrap();
        // --- Robust String Escaping ---
        // Using the `Debug` formatter (`:?`) on a string automatically handles all
        // necessary escaping (e.g., `\`, `"`, `\n`) to produce a valid Rust string literal.
        // This is far more robust than manual `.replace()` calls.
        writeln!(macro_code, "    (\"{}\") => {{ {:?} }};", key, value).unwrap();
    }

    // Add a compile-time error branch for any key that is not found.
    // This is a critical feature for robustness, catching typos in keys at compile time.
    writeln!(
        macro_code,
        "    ($key:expr) => {{ compile_error!(concat!(\"Missing translation key: \", $key)) }};"
    )
    .unwrap();
    writeln!(macro_code, "}}").unwrap();

    // Write the generated code to the destination file.
    fs::write(&dest_path, macro_code).unwrap_or_else(|e| {
        exit_with_error(format!(
            "Failed to write generated code to '{:?}': {}",
            dest_path, e
        ))
    });
}
