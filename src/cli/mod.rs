use clap::Parser;

pub mod handlers;

/// Builds the dynamic, color-aware full help string at runtime.
fn build_help_string() -> &'static str {
    // This function acts as a mini-renderer for our semantic help template.
    // It replaces placeholders like `<title>` with colored/styled text.

    let use_colors = colored::control::SHOULD_COLORIZE.should_colorize();

    let template = t!("cli.help.template");

    // Define styles. If colors are disabled, they are empty strings.
    let title = if use_colors { "\x1b[1;33m" } else { "" }; // Bold Yellow
    let hl = if use_colors { "\x1b[1;36m" } else { "" };    // Bold Cyan (for highlights)
    let hi = if use_colors { "\x1b[1;20m" } else { "" };    // Bold Cyan (for highlights)
    let cmd = if use_colors { "\x1b[36m" } else { "" };     // Cyan (for commands)
    let group = if use_colors { "\x1b[1;32m" } else { "" }; // Bold Green
    let err = if use_colors { "\x1b[91m" } else { "" };      // Bright Red (for destructive)
    let dim = if use_colors { "\x1b[2m" } else { "" };      // Dim
    let reset = if use_colors { "\x1b[0m" } else { "" };

    // Perform replacements in a single, chained expression.
    let formatted_string = template
        .replace("<title>", title)
        .replace("</title>", reset)
        .replace("<hl>", hl)
        .replace("</hl>", reset)
        .replace("<hi>", hi)
        .replace("</hi>", reset)
        .replace("<cmd>", cmd)
        .replace("</cmd>", reset)
        .replace("<group>", group)
        .replace("</group>", reset)
        .replace("<err>", err)
        .replace("</err>", reset)
        .replace("<dim>", dim)
        .replace("</dim>", reset);


    Box::leak(formatted_string.into_boxed_str())
}

/// axes: A high-performance, session-aware workflow orchestrator.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    // Use `help_template` to take full control of the output.
    help_template = { build_help_string() },
    // Custom styles for auto-generated parts (not used now, but good to keep for future)
    styles = clap::builder::Styles::styled()
        .header(clap::builder::styling::AnsiColor::Yellow.on_default().bold())
        .usage(clap::builder::styling::AnsiColor::Yellow.on_default().bold())
        .literal(clap::builder::styling::AnsiColor::Cyan.on_default().bold())
        .placeholder(clap::builder::styling::AnsiColor::Green.on_default()),
)]
// We disable clap's default help subcommand (`help`) as we provide a complete template.
#[command(disable_help_subcommand = true)]
#[command(trailing_var_arg = true)]
pub struct Cli {
    /// The sequence of arguments passed to axes.
    /// This argument is now "invisible" in the help output, which is what we want.
    /// Its presence is only for clap's internal parsing.
    #[arg()]
    pub args: Vec<String>,
}