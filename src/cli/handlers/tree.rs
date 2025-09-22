// EN: src/cli/handlers/tree.rs

use anyhow::Result;
use clap::Parser;

use crate::{
    CancellationToken,
    core::{
        context_resolver,
        graph_display::{self, DisplayOptions},
        index_manager,
    },
};

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct TreeArgs {
    /// The project context to start the tree from. Defaults to the root.
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
}

pub fn handle(args: Vec<String>, cancellation_token: &CancellationToken) -> Result<()> {
    let tree_args = TreeArgs::try_parse_from(&args)?;

    // 2. Load the index.
    let index = index_manager::load_and_ensure_global_project()?;

    // 3. Determine the starting node.
    let (start_node_uuid, header) = if let Some(context_str) = tree_args.context {
        let (uuid, qualified_name) =
            context_resolver::resolve_context(&context_str, &index, cancellation_token)?;
        let header = format!(t!("tree.header.from_project"), name = qualified_name);
        (Some(uuid), header)
    } else {
        (None, t!("tree.header.all_projects").to_string())
    };

    // 4. Set display options.
    let display_options = DisplayOptions {
        show_paths: tree_args.paths || tree_args.all,
        show_uuids: tree_args.uuids || tree_args.all,
    };

    // 5. Delegate to the display module.
    println!("\n{}", header);
    graph_display::display_project_tree(&index, start_node_uuid, &display_options);

    Ok(())
}
