// EN: src/cli/handlers/tree.rs

use anyhow::Result;
use clap::Parser;

use crate::{
    core::{
        context_resolver,
        graph_display::{self, DisplayOptions},
    },
    models::GlobalIndex,
};

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true)]
struct TreeArgs {
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

pub fn handle(context: Option<String>, args: Vec<String>, index: &mut GlobalIndex) -> Result<()> {
    // 1. Parse this handler's specific arguments.
    let tree_args = TreeArgs::try_parse_from(&args)?;

    // 2. Determine the starting node for the tree display.
    //    If no context is given, default to '.', which `resolve_context` will
    //    interpret correctly (either CWD or session).
    let context_str = context.unwrap_or_else(|| ".".to_string());
    
    // FIX: Use the passed mutable `index`. No need to load it again.
    //      `resolve_context` will update `last_used` caches in the index.
    let (uuid, qualified_name) = context_resolver::resolve_context(&context_str, index)?;
    
    let header = format!(t!("tree.header.from_project"), name = qualified_name);
    let start_node_uuid = Some(uuid);

    // 3. Set display options based on flags.
    let display_options = DisplayOptions {
        show_paths: tree_args.paths || tree_args.all,
        show_uuids: tree_args.uuids || tree_args.all,
    };

    // 4. Delegate to the graph display module.
    //    The index is now up-to-date with any `last_used` changes.
    println!("\n{}", header);
    graph_display::display_project_tree(index, start_node_uuid, &display_options);

    Ok(())
}