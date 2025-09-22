// EN: src/core/graph_display.rs

use crate::{
    core::index_manager,
    models::{GlobalIndex, IndexEntry},
};
use colored::*;
use std::collections::HashMap;
use uuid::Uuid;

/// Options to control the appearance of the rendered tree.
pub struct DisplayOptions {
    pub show_paths: bool,
    pub show_uuids: bool,
}

/// Displays an ASCII tree of all registered projects.
pub fn display_project_tree(
    index: &GlobalIndex,
    start_node_uuid: Option<Uuid>,
    options: &DisplayOptions,
) {
    if index.projects.is_empty() {
        println!("\n{}", t!("tree.info.no_projects"));
        return;
    }

    let mut children_map: HashMap<Option<Uuid>, Vec<(Uuid, &IndexEntry)>> = HashMap::new();
    for (uuid, entry) in &index.projects {
        children_map
            .entry(entry.parent)
            .or_default()
            .push((*uuid, entry));
    }
    for children in children_map.values_mut() {
        children.sort_by_key(|(_, entry)| &entry.name);
    }

    if let Some(start_uuid) = start_node_uuid {
        if start_uuid == index_manager::GLOBAL_PROJECT_UUID {
            // If the start node is the root, render the full tree from the top.
            render_from_root(index, &children_map, options);
        } else if let Some(start_entry) = index.projects.get(&start_uuid) {
            // Render a subtree starting from a specific node.
            let root_name = index
                .projects
                .get(&index_manager::GLOBAL_PROJECT_UUID)
                .unwrap()
                .name
                .clone();
            let qualified_name = index_manager::build_qualified_name(start_uuid, index)
                .unwrap_or_else(|| start_entry.name.clone());

            // Adjust name to not show the root project name if it's a direct child.
            let display_name = qualified_name
                .strip_prefix(&format!("{}/", root_name))
                .unwrap_or(&qualified_name);

            print_node_info(start_uuid, start_entry, index, options, display_name);
            println!(); // Start with a newline

            if let Some(children) = children_map.get(&Some(start_uuid)) {
                for (i, (child_uuid, child_entry)) in children.iter().enumerate() {
                    let is_last = i == children.len() - 1;
                    print_node_recursive(
                        *child_uuid,
                        child_entry,
                        index,
                        &children_map,
                        "",
                        is_last,
                        options,
                    );
                }
            }
        } else {
            println!("\n{}", t!("tree.error.project_not_found").red());
        }
    } else {
        render_from_root(index, &children_map, options);
    }
}

/// Renders the entire tree starting from the project root.
fn render_from_root(
    index: &GlobalIndex,
    children_map: &HashMap<Option<Uuid>, Vec<(Uuid, &IndexEntry)>>,
    options: &DisplayOptions,
) {
    let root_uuid = index_manager::GLOBAL_PROJECT_UUID;
    if let Some(root_entry) = index.projects.get(&root_uuid) {
        print_node_info(root_uuid, root_entry, index, options, &root_entry.name);
        println!();
        if let Some(children) = children_map.get(&Some(root_uuid)) {
            for (i, (child_uuid, child_entry)) in children.iter().enumerate() {
                let is_last = i == children.len() - 1;
                print_node_recursive(
                    *child_uuid,
                    child_entry,
                    index,
                    children_map,
                    "",
                    is_last,
                    options,
                );
            }
        }
    } else {
        println!("\n{}", t!("tree.error.root_project_missing").yellow());
    }
}

/// Prints the formatted information for a single node in the tree.
fn print_node_info(
    uuid: Uuid,
    entry: &IndexEntry,
    index: &GlobalIndex,
    options: &DisplayOptions,
    name: &str,
) {
    // Print the main project name.
    print!("{}", name.cyan());

    let mut info_parts = Vec::new();

    if options.show_paths {
        info_parts.push(format!("[{}]", entry.path.display()).dimmed());
    }
    if options.show_uuids {
        info_parts.push(format!("({})", uuid).dimmed());
    }

    for part in info_parts {
        print!(" {}", part);
    }

    if index.last_used == Some(uuid) {
        print!(" {}", "(**)".yellow());
    }
}

/// Recursive function to print a tree node and its descendants.
fn print_node_recursive(
    uuid: Uuid,
    entry: &IndexEntry,
    index: &GlobalIndex,
    children_map: &HashMap<Option<Uuid>, Vec<(Uuid, &IndexEntry)>>,
    prefix: &str,
    is_last: bool,
    options: &DisplayOptions,
) {
    let connector = if is_last { "└─" } else { "├─" };
    print!("{}{}", prefix, connector);
    print_node_info(uuid, entry, index, options, &entry.name);
    println!();

    let child_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
    if let Some(children) = children_map.get(&Some(uuid)) {
        for (i, (child_uuid, child_entry)) in children.iter().enumerate() {
            let is_last_child = i == children.len() - 1;
            print_node_recursive(
                *child_uuid,
                child_entry,
                index,
                children_map,
                &child_prefix,
                is_last_child,
                options,
            );
        }
    }
}
