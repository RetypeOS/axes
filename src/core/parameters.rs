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
            // Skip generic <params> definition and already resolved tokens.
            if (matches!(def.kind, ParameterKind::Positional { index } if index == usize::MAX))
                || resolved_values.contains_key(&def.original_token)
            {
                continue;
            }

            let final_string: String = match &def.kind {
                ParameterKind::Positional { index } => {
                    let cli_value = cli_state.consume_positional(*index);
                    if def.modifiers.required && cli_value.is_none() {
                        return Err(anyhow!(
                            "Positional argument at index {} is required but was not provided.",
                            index
                        ));
                    }

                    // Use Cow to represent a value that is either borrowed from the CLI or owned by the default.
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
                    let cli_value_opt =
                        cli_state.consume_named(name, def.modifiers.alias.as_deref())?;
                    if def.modifiers.required && cli_value_opt.is_none() {
                        return Err(anyhow!(
                            "Flag '--{}' is required but was not provided.",
                            name
                        ));
                    }

                    if let Some(cli_value) = cli_value_opt {
                        let final_value: Option<Cow<'a, str>> =
                            cli_value.map(Cow::Borrowed).or_else(|| {
                                def.modifiers
                                    .default_value
                                    .as_ref()
                                    .map(|s| Cow::Owned(s.clone()))
                            });

                        let final_value_maybe_wrapped = if def.modifiers.literal {
                            final_value.as_ref().map(|v| Cow::Owned(wrap_value(v)))
                        } else {
                            final_value
                        };

                        if let Some(map_str) = &def.modifiers.map {
                            if map_str.is_empty() {
                                final_value_maybe_wrapped
                                    .unwrap_or(Cow::Borrowed(""))
                                    .into_owned()
                            } else {
                                format!(
                                    "{}{}",
                                    map_str,
                                    final_value_maybe_wrapped.unwrap_or(Cow::Borrowed(""))
                                )
                            }
                        } else {
                            let flag_name = format!("--{}", name);
                            match final_value_maybe_wrapped {
                                Some(val) => format!("{} {}", flag_name, val),
                                None => flag_name,
                            }
                        }
                    } else {
                        String::new() // Flag was not provided, resolves to an empty string.
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
