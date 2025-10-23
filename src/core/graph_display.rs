use crate::{
    core::index_manager,
    models::{GlobalIndex, IndexEntry},
};
use colored::*;
use std::collections::HashMap;
use uuid::Uuid;

/// Options to control the appearance of the rendered project tree.
#[derive(Default, Debug)]
pub struct DisplayOptions {
    /// If true, display the absolute filesystem path for each project.
    pub show_paths: bool,
    /// If true, display the UUID for each project.
    pub show_uuids: bool,
    /// An optional limit on the depth of the tree to display.
    pub max_depth: Option<usize>,
    /// If true, check if each project's path exists on the filesystem and show a warning if not.
    pub show_health: bool,
}

/// A context struct to hold all data needed for rendering the tree.
struct TreeRenderer<'a> {
    index: &'a GlobalIndex,
    options: &'a DisplayOptions,
    children_map: HashMap<Option<Uuid>, Vec<(Uuid, &'a IndexEntry)>>,
    alias_map: HashMap<Uuid, Vec<&'a str>>,
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

        let mut alias_map: HashMap<Uuid, Vec<&'a str>> = HashMap::new();
        for (alias_name, uuid) in &index.aliases {
            alias_map.entry(*uuid).or_default().push(alias_name);
        }
        // Sort alias names for deterministic output.
        for aliases in alias_map.values_mut() {
            aliases.sort_unstable();
        }

        Self {
            index,
            options,
            children_map,
            alias_map,
        }
    }

    /// Renders the tree starting from a given node (or the root if `None`).
    fn render_tree(&self, start_node_uuid_opt: Option<Uuid>) {
        // Logic for finding the start node.
        let (start_uuid, is_subtree) = match start_node_uuid_opt {
            Some(uuid) => (uuid, true), // Rendering a specific subtree
            None => (index_manager::GLOBAL_PROJECT_UUID, false), // Rendering the full tree
        };

        if let Some(start_entry) = self.index.projects.get(&start_uuid) {
            // If it's a subtree, we don't print the root node itself as part of the tree lines.
            // The header already announced it.
            if !is_subtree {
                self.print_node_info(start_uuid, start_entry);
                println!();
            }
            self.render_children_of(start_uuid, "", 1);
        } else {
            let error_message = if is_subtree {
                t!("tree.error.project_not_found").red()
            } else {
                t!("tree.error.root_project_missing").yellow()
            };
            println!("\n{}", error_message);
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
            let formatted_aliases = aliases
                .iter()
                .map(|name| format!("{}!", name))
                .collect::<Vec<_>>()
                .join(", ");
            info_parts.push(formatted_aliases.bright_blue().to_string());
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
        let mut legend_items = Vec::new();

        // Build the legend dynamically based on active options.
        legend_items.push(format!(
            "{} = {}",
            "alias!".bright_blue(),
            t!("tree.legend.alias")
        ));
        legend_items.push(format!(
            "{} = {}",
            t!("tree.label.last_used").yellow(),
            t!("tree.legend.last_used")
        ));

        if self.options.show_health {
            legend_items.push(format!(
                "{} = {}",
                t!("tree.label.broken_path").yellow(),
                t!("tree.legend.broken_path")
            ));
        }

        if !legend_items.is_empty() {
            println!("\nLegend: {}", legend_items.join(", ").dimmed());
        }
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
