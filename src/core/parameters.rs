// src/core/parameters.rs

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
        Regex::new(r"^\s*([^(\s]+)\s*(?:\((.*)\))?\s*$").unwrap();
}

lazy_static! {
    static ref MODIFIERS_RE: Regex =
        Regex::new(r#"\s*([^=,\s]+)(?:\s*=\s*(?:'([^']*)'|"([^"]*)"|([^,]*)))?\s*"#).unwrap();
}

// --- DATA STRUCTS ---

/// A preliminary, intermediate representation of a token found during the initial parsing pass.
/// This distinguishes between different token types before the recursive expansion begins.
#[derive(Debug)]
pub enum PreComponent<'a> {
    Literal(&'a str),
    Var(&'a str),
    Script(&'a str),
    RunScript(&'a str),
    RunLiteral(&'a str),
    Param { full_match: &'a str, spec: &'a str },
    GenericParams,
}

/// Representa un único argumento de la CLI con su estado de consumo.
#[derive(Debug, Clone, Copy)]
pub struct CliArgument<'a> {
    pub value: Option<&'a str>,
    pub consumed: bool,
}

/// Contiene y gestiona el estado de todos los argumentos pasados por la CLI.
/// Esta estructura es mutable y se modifica durante el proceso de resolución.
#[derive(Debug, Clone)]
pub struct CliInputState<'a> {
    positional: Vec<CliArgument<'a>>,
    named: HashMap<&'a str, CliArgument<'a>>,
}

/// El orquestador que valida y resuelve los parámetros.
#[derive(Debug)]
pub struct ArgResolver<'a> {
    /// Mapa del token original a su valor final resuelto.
    resolved_values: HashMap<String, String>,
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

    let specifier = caps.get(1).unwrap().as_str();
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
    /// Parses a raw string slice and builds the initial state without cloning any strings.
    pub fn new(cli_params: &'a [String]) -> Result<Self> {
        let mut positional = Vec::new();
        let mut named = HashMap::new();
        let mut params_iter = cli_params.iter().map(String::as_str).peekable();

        while let Some(param) = params_iter.next() {
            if let Some(name) = param.strip_prefix("--") {
                let value = if let Some(next_param) = params_iter.peek() {
                    if !next_param.starts_with('-') {
                        Some(params_iter.next().unwrap())
                    } else {
                        None
                    }
                } else {
                    None
                };
                named.insert(
                    name,
                    CliArgument {
                        value,
                        consumed: false,
                    },
                );
            } else if param.starts_with('-') {
                let value = if let Some(next_param) = params_iter.peek() {
                    if !next_param.starts_with('-') {
                        Some(params_iter.next().unwrap())
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

    /// Tries to consume a positional argument by its index, returning a `&str`.
    pub fn consume_positional(&mut self, index: usize) -> Option<&'a str> {
        if let Some(arg) = self.positional.get_mut(index)
            && !arg.consumed
        {
            arg.consumed = true;
            return arg.value;
        }
        None
    }

    /// Tries to consume a named argument, considering its alias.
    pub fn consume_named(
        &mut self,
        name: &str,
        alias: Option<&str>,
    ) -> Result<Option<Option<&'a str>>> {
        let name_present = self.named.contains_key(name);
        let alias_key = alias.and_then(|a| self.named.keys().find(|&&k| k == a));

        if name_present && alias_key.is_some() {
            return Err(anyhow!(
                "Conflict: Both flag '{}' and its alias '{}' were provided.",
                format!("--{}", name).cyan(),
                format!("-{}", alias.unwrap()).cyan()
            ));
        }

        let key_to_use = if name_present {
            Some(name)
        } else {
            alias_key.copied()
        };

        if let Some(key) = key_to_use
            && let Some(arg) = self.named.get_mut(key)
            && !arg.consumed
        {
            arg.consumed = true;
            return Ok(Some(arg.value));
        }
        Ok(None)
    }

    /// Collects all unconsumed arguments into a Vec<&'a str>.
    pub fn get_unconsumed_values(&self) -> (Vec<&'a str>, bool) {
        let mut parts = Vec::new();
        let mut had_unconsumed = false;

        for arg in self.positional.iter().filter(|a| !a.consumed) {
            parts.push(arg.value.unwrap());
            had_unconsumed = true;
        }

        let mut sorted_named_keys: Vec<_> = self.named.keys().copied().collect();
        sorted_named_keys.sort();

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
    /// The constructor validates the entire user input against the script's contract (definitions)
    /// and pre-calculates the final string value for every parameter token.
    ///
    /// # Performance
    /// This implementation is heavily optimized to avoid string allocations. It operates on string
    /// slices (`&'a str`) borrowed from the original command-line arguments. It only allocates new
    /// strings when a default value is used, or when formatting is required (e.g., for `map` or literal wrapping).
    /// The `Cow<'a, str>` enum is used extensively to handle values that can be either borrowed or owned.
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
                    // This logic is already correct, no changes needed.
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
                    // --- REBUILT LOGIC FOR NAMED PARAMETERS ---
                    let alias = def.modifiers.alias.as_deref();
                    let cli_presence = cli_state.consume_named(name, alias)?; // Option<Option<&'a str>>

                    // 1. Check `required` constraint. This is now correct.
                    if def.modifiers.required && cli_presence.is_none() {
                        return Err(anyhow!(
                            "Flag '--{}' is required but was not provided.",
                            name
                        ));
                    }

                    // 2. Determine the base value.
                    // `cli_presence` has three states:
                    //   - Some(Some(val)): Flag with value (`--flag val`).
                    //   - Some(None): Flag without value (`--flag`).
                    //   - None: Flag not present.
                    let base_value: Option<Cow<'a, str>> = match cli_presence {
                        Some(Some(val)) => Some(Cow::Borrowed(val)),
                        Some(None) => def
                            .modifiers
                            .default_value
                            .as_ref()
                            .map(|s| Cow::Owned(s.clone())),
                        None => def
                            .modifiers
                            .default_value
                            .as_ref()
                            .map(|s| Cow::Owned(s.clone())),
                    };

                    // 3. Format the output based on the value and modifiers.
                    if let Some(map_str) = &def.modifiers.map {
                        // `map` is defined: The presence of the flag is irrelevant, only the value matters.
                        base_value.map_or(String::new(), |val| {
                            let val_maybe_wrapped = if def.modifiers.literal {
                                Cow::Owned(wrap_value(&val))
                            } else {
                                val
                            };
                            if map_str.is_empty() {
                                val_maybe_wrapped.into_owned()
                            } else {
                                format!("{}{}", map_str, val_maybe_wrapped)
                            }
                        })
                    } else {
                        // `map` is NOT defined (pass-through mode). Output depends on flag presence.
                        if cli_presence.is_some() {
                            let flag_name = format!("--{}", name);
                            match base_value {
                                Some(val) => {
                                    let val_maybe_wrapped = if def.modifiers.literal {
                                        Cow::Owned(wrap_value(&val))
                                    } else {
                                        val
                                    };
                                    format!("{} {}", flag_name, val_maybe_wrapped)
                                }
                                None => flag_name, // Flag present, but no value from CLI or default.
                            }
                        } else {
                            // Flag was not present, and no `map`. The token resolves to nothing.
                            String::new()
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
        let params = to_cli_params(&["pos0", "--named1", "val1", "-s", "--bool-flag", "pos1"]);
        let state = CliInputState::new(&params).unwrap();

        assert_eq!(state.positional.len(), 1);
        assert_eq!(
            state.positional[0].value,
            Some("pos0".to_string()).as_deref()
        );

        assert_eq!(state.named.len(), 3);
        assert_eq!(
            state.named.get("named1").unwrap().value,
            Some("val1".to_string()).as_deref()
        );
        assert_eq!(state.named.get("-s").unwrap().value, None);
        assert_eq!(state.named.get("bool-flag").unwrap().value, None);
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
            Some("latest")
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
        let result = ArgResolver::new(&defs, &params, false); // has_generic_params is false
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unexpected arguments")
        );
    }
}
