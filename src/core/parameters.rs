// EN: src/core/parameters.rs

use crate::models::{ParameterDef, ParameterKind, ParameterModifiers, TemplateComponent};
use anyhow::{Context, Result, anyhow};
use colored::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};

// --- ESTRUCTURAS DE DATOS ---

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

/// Parsea el contenido de un token de parámetro, ej. "0(required)" o "target(alias='-t')".
fn parse_parameter_token(original_token: &str, content: &str) -> Result<ParameterDef> {
    // Regex para capturar el especificador (nombre/índice) y el bloque de modificadores.
    let re = Regex::new(r"^\s*([^(\s]+)\s*(?:\((.*)\))?\s*$").unwrap();
    let caps = re
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

/// Parsea la cadena de modificadores, ej. "required, default='staging'".
fn parse_modifiers_string(s: &str) -> Result<ParameterModifiers> {
    log::debug!("Parsing modifiers string: '{}'", s);
    let mut modifiers = ParameterModifiers::default();
    if s.trim().is_empty() {
        return Ok(modifiers);
    }

    // Usamos una regex más robusta para manejar comillas y espacios
    let re = Regex::new(r#"\s*([^=,\s]+)(?:\s*=\s*(?:'([^']*)'|"([^"]*)"|([^,]*)))?\s*"#).unwrap();
    for caps in re.captures_iter(s) {
        let key = caps.get(1).map_or("", |m| m.as_str()).trim();
        if key.is_empty() {
            continue;
        }

        // El valor puede estar en uno de tres grupos: comillas simples, dobles o sin comillas.
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

/// Escanea una cadena y la descompone en una secuencia de `TemplateComponent`.
pub fn discover_and_parse(fully_expanded_string: &str) -> Result<Vec<TemplateComponent>> {
    log::debug!(
        "Discovering parameters from string: '{}'",
        fully_expanded_string
    );
    let re = Regex::new(r"<axes::(params(?:[^>])*)>").unwrap();
    let mut components = Vec::new();
    let mut last_match_end = 0;

    for caps in re.captures_iter(fully_expanded_string) {
        let full_match = caps.get(0).unwrap();
        let token_content = caps.get(1).unwrap().as_str();

        let literal_part = &fully_expanded_string[last_match_end..full_match.start()];
        if !literal_part.is_empty() {
            components.push(TemplateComponent::Literal(literal_part.to_string()));
        }

        if token_content == "params" {
            components.push(TemplateComponent::GenericParams);
        } else if let Some(param_spec) = token_content.strip_prefix("params::") {
            let def = parse_parameter_token(full_match.as_str(), param_spec)?;
            components.push(TemplateComponent::Parameter(def));
        } else {
            return Err(anyhow!(
                "Found an unexpected, non-parameter token: '{}'",
                full_match.as_str()
            ));
        }

        last_match_end = full_match.end();
    }

    let remaining_literal = &fully_expanded_string[last_match_end..];
    if !remaining_literal.is_empty() {
        components.push(TemplateComponent::Literal(remaining_literal.to_string()));
    }

    log::debug!("Discovered components: {:?}", components);
    Ok(components)
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

// --- IMPLEMENTACIÓN DEL RESOLVEDOR (FASE 2) ---

/// Resuelve el valor final de un único parámetro.
fn resolve_parameter(def: &ParameterDef, cli_state: &mut CliInputState) -> Result<String> {
    // --- PASO PRELIMINAR: Determinar si el usuario proporcionó el argumento ---
    let (is_provided, cli_value) = match &def.kind {
        ParameterKind::Positional { index } => {
            let val = cli_state.consume_positional(*index);
            (val.is_some(), val)
        }
        ParameterKind::Named { name } => {
            let alias = def.modifiers.alias.as_deref();
            match cli_state.consume_named(name, alias)? {
                Some(val) => (true, val), // `true` incluso para flags booleanos (Some(None))
                None => (false, None),
            }
        }
    };

    // --- FASE 1: Comprobar `required` ---
    if def.modifiers.required && !is_provided {
        let param_id = match &def.kind {
            ParameterKind::Positional { index } => {
                format!("Positional argument at index {}", index)
            }
            ParameterKind::Named { name } => format!("Flag '--{}'", name),
        };
        return Err(anyhow!(
            "{} is required but was not provided.",
            param_id.cyan()
        ));
    }

    // --- FASE 2: Si no se proporcionó, el valor es una cadena vacía y terminamos ---
    if !is_provided && def.modifiers.default_value.is_none() {
        // No se proporcionó, no es requerido (pasó la fase 1) y no hay default. El resultado es nada.
        return Ok(String::new());
    }

    // --- FASE 3 y 4: Determinar el VALOR (CLI > default) ---
    let final_value: Option<String> = if is_provided {
        cli_value
    } else {
        def.modifiers.default_value.clone()
    };

    // --- FASE 5 y 6: Formatear la SALIDA usando `map` ---
    let output_flag_name: Option<String> = match (&def.kind, &def.modifiers.map) {
        // Un `map` siempre tiene prioridad para el nombre del flag.
        (_, Some(map_str)) => {
            if map_str.is_empty() {
                None
            } else {
                Some(map_str.clone())
            }
        }
        // Si no hay `map`, un parámetro nombrado usa su propio nombre.
        (ParameterKind::Named { name }, None) => Some(name.clone()),
        // Si no hay `map` y es posicional, no tiene nombre de flag.
        (ParameterKind::Positional { .. }, None) => None,
    };

    // Construir la cadena final
    match (output_flag_name, final_value) {
        // Caso: Tiene flag y tiene valor (ej. --key value)
        (Some(flag), Some(val)) => Ok(format!("{} {}", flag, val)),
        // Caso: Tiene flag pero no valor (ej. --key)
        (Some(flag), None) => Ok(flag.to_string()),
        // Caso: No tiene flag pero sí valor (ej. un argumento posicional simple)
        (None, Some(val)) => Ok(val),
        // Caso: No tiene flag ni valor (no debería ocurrir si `is_provided` o `default` era Some)
        (None, None) => Ok(String::new()),
    }
}

impl ArgResolver {
    /// El constructor principal que ejecuta toda la lógica de resolución y validación.
    pub fn new(
        definitions: &[ParameterDef],
        cli_params: &[String],
        has_generic_params: bool,
    ) -> Result<Self> {
        let mut cli_state = CliInputState::new(cli_params)?;
        let mut resolved_values = HashMap::new();

        // Validar que no haya definiciones duplicadas (ej. dos <axes::params::0>).
        let mut seen_defs = HashSet::new();
        for def in definitions {
            if !seen_defs.insert(&def.kind) {
                return Err(anyhow!(
                    "Parameter '{:?}' is defined multiple times in the script.",
                    def.kind
                ));
            }
        }

        // Bucle de resolución lineal.
        for def in definitions {
            let resolved_value = resolve_parameter(def, &mut cli_state)?;
            resolved_values.insert(def.original_token.clone(), resolved_value);
        }

        // Manejo de argumentos sobrantes.
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

    /// Obtiene el valor resuelto para un token de parámetro específico.
    pub fn get_specific_value(&self, original_token: &str) -> Option<&str> {
        self.resolved_values.get(original_token).map(|s| s.as_str())
    }

    /// Obtiene el valor resuelto para el token genérico `<axes::params>`.
    pub fn get_generic_value(&self) -> &str {
        &self.generic_params_value
    }
}
