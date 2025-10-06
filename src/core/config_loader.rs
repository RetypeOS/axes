// EN: src/core/config_loader.rs (FINAL VERSION)

use crate::{
    core::config_resolver,
    models::{GlobalIndex, LayerPromise, LayerResult, ResolvedConfig},
};
use anyhow::{Result, anyhow};
use log;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock}; // Import Mutex
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

        // 1. Determine the full inheritance hierarchy (unchanged).
        let mut hierarchy = Vec::new();
        let mut current_uuid = Some(uuid);
        while let Some(id) = current_uuid {
            hierarchy.push(id);
            let entry = self
                .index
                .projects
                .get(&id)
                .ok_or_else(|| anyhow!("Broken parent link in hierarchy for UUID {}", id))?;
            current_uuid = entry.parent;
        }

        // 2. Prepare promises and a thread-safe container for index updates.
        let layer_promises: HashMap<Uuid, LayerPromise> = hierarchy
            .iter()
            .map(|&id| (id, Arc::new(OnceLock::new())))
            .collect();

        let index_updates = Arc::new(Mutex::new(Vec::new()));

        let index_ref = &*self.index;

        rayon::scope(|s| {
            for &layer_uuid in &hierarchy {
                let promise = layer_promises.get(&layer_uuid).unwrap().clone();
                let updates_clone = index_updates.clone();

                s.spawn(move |_| {
                    log::trace!("Spawned load task for UUID: {}", layer_uuid);
                    // Llama a la tarea de carga
                    let task_result = config_resolver::load_layer_task(layer_uuid, index_ref);

                    // --- CORRECTED LOGIC ---
                    // Ahora manejamos el resultado explícitamente y llenamos la promesa
                    // con el tipo correcto `LayerResult`.

                    let layer_result_for_promise: LayerResult;

                    match task_result {
                        Ok((layer_arc, Some(update))) => {
                            // Carga exitosa, con actualización de índice
                            layer_result_for_promise = Ok(layer_arc);
                            updates_clone.lock().unwrap().push(update);
                        }
                        Ok((layer_arc, None)) => {
                            // Carga exitosa, sin actualización
                            layer_result_for_promise = Ok(layer_arc);
                        }
                        Err(e) => {
                            // La tarea de carga falló
                            layer_result_for_promise = Err(e);
                        }
                    }

                    // `set` solo puede fallar si la celda ya está llena, lo cual es imposible aquí.
                    if promise.set(layer_result_for_promise).is_err() {
                        log::error!("CRITICAL: LayerPromise for {} was set twice.", layer_uuid);
                    }
                });
            }
        });

        // 4. Apply all collected index updates sequentially (unchanged).
        // By this point, all spawned tasks in the scope have completed.
        let updates_to_apply = Arc::try_unwrap(index_updates)
            .expect("Mutex should not be locked elsewhere")
            .into_inner()
            .unwrap();

        if !updates_to_apply.is_empty() {
            log::debug!(
                "Applying {} updates to the global index.",
                updates_to_apply.len()
            );
            for update in updates_to_apply {
                if let Some(entry) = self.index.projects.get_mut(&update.uuid) {
                    entry.config_hash = Some(update.new_hash);
                    entry.cache_dir = Some(update.new_cache_dir);
                }
            }
        }

        // 5. Construct and return the lazy facade (unchanged).
        let primary_entry = self.index.projects.get(&uuid).unwrap();
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
