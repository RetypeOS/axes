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
    pub max_depth: Option<usize>,
    pub show_health: bool,
}

/// Displays an ASCII tree of all registered projects with enhanced diagnostic info.
pub fn display_project_tree(
    index: &GlobalIndex,
    start_node_uuid: Option<Uuid>,
    options: &DisplayOptions,
) {
    if index.projects.is_empty() {
        println!("\n{}", t!("tree.info.no_projects"));
        return;
    }

    // --- Pre-computation for performance ---
    // 1. Build a map of parent -> children.
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

    // 2. Build a reverse map of project UUID -> list of aliases pointing to it.
    let mut alias_map: HashMap<Uuid, Vec<String>> = HashMap::new();
    for (alias_name, uuid) in &index.aliases {
        alias_map
            .entry(*uuid)
            .or_default()
            .push(format!("@{}", alias_name));
    }

    // --- Rendering ---
    let root_uuid = start_node_uuid.unwrap_or(index_manager::GLOBAL_PROJECT_UUID);
    if let Some(root_entry) = index.projects.get(&root_uuid) {
        print_node_info(root_uuid, root_entry, index, &alias_map, options);
        println!();
        if let Some(children) = children_map.get(&Some(root_uuid)) {
            for (i, (child_uuid, child_entry)) in children.iter().enumerate() {
                let is_last = i == children.len() - 1;
                print_node_recursive(
                    *child_uuid,
                    child_entry,
                    index,
                    &children_map,
                    &alias_map,
                    "",
                    is_last,
                    options,
                    1, // Start at depth 1
                );
            }
        }
    } else {
        println!("\n{}", t!("tree.error.project_not_found").red());
    }

    // --- [NEW] Show legend for clarity ---
    print_legend(options);
}

/// Prints the formatted information for a single node in the tree.
fn print_node_info(
    uuid: Uuid,
    entry: &IndexEntry,
    index: &GlobalIndex,
    alias_map: &HashMap<Uuid, Vec<String>>,
    options: &DisplayOptions,
) {
    // Print the main project name.
    print!("{}", entry.name.cyan());

    let mut info_parts = Vec::new();

    // [NEW] Health Check
    if options.show_health && !entry.path.exists() {
        info_parts.push(t!("tree.label.broken_path").yellow().to_string());
    }

    // `last_used` marker
    if index.last_used == Some(uuid) {
        info_parts.push(t!("tree.label.last_used").yellow().to_string());
    }

    // [NEW] Alias markers
    if let Some(aliases) = alias_map.get(&uuid) {
        info_parts.push(aliases.join(", ").bright_blue().to_string());
    }

    if options.show_paths {
        info_parts.push(format!("[{}]", entry.path.display()).dimmed().to_string());
    }
    if options.show_uuids {
        info_parts.push(format!("({})", uuid).dimmed().to_string());
    }

    if !info_parts.is_empty() {
        print!(" {}", info_parts.join(" "));
    }
}

/// Recursive function to print a tree node and its descendants.
fn print_node_recursive(
    uuid: Uuid,
    entry: &IndexEntry,
    index: &GlobalIndex,
    children_map: &HashMap<Option<Uuid>, Vec<(Uuid, &IndexEntry)>>,
    alias_map: &HashMap<Uuid, Vec<String>>,
    prefix: &str,
    is_last: bool,
    options: &DisplayOptions,
    depth: usize,
) {
    // [NEW] Depth check
    if let Some(max_depth) = options.max_depth {
        if depth > max_depth {
            return;
        }
    }

    let connector = if is_last { "└─" } else { "├─" };
    print!("{}{}", prefix, connector);
    print_node_info(uuid, entry, index, alias_map, options);
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
                alias_map,
                &child_prefix,
                is_last_child,
                options,
                depth + 1,
            );
        }
    }
}

/// [NEW] Prints a helpful legend explaining the symbols used in the tree.
fn print_legend(options: &DisplayOptions) {
    let mut legend_items = vec![
        format!(
            "'{}' = {}",
            t!("tree.label.last_used").yellow(),
            t!("tree.legend.last_used")
        ),
        format!("{} = {}", "@alias".bright_blue(), t!("tree.legend.alias")),
    ];
    if options.show_health {
        legend_items.push(format!(
            "{} = {}",
            t!("tree.label.broken_path").yellow(),
            t!("tree.legend.broken_path")
        ));
    }
    println!("\nLegend: {}", legend_items.join(", ").dimmed());
}
