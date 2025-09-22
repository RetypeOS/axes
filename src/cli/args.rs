// EN: src/cli/args.rs
use clap::Parser;

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)] // Important: Prevents clap from expecting "init" as the first arg
pub struct InitArgs {
    /// The name for the new project. If not provided, will be asked interactively.
    pub name: Option<String>,

    /// The context of the parent project. Defaults to 'global'.
    #[arg(long)]
    pub parent: Option<String>,

    /// The name of the template to use from `~/.config/axes/templates`.
    #[arg(long, short)]
    pub template: Option<String>,

    /// The version of the project.
    #[arg(long)]
    pub version: Option<String>,

    /// A short description of the project.
    #[arg(long)]
    pub description: Option<String>,

    /// Do not ask for user input, use defaults for unspecified values.
    #[arg(long)]
    pub autosolve: bool,

    /// Set environment variables for the project (e.g., "KEY=VALUE").
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    pub env: Vec<String>,

    /// Set interpolation variables for the project (e.g., "KEY=VALUE").
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    pub var: Vec<String>,
}

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
pub struct RegisterArgs {
    /// The path to the project to register. Defaults to the current directory.
    pub path: Option<String>,

    /// Do not ask for user input, fail on any conflict.
    #[arg(long)]
    pub autosolve: bool,
}
