// EN: src/core/graph_display.rs (FINAL CORRECTED VERSION)

use crate::{
    core::index_manager,
    models::{GlobalIndex, IndexEntry},
};
use colored::*;
use std::collections::HashMap;
use uuid::Uuid;

/// Options to control the appearance of the rendered tree.
#[derive(Default)]
pub struct DisplayOptions {
    pub show_paths: bool,
    pub show_uuids: bool,
    pub max_depth: Option<usize>,
    pub show_health: bool,
}

/// A context struct to hold all data needed for rendering the tree.
struct TreeRenderer<'a> {
    index: &'a GlobalIndex,
    options: &'a DisplayOptions,
    children_map: HashMap<Option<Uuid>, Vec<(Uuid, &'a IndexEntry)>>,
    alias_map: HashMap<Uuid, Vec<String>>,
}

impl<'a> TreeRenderer<'a> {
    /// Creates a new renderer and pre-computes necessary lookup maps.
    fn new(index: &'a GlobalIndex, options: &'a DisplayOptions) -> Self {
        let mut children_map: HashMap<Option<Uuid>, Vec<(Uuid, &'a IndexEntry)>> = HashMap::new();
        for (uuid, entry) in &index.projects {
            children_map
                .entry(entry.parent)
                .or_default()
                .push((*uuid, entry));
        }
        for children in children_map.values_mut() {
            children.sort_by_key(|(_, entry)| &entry.name);
        }

        let mut alias_map: HashMap<Uuid, Vec<String>> = HashMap::new();
        for (alias_name, uuid) in &index.aliases {
            alias_map
                .entry(*uuid)
                .or_default()
                .push(format!("@{}", alias_name));
        }

        Self {
            index,
            options,
            children_map,
            alias_map,
        }
    }

    /// Renders the tree starting from a given node (or the root if `None`).
    fn render_tree(&self, start_node_uuid: Option<Uuid>) {
        if let Some(start_uuid) = start_node_uuid {
            // Render a subtree from a specific node.
            if let Some(start_entry) = self.index.projects.get(&start_uuid) {
                self.print_node_info(start_uuid, start_entry);
                println!();
                self.render_children_of(start_uuid, "", 1);
            } else {
                println!("\n{}", t!("tree.error.project_not_found").red());
            }
        } else {
            // Render the full tree from the actual root project.
            let root_uuid = index_manager::GLOBAL_PROJECT_UUID;
            if let Some(root_entry) = self.index.projects.get(&root_uuid) {
                self.print_node_info(root_uuid, root_entry);
                println!();
                self.render_children_of(root_uuid, "", 1);
            } else {
                println!("\n{}", t!("tree.error.root_project_missing").yellow());
            }
        }
        self.print_legend();
    }

    /// Renders all direct children of a given node recursively.
    fn render_children_of(&self, parent_uuid: Uuid, prefix: &str, depth: usize) {
        if let Some(max_depth) = self.options.max_depth
            && depth > max_depth
        {
            return;
        }

        if let Some(children) = self.children_map.get(&Some(parent_uuid)) {
            for (i, (child_uuid, child_entry)) in children.iter().enumerate() {
                let is_last_child = i == children.len() - 1;
                let connector = if is_last_child { "└─" } else { "├─" };
                let child_prefix =
                    format!("{}{}", prefix, if is_last_child { "   " } else { "│  " });

                print!("{}{}", prefix, connector);
                self.print_node_info(*child_uuid, child_entry);
                println!();

                self.render_children_of(*child_uuid, &child_prefix, depth + 1);
            }
        }
    }

    /// Prints the formatted information for a single node.
    fn print_node_info(&self, uuid: Uuid, entry: &IndexEntry) {
        print!("{}", entry.name.cyan());
        let mut info_parts = Vec::new();

        if self.options.show_health && !entry.path.exists() {
            info_parts.push(t!("tree.label.broken_path").yellow().to_string());
        }
        if self.index.last_used == Some(uuid) {
            info_parts.push(t!("tree.label.last_used").yellow().to_string());
        }
        if let Some(aliases) = self.alias_map.get(&uuid) {
            info_parts.push(aliases.join(", ").bright_blue().to_string());
        }
        if self.options.show_paths {
            info_parts.push(format!("[{}]", entry.path.display()).dimmed().to_string());
        }
        if self.options.show_uuids {
            info_parts.push(format!("({})", uuid).dimmed().to_string());
        }

        if !info_parts.is_empty() {
            print!(" {}", info_parts.join(" "));
        }
    }

    /// Prints a helpful legend.
    fn print_legend(&self) {
        let mut legend_items = vec![
            format!(
                "{} = {}",
                t!("tree.label.last_used").yellow(),
                t!("tree.legend.last_used")
            ),
            format!("{} = {}", "@alias".bright_blue(), t!("tree.legend.alias")),
        ];
        if self.options.show_health {
            legend_items.push(format!(
                "{} = {}",
                t!("tree.label.broken_path").yellow(),
                t!("tree.legend.broken_path")
            ));
        }
        println!("\nLegend: {}", legend_items.join(", ").dimmed());
    }
}

/// The public entry point for displaying the project tree.
pub fn display_project_tree(
    index: &GlobalIndex,
    start_node_uuid: Option<Uuid>,
    options: &DisplayOptions,
) {
    if index.projects.is_empty() {
        println!("\n{}", t!("tree.info.no_projects"));
        return;
    }

    let renderer = TreeRenderer::new(index, options);
    renderer.render_tree(start_node_uuid);
}
