// EN: src/core/arg_parser.rs

use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct PositionalArg<'a> {
    value: &'a str,
    consumed: bool,
}

#[derive(Debug, Clone)]
struct NamedArg<'a> {
    value: Option<&'a str>, // Some("val") for --key val, None for --key
    consumed: bool,
}

/// A parser and state manager for command-line arguments passed to an `axes` script.
/// It classifies arguments into positional and named (flags) and tracks their consumption
/// by tokens in the script.
#[derive(Debug, Clone)]
pub struct ParsedArgs<'a> {
    positional: Vec<PositionalArg<'a>>,
    named: HashMap<String, NamedArg<'a>>,
}

impl<'a> ParsedArgs<'a> {
    /// Parses the raw CLI parameters from `Vec<String>` into a structured format.
    ///
    /// # Logic:
    /// - Any token starting with `-` or `--` is a named argument (flag).
    /// - If a flag is followed by a token that is *not* a flag, that token is
    ///   considered the value for the flag.
    /// - Otherwise, it's a positional argument.
    pub fn new(cli_params: &'a [String]) -> Result<Self> {
        let mut positional = Vec::new();
        let mut named = HashMap::new();
        let mut params_iter = cli_params.iter().map(String::as_str).peekable();

        while let Some(param) = params_iter.next() {
            let name_opt = if let Some(name) = param.strip_prefix("--") {
                Some(name)
            } else {
                param.strip_prefix('-')
            };

            if let Some(name) = name_opt {
                // It's a flag.
                let value = if let Some(next_param) = params_iter.peek() {
                    if !next_param.starts_with('-') {
                        Some(params_iter.next().unwrap())
                    } else {
                        None // The next token is another flag.
                    }
                } else {
                    None // End of arguments.
                };
                named.insert(
                    name.to_string(),
                    NamedArg {
                        value,
                        consumed: false,
                    },
                );
            } else {
                // It's a positional argument.
                positional.push(PositionalArg {
                    value: param,
                    consumed: false,
                });
            }
        }

        Ok(Self { positional, named })
    }

    /// Retrieves a positional argument by its index.
    /// It marks the argument as "consumed".
    /// Returns an empty string if the argument does not exist.
    pub fn get_positional(&mut self, index: usize) -> &str {
        if let Some(arg) = self.positional.get_mut(index) {
            arg.consumed = true;
            arg.value
        } else {
            ""
        }
    }

    /// Retrieves a mapped value for a named flag.
    /// If the flag is present in the CLI arguments, it returns `value_if_present`.
    /// Otherwise, it returns an empty string. Marks the flag as "consumed".
    pub fn get_mapped_flag<'b>(&mut self, name: &str, value_if_present: &'b str) -> &'b str {
        if let Some(named_arg) = self.named.get_mut(name) {
            named_arg.consumed = true;
            value_if_present
        } else {
            ""
        }
    }

    /// Finds a named argument and returns its reconstructed string version (`--name value` or `--name`).
    /// Marks the argument (and its value, if any) as consumed.
    /// Returns an empty string if the flag is not found.
    pub fn consume_named_passthrough(&mut self, name: &str) -> String {
        if let Some(arg) = self.named.get_mut(name) {
            arg.consumed = true;
            if let Some(val) = arg.value {
                // Return "--name value"
                format!("--{} {}", name, val)
            } else {
                // Return "--name"
                format!("--{}", name)
            }
        } else {
            String::new()
        }
    }

    /// Retrieves and consumes all remaining, unconsumed arguments, formatting them
    /// into a single string suitable for the generic `<params>` token.
    pub fn consume_remaining(&mut self) -> String {
        let mut remaining_parts: Vec<String> = Vec::new();

        // Collect unconsumed positional args
        for arg in self.positional.iter_mut().filter(|arg| !arg.consumed) {
            remaining_parts.push(arg.value.to_string());
            arg.consumed = true;
        }

        // Collect unconsumed named args in a deterministic order
        let mut named_keys: Vec<_> = self.named.keys().cloned().collect();
        named_keys.sort();

        for name in named_keys {
            if let Some(arg) = self.named.get_mut(&name)
                && !arg.consumed
            {
                remaining_parts.push(format!("--{}", name));
                if let Some(val) = arg.value {
                    remaining_parts.push(val.to_string());
                }
                arg.consumed = true;
            }
        }

        remaining_parts.join(" ")
    }

    /// Checks if all arguments originally passed on the command line have been
    /// consumed by either an explicit token (e.g., `<params::0>`) or the
    /// generic `<params>` token.
    pub fn all_consumed(&self) -> bool {
        self.positional.iter().all(|arg| arg.consumed)
            && self.named.values().all(|arg| arg.consumed)
    }
}
