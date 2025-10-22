use anyhow::Result;
use clap::Parser;

use crate::{
    core::{
        context_resolver,
        graph_display::{self, DisplayOptions},
    },
    state::AppStateGuard,
};
use colored::Colorize;

#[derive(Parser, Debug, Default)]
#[command(
    no_binary_name = true,
    about = "Displays the project hierarchy as a tree."
)]
struct TreeArgs {
    /// The project context to use as the root of the tree. Defaults to the full tree.
    context: Option<String>,

    /// Show the full absolute paths for each project.
    #[arg(long, short)]
    paths: bool,

    /// Show the UUID for each project.
    #[arg(long, short)]
    uuids: bool,

    /// Show all available information (paths and UUIDs).
    #[arg(long)]
    all: bool,

    /// Limit the depth of the tree display.
    #[arg(long, short)]
    depth: Option<usize>,

    /// Check if the project paths exist on the filesystem.
    #[arg(long)]
    check: bool,
}

pub fn handle(
    context: Option<String>,
    args: Vec<String>,
    state_guard: &mut AppStateGuard,
) -> Result<()> {
    // 1. Parse this handler's specific arguments.
    let tree_args = TreeArgs::try_parse_from(&args)?;

    // 2. Determine the definitive context with clear priority: cli arg > dispatcher context.
    let final_context = tree_args.context.or(context);

    // 3. Resolve the start node and prepare the UI header based on the context.
    let (start_node_uuid, header) = match final_context {
        Some(context_str) => {
            let (uuid, qualified_name) =
                context_resolver::resolve_context(&context_str, state_guard)?;
            let header_text = format!(t!("tree.header.from_project"), name = qualified_name.cyan());
            (Some(uuid), header_text)
        }
        None => {
            // No context provided, display the full tree from the global project.
            (None, t!("tree.header.full_tree").to_string())
        }
    };

    // 4. Set display options based on flags.
    let display_options = DisplayOptions {
        show_paths: tree_args.paths || tree_args.all,
        show_uuids: tree_args.uuids || tree_args.all,
        max_depth: tree_args.depth,
        show_health: tree_args.check,
    };

    // 5. Delegate to the graph display module for rendering.
    println!("\n{}", header);
    graph_display::display_project_tree(state_guard.index(), start_node_uuid, &display_options);

    Ok(())
}
