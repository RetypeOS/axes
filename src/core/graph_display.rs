// src/core/graph_display.rs

use crate::models::{GlobalIndex, IndexEntry};
use std::collections::HashMap;
use uuid::Uuid;

/// Displays an ASCII tree of all registered projects.
pub fn display_project_tree(index: &GlobalIndex, start_node_uuid: Option<Uuid>) {
    if index.projects.is_empty() {
        println!("\nNo hay proyectos registrados. Usa 'axes init <nombre>' para empezar.");
        return;
    }

    // 1. Build the relationship map (unchanged)
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

    // 2. Determine the starting point
    if let Some(start_uuid) = start_node_uuid {
        // Start from a specific node
        if let Some(start_entry) = index.projects.get(&start_uuid) {
            // Print the starting node as if it were a root (no prefix or connector)
            let last_used_marker = if index.last_used == Some(start_uuid) {
                " (**)"
            } else {
                ""
            };
            println!(
                "{} [{}] {}",
                start_entry.name,
                start_entry.path.display(),
                last_used_marker
            );

            // Print its children
            if let Some(children) = children_map.get(&Some(start_uuid)) {
                for (i, (child_uuid, child_entry)) in children.iter().enumerate() {
                    let is_last_child = i == children.len() - 1;
                    print_node(
                        *child_uuid,
                        child_entry,
                        index,
                        &children_map,
                        "",
                        is_last_child,
                    );
                }
            }
        } else {
            println!("\nError: The specified starter project was not found in the index.");
        }
    } else {
        // Default behavior: start from the roots (`global`)
        if let Some(roots) = children_map.get(&None) {
            println!("\nRegistered Project Tree:");
            for (i, (uuid, root_entry)) in roots.iter().enumerate() {
                let is_last = i == roots.len() - 1;
                print_node(*uuid, root_entry, index, &children_map, "", is_last);
            }
        } else {
            println!("\nWarning: No root projects found, but there are registered projects.");
            println!("This may indicate a corrupt index.");
        }
    }
}

/// Recursive function to print a tree node and its descendants.
fn print_node(
    uuid: Uuid,
    entry: &IndexEntry,
    index: &GlobalIndex,
    children_map: &HashMap<Option<Uuid>, Vec<(Uuid, &IndexEntry)>>,
    prefix: &str,
    is_last: bool,
) {
    let connector = if is_last { "└─" } else { "├─" };

    // Check if it's the globally last used project
    let last_used_marker = if index.last_used == Some(uuid) {
        " (**)"
    } else {
        ""
    };

    println!(
        "{}{}{} [{}] {}",
        prefix,
        connector,
        entry.name,
        entry.path.display(),
        last_used_marker
    );

    // Prepare the prefix for the children of this node
    let child_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });

    // Recursion over the children
    if let Some(children) = children_map.get(&Some(uuid)) {
        for (i, (child_uuid, child_entry)) in children.iter().enumerate() {
            let is_last_child = i == children.len() - 1;
            print_node(
                *child_uuid,
                child_entry,
                index,
                children_map,
                &child_prefix,
                is_last_child,
            );
        }
    }
}
