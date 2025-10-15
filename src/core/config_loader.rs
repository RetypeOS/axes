use crate::{
    core::compiler,
    models::{GlobalIndex, LayerPromise, ResolvedConfig},
};
use anyhow::{Result, anyhow};
use log;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use uuid::Uuid;

/// Orchestrates the loading of configuration layers for a project hierarchy.
pub struct ConfigLoader<'a> {
    index: &'a mut GlobalIndex,
}

impl<'a> ConfigLoader<'a> {
    /// Creates a new ConfigLoader.
    pub fn new(index: &'a mut GlobalIndex) -> Self {
        Self { index }
    }

    /// The main entry point for configuration resolution.
    /// It determines the project hierarchy and orchestrates the PARALLEL loading of each layer.
    pub fn resolve(&mut self, uuid: Uuid) -> Result<ResolvedConfig> {
        log::debug!("ConfigLoader resolving UUID: {}", uuid);

        // 1. Determine the full inheritance hierarchy.
        let mut hierarchy = Vec::new();
        let mut current_uuid = Some(uuid);
        while let Some(id) = current_uuid {
            hierarchy.push(id);
            let entry = self.index.projects.get(&id).ok_or_else(|| {
                let child_uuid = hierarchy.get(hierarchy.len().saturating_sub(2)).unwrap_or(&uuid);
                anyhow!(
                    "Data Integrity Error: Project UUID '{}' lists a parent with UUID '{}', but no such project exists in the index.",
                    child_uuid,
                    id
                )
            })?;
            current_uuid = entry.parent;
        }

        // 2. Prepare promises and a thread-safe container for index updates.
        let layer_promises: HashMap<Uuid, LayerPromise> = hierarchy
            .iter()
            .map(|&id| (id, Arc::new(OnceLock::new())))
            .collect();

        let index_updates = Arc::new(Mutex::new(Vec::new()));

        // Create an immutable reference to the index to share across threads.
        let index_ref: &GlobalIndex = self.index;

        hierarchy.par_iter().for_each(|&layer_uuid| {
            let promise = layer_promises.get(&layer_uuid).unwrap();

            log::trace!("Executing load task for UUID: {}", layer_uuid);
            let task_result = compiler::load_layer_task(layer_uuid, index_ref);

            // The result that will be stored in the promise.
            // We simplify the match logic to be more direct.
            let layer_result_for_promise = match task_result {
                Ok((layer_arc, Some(update))) => {
                    index_updates.lock().unwrap().push(update);
                    Ok(layer_arc)
                }
                Ok((layer_arc, None)) => Ok(layer_arc),
                Err(e) => Err(e),
            };

            // `set` can only fail if the cell is already full, which is impossible in this
            // single-producer logic, but we log a critical error just in case.
            if promise.set(layer_result_for_promise).is_err() {
                log::error!(
                    "CRITICAL: LayerPromise for {} was set twice. This indicates a logic error.",
                    layer_uuid
                );
            }
        });

        // 4. Apply all collected index updates sequentially.
        // This part runs after the parallel section is fully completed.
        let updates_to_apply = Arc::try_unwrap(index_updates)
            .expect("Mutex should not be locked elsewhere at this point")
            .into_inner()
            .unwrap();

        if !updates_to_apply.is_empty() {
            log::debug!(
                "Applying {} cache metadata updates to the global index.",
                updates_to_apply.len()
            );
            for update in updates_to_apply {
                if let Some(entry) = self.index.projects.get_mut(&update.uuid) {
                    entry.config_hash = Some(update.new_hash);
                    entry.cache_dir = Some(update.new_cache_dir);
                }
            }
        }

        // 5. Construct and return the lazy facade.
        let primary_entry = self.index.projects.get(&uuid).unwrap(); // Safe due to hierarchy check
        let qualified_name = crate::core::index_manager::build_qualified_name(uuid, self.index)
            .unwrap_or_else(|| primary_entry.name.clone());

        Ok(ResolvedConfig::new(
            uuid,
            qualified_name,
            primary_entry.path.clone(),
            hierarchy,
            layer_promises,
        ))
    }
}
