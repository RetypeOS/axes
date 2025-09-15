// src/cli.rs

use clap::Parser;

/// axes: A holistic and hierarchical development workflow orchestrator.
///
/// `axes` operates in two main modes:
///
/// 1. SCRIPT MODE (default):
///    The syntax is flexible. `axes` determines if an argument is an action or a
///    project context based on a list of known system actions.
///
///    Valid formats:
///    - `axes <context> <action> [args...]` (e.g., `axes my-app/api info`)
///    - `axes <action> <context> [args...]` (e.g., `axes info my-app/api`)
///
///    Shortcuts:
///    - `axes <context>` -> expands to `axes <context> start`
///    - `axes <context> <script>` -> expands to `axes <context> run <script>`
///
/// 2. SESSION MODE (when `AXES_PROJECT_UUID` is defined):
///    The syntax is strict, as the project context is implicit.
///
///    Valid format:
///    - `axes <action> [args...]` (e.g., `axes tree`)
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(disable_help_subcommand = true)]
pub struct Cli {
    /// The first positional argument.
    ///
    /// Its role depends on the mode and other arguments:
    /// - In SCRIPT MODE, it can be a project context, a system action,
    ///   or a global action.
    /// - In SESSION MODE, it is ALWAYS an action.
    /// - If omitted, an attempt will be made to launch the TUI.
    pub context_or_action: Option<String>,

    /// The second positional argument.
    ///
    /// Its role depends on the first argument:
    /// - If the first argument was an ACTION, this is the CONTEXT.
    /// - If the first argument was a CONTEXT, this can be an ACTION or the
    ///   name of a SCRIPT.
    /// - For global actions (`init`, `register`, `alias`), this is the first
    ///   argument for that action (e.g., the name of an alias).
    pub action_or_context_or_arg: Option<String>,

    /// All remaining arguments.
    ///
    /// These are passed directly to the action being executed. For example, the
    /// parameters for a `run` script, the new name for `rename`, or the
    /// flags for `delete`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}
