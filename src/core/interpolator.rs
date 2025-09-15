// src/core/interpolator.rs

use crate::models::ResolvedConfig;
use dunce;
use std::path::PathBuf;

pub struct Interpolator<'a> {
    config: &'a ResolvedConfig,
    params: &'a [String],
    owner_root: &'a PathBuf,
}

impl<'a> Interpolator<'a> {
    pub fn new(config: &'a ResolvedConfig, params: &'a [String]) -> Self {
        Self {
            config,
            params,
            owner_root: &config.project_root,
        }
    }

    /// Interpolates a text string, replacing all known tokens
    /// in a fixed order of precedence to ensure security and predictability.
    pub fn interpolate(&self, input: &str) -> String {
        let pass1 = self.interpolate_reserved(input);
        let pass2 = self.interpolate_vars(&pass1);
        self.interpolate_params(&pass2)
    }

    /// Replaces reserved tokens and project metadata.
    fn interpolate_reserved(&self, input: &str) -> String {
        let mut result = input.to_string();

        result = result.replace("{uuid}", &self.config.uuid.to_string());
        result = result.replace("{name}", &self.config.qualified_name);

        // **NEW PATH FORMATTING LOGIC**
        // `dunce::canonicalize` does the same as `std::fs::canonicalize`
        // but on Windows it ensures a clean path without `\\?\`.
        // However, since we already have the path, we just need to format it.
        // A simple way is to use dunce to clean the path we already have.

        // The `owner_root` also needs to be cleaned.
        let owner_root_clean = dunce::simplified(self.owner_root).to_string_lossy();
        let current_path_clean = dunce::simplified(&self.config.project_root).to_string_lossy();

        result = result.replace("{root}", &owner_root_clean);
        result = result.replace("{path}", &current_path_clean);

        let version = self.config.version.as_deref().unwrap_or("");
        result = result.replace("{version}", version);

        result
    }

    /// Replaces custom tokens from the merged [vars] section.
    /// It is important that this pass runs after `interpolate_reserved`,
    /// to allow variables to depend on reserved tokens
    /// (e.g. `build_dir = "{root}/build"`).
    fn interpolate_vars(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (key, value) in &self.config.vars {
            let token = format!("{{{}}}", key);
            // We also interpolate the variable's value, in case it nests other tokens.
            let interpolated_value = self.interpolate_reserved(value);
            result = result.replace(&token, &interpolated_value);
        }
        result
    }

    /// Replaces the special `{params}` token with user-provided arguments.
    /// Runs at the end so that user input cannot interfere with
    /// the configuration tokens.
    fn interpolate_params(&self, input: &str) -> String {
        if input.contains("{params}") {
            let params_str = self.params.join(" ");
            input.replace("{params}", &params_str)
        } else {
            // If the command doesn't use {params}, we append the parameters at the end
            // for intuitive behavior.
            let mut result = input.to_string();
            if !self.params.is_empty() {
                result.push(' ');
                result.push_str(&self.params.join(" "));
            }
            result
        }
    }
}
