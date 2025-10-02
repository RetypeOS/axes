// EN: src/cli/mod.rs
use clap::Parser;

pub mod handlers;

/// axes: A high-performance, session-aware workflow orchestrator.
///
/// axes uses a universal grammar to interpret commands. The logic is as follows:
/// 1. `axes <context> <action> [args...]` - If the second argument is a system action.
/// 2. `axes <action> [args...]` - If the first argument is a system action.
/// 3. `axes <script> [params...]` - The default, a shortcut for running a script in the current context.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(disable_help_subcommand = true)]
#[command(trailing_var_arg = true)]
pub struct Cli {
    /// The sequence of arguments passed to axes.
    ///
    /// This captures all arguments (contexts, actions, scripts, parameters, flags)
    /// into a single vector. The `dispatcher` is then responsible for interpreting
    /// this sequence according to the universal grammar.
    #[arg()]
    pub args: Vec<String>,
}
