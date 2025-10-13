// build.rs

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // --- 1. Determine the language using a prioritized approach ---
    let lang: String;

    // Priority 1: Check for `lang_*` feature flags set by Cargo.
    let mut active_langs = Vec::new();
    for (key, _) in env::vars() {
        if let Some(l) = key.strip_prefix("CARGO_FEATURE_LANG_") {
            active_langs.push(l.to_lowercase());
        }
    }

    if !active_langs.is_empty() {
        // A feature flag was found. This takes highest priority.
        if active_langs.len() > 1 {
            println!(
                "cargo:warning=Multiple language features enabled ({:?}). Using the first one found: '{}'.",
                active_langs, active_langs[0]
            );
        }
        lang = active_langs[0].clone();
    } else {
        // Priority 2 & 3: No feature flags were found.
        // Fall back to the original behavior: check AXES_LANG, then default to "en".
        lang = env::var("AXES_LANG").unwrap_or_else(|_| "en".to_string());
    }

    println!("cargo:rustc-env=AXES_LANG_EFFECTIVE={}", lang);

    // --- 2. Inform Cargo about rerun triggers ---
    // This setup is robust and covers all cases.
    println!("cargo:rerun-if-env-changed=AXES_LANG");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=locales/");


    // --- 3. Load the base language file (English) as a fallback ---
    let fallback_file_path = "locales/en.toml";
    let fallback_content = fs::read_to_string(fallback_file_path)
        .expect("Failed to read fallback language file: locales/en.toml");
    let mut translations: HashMap<String, String> =
        toml::from_str(&fallback_content).expect("Failed to parse locales/en.toml");

    // --- 4. If the language is not 'en', load the specific file and merge it ---
    if lang != "en" {
        let lang_file_path = format!("locales/{}.toml", lang);
        if let Ok(content) = fs::read_to_string(&lang_file_path) {
            let specific_translations: HashMap<String, String> = toml::from_str(&content)
                .unwrap_or_else(|_| panic!("Failed to parse {}", lang_file_path));
            translations.extend(specific_translations);
        } else {
            // Warn if the language file does not exist, but continue with the fallback.
            println!(
                "cargo:warning=Language file '{}' not found. Falling back to 'en'.",
                lang_file_path
            );
        }
    }

    // --- 5. Generate the `t!` macro code ---
    let mut macro_code = String::from("#[macro_export]\nmacro_rules! t {\n");
    for (key, value) in &translations {
        // Escape problematic characters to make them valid Rust string literals.
        let escaped_value = value.replace('\\', "\\\\").replace('"', "\\\"");
        let line = format!("    (\"{}\") => {{ \"{}\" }};\n", key, escaped_value);
        macro_code.push_str(&line);
    }
    // Compile-time error branch for missing keys. This is crucial for robustness!
    macro_code.push_str(
        "    ($key:expr) => {{ compile_error!(concat!(\"Missing translation key: \", $key)) }};\n",
    );
    macro_code.push('}');

    // --- 6. Write the generated code to the `OUT_DIR` directory ---
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("translations.rs");
    fs::write(&dest_path, macro_code).unwrap();
}