//! # Config Loader
//!
//! This module provides the `ConfigLoader` struct, which is responsible for orchestrating the loading
//! of configuration layers for a project hierarchy. It handles both registered and ephemeral projects,
//! leveraging parallel processing and caching to ensure high performance.
use crate::{
    core::{compiler, index_manager, paths}, dev_utils, models::{GlobalIndex, IndexEntry, LayerPromise, ResolvedConfig}
};
use anyhow::{Result, anyhow};
use log;
use rayon::prelude::*;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
use uuid::Uuid;

/// Orchestrates the loading of configuration layers for a project hierarchy.
#[derive(Debug)]
pub struct ConfigLoader<'a> {
    index: &'a mut GlobalIndex,
}

impl<'a> ConfigLoader<'a> {
    /// Creates a new `ConfigLoader`.
    ///
    /// # Arguments
    ///
    /// * `index` - A mutable reference to the `GlobalIndex`.
    pub fn new(index: &'a mut GlobalIndex) -> Self {
        Self { index }
    }

    /// The main entry point for configuration resolution for a registered project.
    ///
    /// This function orchestrates a complex, multi-stage process designed for maximum
    /// performance and correctness:
    ///
    /// 1. **Hierarchy Discovery:** It traverses the `GlobalIndex` to determine the full
    ///    inheritance chain for the requested project UUID.
    ///
    /// 2. **Parallel Loading & Compilation:** It spawns a parallel task via `rayon` for
    ///    each project in the hierarchy. Each task (`compiler::load_layer_task`) is
    ///    responsible for:
    ///    a. Checking if a valid binary cache exists for its `axes.toml` file.
    ///    b. If a `Cache Miss` occurs, it compiles the `axes.toml` into a `CachedProjectConfig` (AST)
    ///    and writes it to a new binary cache file.
    ///
    /// 3. **State Synchronization:**
    ///    a. **Hashes:** If any layers were recompiled, their new content hashes are collected
    ///    and updated in the in-memory `GlobalIndex`.
    ///    b. **Cache Path:** After all layers are loaded, it constructs the `ResolvedConfig` facade.
    ///    It then uses this facade to determine the *final, inherited `cache_dir` path*.
    ///    If this final path differs from what's stored in the index, the index is updated.
    ///    This ensures the next run will find the cache in the correct, inherited location.
    ///
    /// 4. **Cache File Migration:** If a re-compilation occurred *and* the final `cache_dir`
    ///    path changed, this function will atomically move the newly created cache file from
    ///    the old path to the new, correct path.
    ///
    /// 5. **Facade Construction:** Finally, it returns a `ResolvedConfig` instance, a lazy
    ///    facade that provides on-demand access to the fully merged configuration without
    ///    further blocking or I/O.
    ///
    /// # Arguments
    ///
    /// * `uuid` - The UUID of the project to resolve.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `ResolvedConfig` on success, or an error if resolution fails.
    pub fn resolve(&mut self, uuid: Uuid) -> Result<ResolvedConfig> {
        let _timer = dev_utils::BlockTimer::new("    --> ConfigLoader::resolve");
        log::debug!("ConfigLoader resolving UUID: {}", uuid);

        // --- 1. Hierarchy Discovery ---
        let mut hierarchy = Vec::new();
        let _timer_h = dev_utils::BlockTimer::new("      ---> Hierarchy Discovery");
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

        // --- 2. Parallel Loading & Compilation ---
        let layer_promises: HashMap<Uuid, LayerPromise> = hierarchy
            .par_iter()
            .map(|&id| (id, Arc::new(OnceLock::new())))
            .collect();

        let hash_updates = Arc::new(Mutex::new(Vec::new()));
        let index_ref: &GlobalIndex = self.index;

        let _timer_pl = dev_utils::BlockTimer::new("      ---> Parallel Layer Loading");
        hierarchy.par_iter().for_each(|&layer_uuid| {
            let promise = layer_promises
                .get(&layer_uuid)
                .expect("Layer UUID must be in the promises map");
            log::trace!("Executing load task for UUID: {}", layer_uuid);

            let task_result = compiler::load_layer_task(layer_uuid, index_ref);

            let layer_result_for_promise = match task_result {
                Ok((layer_arc, Some(update))) => {
                    // On a cache miss, we get a hash update.
                    hash_updates
                        .lock()
                        .expect("Mutex should not be poisoned")
                        .push(update);
                    Ok(layer_arc)
                }
                Ok((layer_arc, None)) => Ok(layer_arc), // Cache hit
                Err(e) => Err(e),
            };

            if promise.set(layer_result_for_promise).is_err() {
                log::error!("CRITICAL: LayerPromise for {} was set twice.", layer_uuid);
            }
        });

        // --- 3. State Synchronization ---
        let primary_entry = self
            .index
            .projects
            .get(&uuid)
            .expect("Primary project UUID must exist in the index");
        let qualified_name = crate::core::index_manager::build_qualified_name(uuid, self.index)
            .unwrap_or_else(|| primary_entry.name.clone());

        // Step 3a: Construct the facade *before* final index updates.
        let resolved_config = ResolvedConfig::new(
            uuid,
            qualified_name,
            primary_entry.path.clone(),
            hierarchy,
            layer_promises,
        );

        // Step 3b: Determine the final, inherited cache path (as a String).
        let final_options = resolved_config.get_options()?;
        let final_cache_path_str = final_options.cache_dir.ok_or_else(|| {
            anyhow!("Internal logic error: ResolvedConfig failed to produce a cache_dir.")
        })?;

        let final_cache_path = PathBuf::from(&final_cache_path_str);

        // Step 3c: Collect hash updates from the parallel tasks.
        let updates_to_apply = Arc::try_unwrap(hash_updates)
            .expect("Mutex should not be locked elsewhere")
            .into_inner()
            .expect("Mutex should not be poisoned");

        // Step 3d: Apply updates to the in-memory index.
        let entry = self
            .index
            .projects
            .get_mut(&uuid)
            .expect("Primary project UUID must exist in the index");
        let mut cache_path_changed = false;

        if entry.cache_dir.as_ref() != Some(&final_cache_path) {
            log::debug!(
                "Updating stale cache_dir for project {} in index. Old: {:?}, New: {}",
                uuid,
                entry.cache_dir,
                final_cache_path.display() // .display() works on PathBuf. Correct.
            );
            entry.cache_dir = Some(final_cache_path.clone()); // .clone() on PathBuf. Correct.
            cache_path_changed = true;
        }

        let mut new_hash = None;
        if let Some(update) = updates_to_apply.iter().find(|u| u.uuid == uuid) {
            entry.config_hash = Some(update.new_hash.clone());
            new_hash = Some(update.new_hash.clone());
        }

        // --- 4. Cache File Migration ---
        // If the toml was recompiled AND the final cache path has changed, we must move the file.
        if let (true, Some(hash)) = (cache_path_changed, new_hash) {
            // The `load_layer_task` wrote the cache file to the *old* indexed path. We need to move it.
            let old_cache_dir = resolved_config
                .get_layer(uuid)?
                .options
                .cache_dir
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    paths::get_default_cache_dir_for_project(uuid)
                        .expect("Default cache dir should always be resolvable")
                });

            let old_path = old_cache_dir.join(&hash);
            let new_path = final_cache_path.join(&hash);

            if old_path != new_path && old_path.exists() {
                log::info!(
                    "Cache directory for project '{}' has changed. Migrating cache file from {} to {}",
                    resolved_config.qualified_name,
                    old_path.display(),
                    new_path.display()
                );
                // Ensure parent directory of the new path exists.
                if let Some(parent) = new_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                if let Err(e) = std::fs::rename(&old_path, &new_path) {
                    log::error!(
                        "Failed to migrate cache file: {}. A new cache will be generated on the next run.",
                        e
                    );
                    // Attempt to clean up the old file if rename fails, but don't panic.
                    let _ = std::fs::remove_file(old_path);
                }
            }
        }

        // --- 5. Return Facade ---
        Ok(resolved_config)
    }

    /// Resolves a project configuration ephemerally from the filesystem.
    /// It reads the local `project_ref.bin` to find its identity and parent,
    /// then uses the global index ONLY to resolve the inheritance chain of its parents.
    /// It does not perform any cache checks or index updates for the ephemeral project itself.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the ephemeral project.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `ResolvedConfig` on success, or an error if resolution fails.
    pub fn resolve_ephemeral(&mut self, path: &Path) -> Result<ResolvedConfig> {
        log::debug!(
            "ConfigLoader resolving ephemerally from path: {}",
            path.display()
        );

        let canonical_path = dunce::canonicalize(path)?;

        // 1. Read local identity. If this fails, we can't proceed.
        let project_ref = index_manager::read_project_ref(&canonical_path)
            .map_err(|_| anyhow!("No '.axes/project_ref.bin' found at '{}'. An ephemeral project must have a local identity file.", canonical_path.display()))?;

        // 2. Build the hierarchy: self (from project_ref) + parents (from index).
        let mut hierarchy = vec![project_ref.self_uuid];
        let mut current_uuid_opt = project_ref.parent_uuid;
        while let Some(id) = current_uuid_opt {
            let entry = self.index.projects.get(&id)
                .ok_or_else(|| anyhow!("Ephemeral project at '{}' depends on parent UUID '{}', which is not registered in the global index.", canonical_path.display(), id))?;
            hierarchy.push(id);
            current_uuid_opt = entry.parent;
        }

        // 3. Prepare promises for parallel loading.
        let layer_promises: HashMap<_, _> = hierarchy
            .iter()
            .map(|&id| (id, Arc::new(OnceLock::new())))
            .collect();
        let index_ref = &*self.index;

        // Create a temporary, in-memory IndexEntry for the ephemeral project.
        let ephemeral_entry = IndexEntry {
            name: project_ref.name.clone(),
            path: canonical_path.clone(),
            parent: project_ref.parent_uuid,
            ..Default::default()
        };

        let ephemeral_entry_arc = Arc::new(ephemeral_entry);

        rayon::scope(|s| {
            for &layer_uuid in &hierarchy {
                let promise = layer_promises
                    .get(&layer_uuid)
                    .expect("Layer UUID from hierarchy must be in promises map")
                    .clone();

                let ephemeral_entry_clone = ephemeral_entry_arc.clone();

                s.spawn(move |_| {
                    let task_result = if layer_uuid == project_ref.self_uuid {
                        // For the ephemeral project, COMPILE a fresh layer. NO CACHING.
                        log::trace!("Compiling ephemeral layer for UUID: {}", layer_uuid);
                        // The thread now uses its cloned Arc.
                        compiler::load_and_compile_layer(&ephemeral_entry_clone)
                            .map(|layer| (Arc::new(layer), None))
                    } else {
                        // For registered parents, use the NORMAL cached loading task.
                        log::trace!("Loading parent layer from index for UUID: {}", layer_uuid);
                        compiler::load_layer_task(layer_uuid, index_ref)
                    };

                    let layer_result_for_promise = match task_result {
                        Ok((layer_arc, _)) => Ok(layer_arc),
                        Err(e) => Err(e),
                    };

                    promise
                        .set(layer_result_for_promise)
                        .expect("Promise should only be set once");
                });
            }
        });

        // 5. Construct and return the lazy facade.
        // Use a special qualified name to indicate its ephemeral nature in logs/UI.
        let qualified_name = format!("_{}", project_ref.name);

        Ok(ResolvedConfig::new(
            project_ref.self_uuid,
            qualified_name,
            canonical_path,
            hierarchy,
            layer_promises,
        ))
    }
}
