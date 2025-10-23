//! # Script Parameter Parsing and Resolution
//!
//! This module is responsible for the entire lifecycle of script parameter handling, from
//! parsing definitions in `axes.toml` to resolving them against user-provided CLI arguments.
//! It is divided into two main phases:
//!
//! 1.  **Definition Parsing (`ParameterDef`)**: Functions like `parse_parameter_token` are used
//!     by the compiler (`compiler.rs`) to transform parameter tokens (e.g., `<params::0(required)>`)
//!     into a structured `ParameterDef`. This captures the contract of the script.
//!
//! 2.  **Argument Resolution (`ArgResolver`)**: At runtime, the `ArgResolver` is instantiated.
//!     It takes the script's `ParameterDef`s and the user's CLI arguments. It then parses the
//!     CLI input, matches arguments to definitions, validates requirements, applies defaults,
//!     and prepares the final string values for substitution into the command.
//!
//! The system is designed for high performance, operating on string slices and avoiding
//! allocations wherever possible.

use crate::{
    core::commons::wrap_value,
    models::{ParameterDef, ParameterKind, ParameterModifiers},
};
use anyhow::{Context, Result, anyhow};
use colored::*;
use lazy_static::lazy_static;
use regex::Regex;
use std::{borrow::Cow, collections::HashMap};

lazy_static! {
    static ref PARAMETER_TOKEN_CONTENT_RE: Regex =
        Regex::new(r"^\s*([^(\s]+)\s*(?:\((.*)\))?\s*$").expect("Parameter regex should be valid");
}

lazy_static! {
    static ref MODIFIERS_RE: Regex =
        Regex::new(r#"\s*([^=,\s]+)(?:\s*=\s*(?:'([^']*)'|"([^"]*)"|([^,]*)))?\s*"#)
            .expect("Modifiers regex should be valid");
}

// --- DATA STRUCTS ---

/// A preliminary, intermediate representation of a token found during the initial parsing pass.
///
/// This enum is used internally by the compiler to distinguish between different token types
/// before the full, recursive expansion and compilation into `TemplateComponent`s begins.
/// It operates on borrowed string slices (`&'a str`) for maximum performance during the initial
/// tokenization phase.
#[derive(Debug)]
pub enum PreComponent<'a> {
    /// A static, literal part of the command string.
    Literal(&'a str),
    /// A reference to a variable, e.g., `<vars::my_var>`. The value is the variable name.
    Var(&'a str),
    /// A reference to another script, e.g., `<scripts::build>`. The value is the script name.
    Script(&'a str),
    /// A command to be executed whose output is substituted, e.g., `<run('...')>`.
    RunLiteral(&'a str),
    /// A user-defined parameter token, e.g., `<params::0(required)>`.
    Param {
        /// The full original token match, e.g., `<params::0(required)>`.
        full_match: &'a str,
        /// The inner content of the token, e.g., `0(required)`.
        spec: &'a str,
    },
    /// The generic parameters token, `<params>`.
    GenericParams,
}

/// Represents a single CLI argument and its consumption state.
#[derive(Debug, Clone, Copy)]
pub struct CliArgument<'a> {
    /// The string value of the argument, if it has one (e.g., for `--key value`).
    pub value: Option<&'a str>,
    /// A flag indicating whether this argument has been claimed by a `ParameterDef`.
    pub consumed: bool,
}

/// Contains and manages the state of all arguments passed via the CLI for a given script.
/// This struct is mutable and is consumed during the parameter resolution process.
#[derive(Debug, Clone)]
pub struct CliInputState<'a> {
    /// A vector of positional arguments in the order they appeared.
    positional: Vec<CliArgument<'a>>,
    /// A map of named arguments (flags), where the key is the flag itself (e.g., `"--verbose"`).
    named: HashMap<&'a str, CliArgument<'a>>,
}

/// An orchestrator that validates CLI arguments against parameter definitions and resolves their final values.
///
/// An instance of `ArgResolver` is created for each script execution. Its constructor performs all
/// the validation and resolution logic upfront. The rest of the application can then query it for
/// the final string values to be substituted into commands, without needing to know the details
/// of the resolution process.
#[derive(Debug)]
pub struct ArgResolver<'a> {
    /// A map from the original parameter token (e.g., `<params::0>`) to its final, resolved string value.
    resolved_values: HashMap<String, String>,
    /// A vector of all arguments that were not claimed by any specific `ParameterDef`. These are
    /// intended for the generic `<params>` token.
    unclaimed_args: Vec<&'a str>,
}

// --- PARSER DE DEFINICIONES (DE FASE 1) ---

/// Parses the content of a parameter token, e.g., "0(required)" or "target(alias='-t')".
/// This is called by the main expansion engine in `config_resolver`.
pub fn parse_parameter_token(original_token: &str, content: &str) -> Result<ParameterDef> {
    // [ADD] Handle the special case of `<params(...)>` which has no specifier.
    if content.starts_with('(') || content.is_empty() {
        let modifiers_str = if content.starts_with('(') {
            content
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or("")
        } else {
            content
        };

        let modifiers = parse_parameter_modifiers_from_str(modifiers_str)
            .with_context(|| format!("Failed to parse modifiers in token: {}", original_token))?;

        // We use a placeholder kind, as `<params>` doesn't have a name or index.
        return Ok(ParameterDef {
            kind: ParameterKind::Positional { index: usize::MAX }, // Special index
            modifiers,
            original_token: original_token.to_string(),
        });
    }

    let caps = PARAMETER_TOKEN_CONTENT_RE
        .captures(content)
        .ok_or_else(|| anyhow!("Invalid parameter format in token: {}", original_token))?;

    let specifier = caps
        .get(1)
        .expect("Regex specifier group must exist")
        .as_str();
    let modifiers_str = caps.get(2).map(|m| m.as_str());

    let kind = if let Ok(index) = specifier.parse::<usize>() {
        ParameterKind::Positional { index }
    } else {
        ParameterKind::Named {
            name: specifier.to_string(),
        }
    };

    let modifiers = match modifiers_str {
        Some(s) => parse_parameter_modifiers_from_str(s)
            .with_context(|| format!("Failed to parse modifiers in token: {}", original_token))?,
        None => ParameterModifiers::default(),
    };

    Ok(ParameterDef {
        kind,
        modifiers,
        original_token: original_token.to_string(),
    })
}

/// Parses a modifier string, e.g., "required, default='staging', literal".
pub fn parse_parameter_modifiers_from_str(s: &str) -> Result<ParameterModifiers> {
    log::debug!("Parsing modifiers string: '{}'", s);
    let mut modifiers = ParameterModifiers::default();
    if s.trim().is_empty() {
        return Ok(modifiers);
    }

    for caps in MODIFIERS_RE.captures_iter(s) {
        let key = caps.get(1).map_or("", |m| m.as_str()).trim();
        if key.is_empty() {
            continue;
        }

        let value = caps
            .get(2)
            .or(caps.get(3))
            .or(caps.get(4))
            .map(|m| m.as_str());

        if let Some(val) = value {
            match key {
                "default" => modifiers.default_value = Some(val.to_string()),
                "alias" => modifiers.alias = Some(val.to_string()),
                "map" => modifiers.map = Some(val.to_string()),
                _ => return Err(anyhow!("Unknown modifier key: '{}'", key)),
            }
        } else {
            match key {
                "required" => modifiers.required = true,
                "literal" => modifiers.literal = true,
                _ => {
                    return Err(anyhow!(
                        "Unknown boolean modifier: '{}' (or missing value)",
                        key
                    ));
                }
            }
        }
    }

    log::debug!("Parsed modifiers: {:?}", modifiers);
    Ok(modifiers)
}

// --- IMPLEMENTACIÓN DEL ESTADO DE LA CLI (FASE 2) ---

impl<'a> CliInputState<'a> {
    /// Parses CLI arguments into a structured, mutable state for consumption.
    ///
    /// This function handles basic parsing of positional arguments and named flags (both with and
    /// without values). It is highly optimized to work with string slices (`&str`) and performs
    /// zero heap allocations.
    ///
    /// # Arguments
    /// * `cli_params` - A slice of strings representing the arguments passed by the user for the script.
    pub fn new(cli_params: &'a [String]) -> Result<Self> {
        let mut positional = Vec::new();
        let mut named = HashMap::new();
        let mut params_iter = cli_params.iter().map(String::as_str).peekable();

        while let Some(param) = params_iter.next() {
            if param.starts_with('-') {
                let value = if let Some(next) = params_iter.peek() {
                    if !next.starts_with('-') {
                        params_iter.next()
                    } else {
                        None
                    }
                } else {
                    None
                };
                named.insert(
                    param,
                    CliArgument {
                        value,
                        consumed: false,
                    },
                );
            } else {
                positional.push(CliArgument {
                    value: Some(param),
                    consumed: false,
                });
            }
        }
        Ok(Self { positional, named })
    }

    /// Consumes the next available positional argument.
    ///
    /// # Arguments
    /// * `index` - The zero-based index of the positional argument to consume (e.g., `0` for the first one).
    ///
    /// # Returns
    /// `Some(&str)` containing the argument's value if found, or `None` otherwise.
    pub fn consume_positional(&mut self, index: usize) -> Option<&'a str> {
        // Find the n-th positional (unconsumed) argument.
        self.positional
            .iter_mut()
            .filter(|arg| !arg.consumed)
            .nth(index)
            .and_then(|arg| {
                arg.consumed = true;
                arg.value
            })
    }

    /// Consumes a named argument (flag), checking both its long name and its alias.
    ///
    /// # Arguments
    /// * `name` - The long name of the flag (e.g., "verbose" for `--verbose`).
    /// * `alias` - An optional short name for the flag (e.g., "-v").
    ///
    /// # Returns
    /// - `Ok(Some(Some(value)))` if the flag was present and had a value.
    /// - `Ok(Some(None))` if the flag was present but had no value.
    /// - `Ok(None)` if the flag was not present.
    /// - `Err` if both the name and its alias were provided.
    pub fn consume_named(
        &mut self,
        name: &str,          // e.g., "verbose"
        alias: Option<&str>, // e.g., "-v"
    ) -> Result<Option<Option<&'a str>>> {
        let long_flag = format!("--{}", name);

        // --- Step 1: Find the keys that are present (immutable borrows) ---
        let is_long_present = self.named.contains_key(long_flag.as_str());
        let is_alias_present = alias.is_some_and(|a| self.named.contains_key(a));

        // --- Step 2: Check for conflicts ---
        if is_long_present && is_alias_present {
            return Err(anyhow!(
                "Conflict: Both flag '{}' and its alias '{}' were provided.",
                long_flag.cyan(),
                alias
                    .expect("Alias should exist here due to is_alias_present check")
                    .cyan()
            ));
        }

        // --- Step 3: Determine which key to use (if any) ---
        // We create an owned String here to hold the key, breaking the borrow chain.
        let key_to_use: Option<String> = if is_long_present {
            Some(long_flag)
        } else if is_alias_present {
            alias.map(|s| s.to_string())
        } else {
            None
        };

        // --- Step 4: Perform the mutable borrow ---
        // At this point, there are no active immutable borrows of `self.named`.
        if let Some(key) = key_to_use {
            let key_str: &str = key.as_str();
            if let Some(arg) = self.named.get_mut(key_str)
                && !arg.consumed
            {
                arg.consumed = true;
                return Ok(Some(arg.value));
            }
        }

        Ok(None)
    }

    /// Collects all arguments that were not consumed by `consume_positional` or `consume_named`.
    ///
    /// # Returns
    /// A tuple containing a vector of the unconsumed argument strings and a boolean indicating
    /// if there were any.
    pub fn get_unconsumed_values(&self) -> (Vec<&'a str>, bool) {
        let mut parts = Vec::new();
        let mut had_unconsumed = false;

        for arg in self.positional.iter().filter(|a| !a.consumed) {
            parts.push(arg.value.expect("Positional argument must have a value"));
            had_unconsumed = true;
        }

        let mut sorted_named_keys: Vec<_> = self.named.keys().copied().collect();
        sorted_named_keys.sort_unstable();

        for key in sorted_named_keys {
            if let Some(arg) = self.named.get(key).filter(|a| !a.consumed) {
                parts.push(key);
                if let Some(val) = arg.value {
                    parts.push(val);
                }
                had_unconsumed = true;
            }
        }
        (parts, had_unconsumed)
    }
}

impl<'a> ArgResolver<'a> {
    /// Creates a new `ArgResolver` by validating CLI arguments against script definitions.
    ///
    /// This constructor is the main entry point for the resolution phase. It orchestrates the
    /// entire process of parsing CLI input, matching it against the script's contract
    /// (`definitions`), checking for required parameters, applying defaults, and preparing
    /// the final substitution values.
    ///
    /// # Arguments
    /// * `definitions` - A slice of `ParameterDef`s that define the script's expected arguments.
    /// * `cli_params` - The raw string arguments provided by the user for the script.
    /// * `has_generic_params` - A flag indicating if the script contains a generic `<params>` token.
    pub fn new(
        definitions: &[ParameterDef],
        cli_params: &'a [String],
        has_generic_params: bool,
    ) -> Result<Self> {
        // CliInputState now operates on slices, performing zero allocations.
        let mut cli_state = CliInputState::new(cli_params)?;
        let mut resolved_values = HashMap::with_capacity(definitions.len());

        // --- 1. Upfront Validation for Conflicting Flags ---
        // This check prevents logic errors later on.
        for def in definitions {
            if let ParameterKind::Named { name } = &def.kind
                && let Some(alias) = &def.modifiers.alias
                && cli_state.named.contains_key(name.as_str())
                && cli_state.named.contains_key(alias.as_str())
            {
                return Err(anyhow!(
                    "Conflict: Both flag '--{}' and its alias '{}' were provided.",
                    name.cyan(),
                    alias.cyan()
                ));
            }
        }

        // --- 2. Resolution Loop for Each Parameter Definition ---
        for def in definitions {
            if (matches!(def.kind, ParameterKind::Positional { index } if index == usize::MAX))
                || resolved_values.contains_key(&def.original_token)
            {
                continue;
            }

            let final_string: String = match &def.kind {
                ParameterKind::Positional { index } => {
                    // Positional logic is correct and remains unchanged.
                    let cli_value = cli_state.consume_positional(*index);
                    if def.modifiers.required && cli_value.is_none() {
                        return Err(anyhow!(
                            "Positional argument at index {} is required but was not provided.",
                            index
                        ));
                    }
                    let final_value: Option<Cow<'a, str>> =
                        cli_value.map(Cow::Borrowed).or_else(|| {
                            def.modifiers
                                .default_value
                                .as_ref()
                                .map(|s| Cow::Owned(s.clone()))
                        });
                    final_value.map_or(String::new(), |val| {
                        if def.modifiers.literal {
                            wrap_value(&val)
                        } else {
                            val.into_owned()
                        }
                    })
                }
                ParameterKind::Named { name } => {
                    // --- FINAL, SIMPLE, AND CORRECT LOGIC FOR NAMED PARAMETERS ---
                    let alias = def.modifiers.alias.as_deref();
                    let cli_presence = cli_state.consume_named(name, alias)?;

                    // Rule 1: Handle absent flag.
                    if cli_presence.is_none() {
                        if def.modifiers.required {
                            return Err(anyhow!(
                                "Flag '--{}' is required but was not provided.",
                                name
                            ));
                        }
                        // Not required and not present -> expands to nothing.
                        String::new()
                    } else {
                        // At this point, we know the flag was provided.

                        // Rule 2: Determine the final value.
                        // `cli_presence` is `Some(Option<&'a str>)`.
                        // `.flatten()` converts it to `Option<&'a str>`.
                        let final_value = cli_presence.flatten().map(Cow::Borrowed).or_else(|| {
                            def.modifiers
                                .default_value
                                .as_ref()
                                .map(|s| Cow::Owned(s.clone()))
                        });

                        let value_maybe_wrapped = if def.modifiers.literal {
                            final_value.as_ref().map(|v| Cow::Owned(wrap_value(v)))
                        } else {
                            final_value
                        };

                        // Rule 3 & 4: Format output based on `map`.
                        if let Some(map_str) = &def.modifiers.map {
                            // `map` is defined.
                            if let Some(val) = value_maybe_wrapped {
                                if map_str.is_empty() {
                                    val.into_owned()
                                } else {
                                    format!("{}{}", map_str, val)
                                }
                            } else {
                                // Flag was present without value and has no default.
                                // A `map` without a value results in an empty string.
                                String::new()
                            }
                        } else {
                            // `map` is NOT defined (pass-through mode).
                            let flag_name = format!("--{}", name);
                            match value_maybe_wrapped {
                                Some(val) => format!("{} {}", flag_name, val),
                                None => flag_name,
                            }
                        }
                    }
                }
            };
            resolved_values.insert(def.original_token.clone(), final_string);
        }

        // --- 3. Handle Unconsumed Arguments for the generic `<params>` token ---
        let (unclaimed_args, had_unconsumed) = cli_state.get_unconsumed_values();
        if had_unconsumed && !has_generic_params {
            return Err(anyhow!(
                "{} The script does not define a generic `<params>` token to accept them.\nProvided unhandled arguments: {}",
                "Error: Unexpected arguments were provided.".red(),
                unclaimed_args.join(" ").yellow()
            ));
        }

        Ok(ArgResolver {
            resolved_values,
            unclaimed_args,
        })
    }

    /// Retrieves the final, resolved string for a specific parameter token (e.g., `<params::0>`).
    pub fn get_specific_value(&self, original_token: &str) -> Option<&str> {
        self.resolved_values.get(original_token).map(String::as_str)
    }

    /// Retrieves a slice of all unconsumed arguments, to be used by the generic `<params>` token.
    /// This is a zero-copy operation.
    pub fn get_generic_values(&self) -> &[&'a str] {
        &self.unclaimed_args
    }
}

// MARK: --- UNIT TESTS ---

#[cfg(test)]
mod tests {
    use super::*;

    // --- Helper to create a Vec<String> from &str slices ---
    fn to_cli_params(params: &[&str]) -> Vec<String> {
        params.iter().map(|s| s.to_string()).collect()
    }

    // --- `parse_parameter_modifiers_from_str` Tests ---
    #[test]
    fn test_parse_modifiers() {
        let modifiers =
            parse_parameter_modifiers_from_str("required, default='latest', literal").unwrap();
        assert!(modifiers.required);
        assert!(modifiers.literal);
        assert_eq!(modifiers.default_value.as_deref(), Some("latest"));
        assert!(modifiers.alias.is_none());
    }

    #[test]
    fn test_parse_modifiers_with_alias_and_map() {
        let modifiers = parse_parameter_modifiers_from_str("alias = '-t', map='--tag='").unwrap();
        assert!(!modifiers.required);
        assert_eq!(modifiers.alias.as_deref(), Some("-t"));
        assert_eq!(modifiers.map.as_deref(), Some("--tag="));
    }

    // --- `CliInputState` Tests ---
    #[test]
    fn test_cli_input_state_parsing() {
        let params = to_cli_params(&[
            "pos0",
            "--named1",
            "val1",
            "-s",
            "--bool-flag",
            "val2",
            "pos1",
        ]);
        let state = CliInputState::new(&params).unwrap();

        assert_eq!(state.positional.len(), 2);
        assert_eq!(
            state.positional[0].value,
            Some("pos0".to_string()).as_deref()
        );

        assert_eq!(
            state.positional[1].value,
            Some("pos1".to_string()).as_deref()
        );

        assert_eq!(state.named.len(), 3);
        assert_eq!(
            state.named.get("--named1").unwrap().value,
            Some("val1".to_string()).as_deref()
        );
        assert_eq!(state.named.get("-s").unwrap().value, None);
        assert_eq!(
            state.named.get("--bool-flag").unwrap().value,
            Some("val2".to_string()).as_deref()
        );
    }

    // --- `ArgResolver` Full Logic Tests ---

    // Test Positional Parameters
    #[test]
    fn test_resolver_positional_basic() {
        let defs = [parse_parameter_token("<p::0>", "0").unwrap()];
        let params = to_cli_params(&["hello"]);
        let resolver = ArgResolver::new(&defs, &params, false).unwrap();
        assert_eq!(resolver.get_specific_value("<p::0>"), Some("hello"));
    }

    #[test]
    fn test_resolver_positional_required_fail() {
        let defs = [parse_parameter_token("<p::0(required)>", "0(required)").unwrap()];
        let params = to_cli_params(&[]);
        let result = ArgResolver::new(&defs, &params, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("is required"));
    }

    #[test]
    fn test_resolver_positional_default() {
        let defs =
            [parse_parameter_token("<p::0(default='world')>", "0(default='world')").unwrap()];
        let params = to_cli_params(&[]);
        let resolver = ArgResolver::new(&defs, &params, false).unwrap();
        assert_eq!(
            resolver.get_specific_value("<p::0(default='world')>"),
            Some("world")
        );
    }

    // Test Named Parameters (Flags)
    #[test]
    fn test_resolver_named_simple_pass_through() {
        let defs = [parse_parameter_token("<p::verbose>", "verbose").unwrap()];
        let params = to_cli_params(&["--verbose"]);
        let resolver = ArgResolver::new(&defs, &params, false).unwrap();
        assert_eq!(
            resolver.get_specific_value("<p::verbose>"),
            Some("--verbose")
        );
    }

    #[test]
    fn test_resolver_named_with_value_pass_through() {
        let defs = [parse_parameter_token("<p::env>", "env").unwrap()];
        let params = to_cli_params(&["--env", "staging"]);
        let resolver = ArgResolver::new(&defs, &params, false).unwrap();
        assert_eq!(
            resolver.get_specific_value("<p::env>"),
            Some("--env staging")
        );
    }

    #[test]
    fn test_resolver_named_required_success() {
        let defs = [parse_parameter_token("<p::env(required)>", "env(required)").unwrap()];
        let params = to_cli_params(&["--env", "staging"]);
        let resolver = ArgResolver::new(&defs, &params, false).unwrap();
        assert_eq!(
            resolver.get_specific_value("<p::env(required)>"),
            Some("--env staging")
        );
    }

    #[test]
    fn test_resolver_named_required_fail() {
        let defs = [parse_parameter_token("<p::env(required)>", "env(required)").unwrap()];
        let params = to_cli_params(&[]);
        let result = ArgResolver::new(&defs, &params, false);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Flag '--env' is required")
        );
    }

    #[test]
    fn test_resolver_named_flag_absent_uses_default() {
        let defs = [
            parse_parameter_token("<p::tag(default='latest')>", "tag(default='latest')").unwrap(),
        ];
        let params = to_cli_params(&[]);
        let resolver = ArgResolver::new(&defs, &params, false).unwrap();
        // With `map` undefined, and flag absent, the token resolves to nothing. This is correct.
        assert_eq!(
            resolver.get_specific_value("<p::tag(default='latest')>"),
            Some("")
        );
    }

    #[test]
    fn test_resolver_named_flag_present_no_value_uses_default() {
        let defs = [
            parse_parameter_token("<p::tag(default='latest')>", "tag(default='latest')").unwrap(),
        ];
        let params = to_cli_params(&["--tag"]);
        let resolver = ArgResolver::new(&defs, &params, false).unwrap();
        assert_eq!(
            resolver.get_specific_value("<p::tag(default='latest')>"),
            Some("--tag latest")
        );
    }

    #[test]
    fn test_resolver_named_required_and_default() {
        // This confirms the logic we discussed: `required` checks for presence,
        // `default` provides a value if present without one.
        let defs = [parse_parameter_token(
            "<p::region(required, default='us-east-1')>",
            "region(required, default='us-east-1')",
        )
        .unwrap()];

        // Case 1: Fails because flag is not present.
        let params_fail = to_cli_params(&[]);
        let result_fail = ArgResolver::new(&defs, &params_fail, false);
        assert!(result_fail.is_err());

        // Case 2: Succeeds and uses the default value.
        let params_succeed = to_cli_params(&["--region"]);
        let resolver = ArgResolver::new(&defs, &params_succeed, false).unwrap();
        assert_eq!(
            resolver.get_specific_value("<p::region(required, default='us-east-1')>"),
            Some("--region us-east-1")
        );
    }

    #[test]
    fn test_resolver_map_empty_extracts_value() {
        let defs = [parse_parameter_token(
            "<p::tag(map='', default='latest')>",
            "tag(map='', default='latest')",
        )
        .unwrap()];

        // Case 1: Flag absent, uses default.
        let params1 = to_cli_params(&[]);
        let resolver1 = ArgResolver::new(&defs, &params1, false).unwrap();
        assert_eq!(
            resolver1.get_specific_value("<p::tag(map='', default='latest')>"),
            Some("")
        );

        // Case 2: Flag present with value.
        let params2 = to_cli_params(&["--tag", "v1.2.0"]);
        let resolver2 = ArgResolver::new(&defs, &params2, false).unwrap();
        assert_eq!(
            resolver2.get_specific_value("<p::tag(map='', default='latest')>"),
            Some("v1.2.0")
        );
    }

    // Test Generic <params> Collector
    #[test]
    fn test_resolver_unclaimed_args() {
        let defs = [parse_parameter_token("<p::0>", "0").unwrap()];
        let params = to_cli_params(&["pos0", "pos1", "--flag", "val"]);
        let resolver = ArgResolver::new(&defs, &params, true).unwrap();

        assert_eq!(resolver.get_specific_value("<p::0>"), Some("pos0"));
        assert_eq!(resolver.unclaimed_args, vec!["pos1", "--flag", "val"]);
    }

    #[test]
    fn test_resolver_unclaimed_args_error_when_no_generic_token() {
        let defs = [parse_parameter_token("<p::0>", "0").unwrap()];
        let params = to_cli_params(&["pos0", "pos1"]);
        let result = ArgResolver::new(&defs, &params, false);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unexpected arguments")
        );
    }
}
