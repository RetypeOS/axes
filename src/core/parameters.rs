// src/core/parameters.rs

use crate::models::{ParameterDef, ParameterKind, ParameterModifiers};
use anyhow::{Context, Result, anyhow};
use colored::*;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;

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
#[derive(Debug, Clone)]
pub struct CliArgument {
    /// Some(v) para `--k v` y posicionales.
    /// None para flags booleanos como `--k`.
    pub value: Option<String>,
    pub consumed: bool,
}

/// Contiene y gestiona el estado de todos los argumentos pasados por la CLI.
/// Esta estructura es mutable y se modifica durante el proceso de resolución.
#[derive(Debug, Clone)]
pub struct CliInputState {
    positional: Vec<CliArgument>,
    named: HashMap<String, CliArgument>,
}

/// El orquestador que valida y resuelve los parámetros.
pub struct ArgResolver {
    /// Mapa del token original a su valor final resuelto.
    resolved_values: HashMap<String, String>,
    /// El valor final para el token genérico `<axes::params>`.
    generic_params_value: String,
}

// --- PARSER DE DEFINICIONES (DE FASE 1) ---

/// Parses the content of a parameter token, e.g., "0(required)" or "target(alias='-t')".
/// This is called by the main expansion engine in `config_resolver`.
pub fn parse_parameter_token(original_token: &str, content: &str) -> Result<ParameterDef> {
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
        Some(s) => parse_modifiers_string(s)
            .with_context(|| format!("Failed to parse modifiers in token: {}", original_token))?,
        None => ParameterModifiers::default(),
    };

    Ok(ParameterDef {
        kind,
        modifiers,
        original_token: original_token.to_string(),
    })
}

/// Parses a modifier string, e.g., "required, default='staging'".
fn parse_modifiers_string(s: &str) -> Result<ParameterModifiers> {
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

impl CliInputState {
    /// Parsea los `Vec<String>` crudos y construye el estado inicial.
    pub fn new(cli_params: &[String]) -> Result<Self> {
        let mut positional = Vec::new();
        let mut named = HashMap::new();
        let mut params_iter = cli_params.iter().peekable();

        while let Some(param) = params_iter.next() {
            let name_opt = if let Some(name) = param.strip_prefix("--") {
                Some(name)
            } else {
                param.strip_prefix('-')
            };

            if let Some(name) = name_opt {
                let value = if let Some(next_param) = params_iter.peek() {
                    if !next_param.starts_with('-') {
                        Some(params_iter.next().unwrap().clone())
                    } else {
                        None
                    }
                } else {
                    None
                };
                named.insert(
                    name.to_string(),
                    CliArgument {
                        value,
                        consumed: false,
                    },
                );
            } else {
                positional.push(CliArgument {
                    value: Some(param.clone()),
                    consumed: false,
                });
            }
        }
        Ok(Self { positional, named })
    }

    /// Intenta consumir un argumento posicional por su índice.
    pub fn consume_positional(&mut self, index: usize) -> Option<String> {
        if let Some(arg) = self.positional.get_mut(index)
            && !arg.consumed
        {
            arg.consumed = true;
            return arg.value.clone();
        }
        None
    }

    /// Intenta consumir un argumento nombrado, considerando su alias.
    /// Devuelve `Result` para manejar el caso de conflicto.
    pub fn consume_named(
        &mut self,
        name: &str,
        alias: Option<&str>,
    ) -> Result<Option<Option<String>>> {
        let name_present = self.named.contains_key(name);
        let alias_present = alias.is_some_and(|a| self.named.contains_key(a));

        if name_present && alias_present {
            return Err(anyhow!(
                "Conflict: Both flag '{}' and its alias '{}' were provided.",
                format!("--{}", name).cyan(),
                format!("-{}", alias.unwrap()).cyan()
            ));
        }

        let (_key_to_use, arg) = if name_present {
            (name, self.named.get_mut(name).unwrap())
        } else if alias_present {
            (alias.unwrap(), self.named.get_mut(alias.unwrap()).unwrap())
        } else {
            return Ok(None); // Ni el nombre ni el alias fueron provistos.
        };

        if !arg.consumed {
            arg.consumed = true;
            Ok(Some(arg.value.clone()))
        } else {
            // Este caso es teóricamente imposible en un flujo lineal, pero es bueno tenerlo.
            Ok(None)
        }
    }

    /// Recolecta todos los argumentos no consumidos y los formatea en un string.
    /// A diferencia de `consume_...`, esta función no cambia el estado `consumed`.
    pub fn get_unconsumed_as_string(&self) -> (String, bool) {
        let mut parts: Vec<String> = Vec::new();
        let mut had_unconsumed = false;

        for arg in self.positional.iter().filter(|a| !a.consumed) {
            parts.push(arg.value.as_ref().unwrap().clone());
            had_unconsumed = true;
        }

        let mut sorted_named_keys: Vec<_> = self.named.keys().collect();
        sorted_named_keys.sort();

        for key in sorted_named_keys {
            if let Some(arg) = self.named.get(key).filter(|a| !a.consumed) {
                parts.push(format!("-{}", key));
                if let Some(val) = &arg.value {
                    parts.push(val.clone());
                }
                had_unconsumed = true;
            }
        }

        (parts.join(" "), had_unconsumed)
    }
}

impl ArgResolver {
    /// The constructor validates the entire user input against the script's contract
    /// and pre-calculates the value for every parameter definition. It does not
    /// "consume" CLI arguments, allowing multiple parameters to reference the same input.
    pub fn new(
        definitions: &[ParameterDef],
        cli_params: &[String],
        has_generic_params: bool,
    ) -> Result<Self> {
        let mut cli_state = CliInputState::new(cli_params)?;
        let mut resolved_values = HashMap::new();

        // --- Upfront Validation ---
        // 1. Check for alias conflicts (e.g., using --verbose and -v at the same time).
        for def in definitions {
            if let ParameterKind::Named { name } = &def.kind {
                if let Some(alias) = &def.modifiers.alias {
                    if cli_state.named.contains_key(name) && cli_state.named.contains_key(alias) {
                        return Err(anyhow!(
                            "Conflict: Both flag '--{}' and its alias '-{}' were provided.",
                            name.cyan(),
                            alias.cyan()
                        ));
                    }
                }
            }
        }
        
        // --- Resolution Loop ---
        // This loop now correctly handles multiple identical definitions pointing to the same CLI argument.
        for def in definitions {
            // Check if we have already resolved this exact token. If so, skip.
            // This is a micro-optimization for the case where the exact same token string
            // appears multiple times, but the core logic works even without it.
            if resolved_values.contains_key(&def.original_token) {
                continue;
            }

            // Determine if the user provided a value for this definition, WITHOUT consuming state.
            let (is_provided, cli_value) = match &def.kind {
                ParameterKind::Positional { index } => {
                    let val = cli_state.positional.get(*index).and_then(|arg| arg.value.clone());
                    (val.is_some(), val)
                }
                ParameterKind::Named { name } => {
                    let alias = def.modifiers.alias.as_deref();
                    let name_val = cli_state.named.get(name);
                    let alias_val = alias.and_then(|a| cli_state.named.get(a));

                    match (name_val, alias_val) {
                        (Some(arg), _) => (true, arg.value.clone()), // Name takes precedence
                        (_, Some(arg)) => (true, arg.value.clone()), // Alias is used
                        (None, None) => (false, None),
                    }
                }
            };
            
            // Mark the corresponding CLI arguments as "seen" by at least one definition.
            // This is for the final "unconsumed arguments" check.
            if is_provided {
                match &def.kind {
                    ParameterKind::Positional { index } => {
                        if let Some(arg) = cli_state.positional.get_mut(*index) {
                            arg.consumed = true;
                        }
                    },
                    ParameterKind::Named { name } => {
                        if let Some(arg) = cli_state.named.get_mut(name) {
                            arg.consumed = true;
                        } else if let Some(alias) = &def.modifiers.alias {
                            if let Some(arg) = cli_state.named.get_mut(alias) {
                                arg.consumed = true;
                            }
                        }
                    }
                }
            }

            // --- Check `required` constraint ---
            if def.modifiers.required && !is_provided {
                let param_id = match &def.kind {
                    ParameterKind::Positional { index } => format!("Positional argument at index {}", index),
                    ParameterKind::Named { name } => format!("Flag '--{}'", name),
                };
                return Err(anyhow!("{} is required but was not provided.", param_id.cyan()));
            }
            
            // --- Determine Final Value (CLI > Default) ---
            let final_value = if is_provided {
                cli_value
            } else {
                def.modifiers.default_value.clone()
            };

            // --- Apply `map` transformation ---
            let output_flag_name = match (&def.kind, &def.modifiers.map) {
                (_, Some(map_str)) => {
                    if map_str.is_empty() { None } else { Some(map_str.clone()) }
                }
                (ParameterKind::Named { name }, None) => Some(name.clone()),
                (ParameterKind::Positional { .. }, None) => None,
            };
            
            // --- Assemble Final String ---
            let final_string = match (output_flag_name, final_value) {
                (Some(flag), Some(val)) => format!("{} {}", flag, val),
                (Some(flag), None) => flag.to_string(),
                (None, Some(val)) => val,
                (None, None) => String::new(),
            };
            
            resolved_values.insert(def.original_token.clone(), final_string);
        }

        // --- Handle Unconsumed/Leftover Arguments ---
        let (unconsumed_str, had_unconsumed) = cli_state.get_unconsumed_as_string();
        if had_unconsumed && !has_generic_params {
            return Err(anyhow!(
                "{} The script does not define a generic `<axes::params>` token to accept them.\nProvided unhandled arguments: {}",
                "Error: Unexpected arguments were provided.".red(),
                unconsumed_str.yellow()
            ));
        }

        Ok(ArgResolver {
            resolved_values,
            generic_params_value: unconsumed_str,
        })
    }
    
    // This function is now a simple HashMap lookup.
    pub fn get_specific_value(&self, original_token: &str) -> Option<&str> {
        self.resolved_values.get(original_token).map(|s| s.as_str())
    }

    pub fn get_generic_value(&self) -> &str {
        &self.generic_params_value
    }
}