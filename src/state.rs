// src/state.rs

use crate::core::index_manager;
use crate::models::GlobalIndex;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use uuid::Uuid;

// --- Public Structs & Enums ---

/// Manages the application's global state using a high-performance journaling system.
///
/// This struct acts as an intelligent gatekeeper to the `GlobalIndex`. It avoids the high cost
/// of cloning the index on every run by instead keeping a snapshot of the original state *only*
/// when a mutation is first requested. At the end of the application lifecycle, it can
/// efficiently determine if a save-to-disk operation is truly necessary by comparing the final
/// state against the initial snapshot.
pub struct AppState {
    state: IndexState,
}

/// A custom `MutexGuard` that provides a controlled, explicit API for state interaction.
///
/// Instead of transparently dereferencing, this guard requires consumers to explicitly
/// request read-only (`.index()`) or mutable (`.index_mut()`) access. This makes the intent
/// of the code clearer and enables fine-grained, intelligent updates that minimize performance
/// overhead.
pub struct AppStateGuard<'a> {
    guard: MutexGuard<'a, AppState>,
}

// --- Private Implementation Details ---

/// An internal enum representing the two possible conditions of the `GlobalIndex`.
enum IndexState {
    /// The index has not been touched. It contains the state as it was loaded from disk.
    /// This is the default state for read-only commands.
    Pristine(GlobalIndex),
    /// A mutable operation has been requested. The state now holds both a clone of the
    /// original state and the current, modified state.
    Dirty {
        original: GlobalIndex,
        current: GlobalIndex,
    },
}

// --- Implementations ---

impl AppState {
    /// Creates a new `AppState` instance, starting in a `Pristine` state.
    fn new(index: GlobalIndex) -> Self {
        Self {
            state: IndexState::Pristine(index),
        }
    }

    /// Determines if the index has changed and needs to be saved to disk.
    ///
    /// This is a highly efficient check. If the state is `Pristine`, it returns `false`
    /// instantly. If `Dirty`, it performs a deep comparison between the original snapshot
    /// and the current state.
    pub fn needs_saving(&self) -> bool {
        match &self.state {
            IndexState::Pristine(_) => false,
            IndexState::Dirty { original, current } => original != current,
        }
    }

    /// Returns a read-only reference to the current `GlobalIndex`.
    /// This is used by `main.rs` to save the final state if `needs_saving` returns true.
    pub fn index(&self) -> &GlobalIndex {
        match &self.state {
            IndexState::Pristine(index) => index,
            IndexState::Dirty { current, .. } => current,
        }
    }

    /// Transitions the state to `Dirty` if it's currently `Pristine`.
    ///
    /// This is the core of the journaling mechanism. It is called only on the first
    /// request for mutable access. It performs the single, lazy clone operation required
    /// to enable change tracking.
    fn ensure_dirty_and_get_mut(&mut self) -> &mut GlobalIndex {
        if let IndexState::Pristine(_) = self.state {
            // Atomically replace the state to move ownership of the index.
            self.state = match std::mem::replace(
                &mut self.state,
                IndexState::Pristine(GlobalIndex::default()), // Temporary placeholder
            ) {
                IndexState::Pristine(index) => IndexState::Dirty {
                    original: index.clone(), // The one and only clone operation!
                    current: index,
                },
                _ => unreachable!(), // Should never happen
            };
        }

        match &mut self.state {
            IndexState::Dirty { current, .. } => current,
            _ => unreachable!(),
        }
    }
}

impl<'a> AppStateGuard<'a> {
    /// Returns a read-only reference to the `GlobalIndex`.
    /// This operation is always cheap and never triggers a state change.
    pub fn index(&self) -> &GlobalIndex {
        self.guard.index()
    }

    /// Returns a mutable reference to the `GlobalIndex`.
    ///
    /// This is the primary entry point for structural modifications (e.g., adding/deleting
    /// projects). Calling this method will trigger the `Pristine` -> `Dirty` state
    /// transition on its first invocation.
    pub fn index_mut(&mut self) -> &mut GlobalIndex {
        self.guard.ensure_dirty_and_get_mut()
    }

    /// Intelligently updates a project's cache metadata (`config_hash`, `cache_dir`).
    ///
    /// It performs read-only checks first and only requests mutable access if the new
    /// data is different from the existing data, preventing unnecessary state clones.
    pub fn update_project_cache_info(
        &mut self,
        uuid: Uuid,
        new_hash: Option<String>,
        new_cache_dir: Option<PathBuf>,
    ) {
        let index = self.guard.index();
        let project = match index.projects.get(&uuid) {
            Some(p) => p,
            None => return,
        };

        let hash_changed = new_hash
            .as_ref()
            .is_some_and(|h| project.config_hash.as_ref() != Some(h));
        let dir_changed = new_cache_dir
            .as_ref()
            .is_some_and(|d| project.cache_dir.as_ref() != Some(d));

        if hash_changed || dir_changed {
            let mutable_project = self
                .guard
                .ensure_dirty_and_get_mut()
                .projects
                .get_mut(&uuid)
                .unwrap();
            if hash_changed {
                mutable_project.config_hash = new_hash;
            }
            if dir_changed {
                mutable_project.cache_dir = new_cache_dir;
            }
        }
    }

    /// Intelligently updates `last_used` and `last_used_child` metadata.
    ///
    // It first performs a series of read-only checks to determine if any updates are
    // necessary. It only requests mutable access if a change is guaranteed to happen,
    // avoiding unnecessary clone operations for idempotent updates.
    pub fn update_last_used_caches(&mut self, final_uuid: Uuid) {
        // Phase 1: Read-Only Analysis
        let mut needs_update = false;
        let index = self.guard.index();

        if index.last_used != Some(final_uuid) {
            needs_update = true;
        } else {
            let mut child_uuid = final_uuid;
            let mut parent_uuid_opt = index.projects.get(&child_uuid).and_then(|e| e.parent);

            while let Some(parent_uuid) = parent_uuid_opt {
                if let Some(parent) = index.projects.get(&parent_uuid) {
                    if parent.last_used_child != Some(child_uuid) {
                        needs_update = true;
                        break;
                    }
                    child_uuid = parent_uuid;
                    parent_uuid_opt = parent.parent;
                } else {
                    break;
                }
            }
        }

        // Phase 2: Conditional Write Operation
        if !needs_update {
            log::trace!("`last_used` metadata is already up-to-date. Skipping write.");
            return;
        }

        log::debug!("`last_used` metadata needs updating. Performing write operation.");
        let mutable_index = self.guard.ensure_dirty_and_get_mut();

        mutable_index.last_used = Some(final_uuid);

        let mut child_uuid_to_save = final_uuid;
        let mut current_uuid_opt = mutable_index
            .projects
            .get(&child_uuid_to_save)
            .and_then(|e| e.parent);

        while let Some(parent_uuid) = current_uuid_opt {
            if let Some(parent_entry) = mutable_index.projects.get_mut(&parent_uuid) {
                parent_entry.last_used_child = Some(child_uuid_to_save);
                child_uuid_to_save = parent_uuid;
                current_uuid_opt = parent_entry.parent;
            } else {
                break;
            }
        }
    }
}

// --- Static Global State ---

/// The single, global, lazily-initialized application state.
static APP_STATE: OnceLock<Arc<Mutex<AppState>>> = OnceLock::new();

/// Returns a reference to the global `AppState` singleton.
/// It handles the initial loading and creation of the state on the first call.
pub fn get_app_state() -> &'static Arc<Mutex<AppState>> {
    APP_STATE.get_or_init(|| {
        let index =
            index_manager::load_and_ensure_global_project().expect("Failed to load global index.");
        Arc::new(Mutex::new(AppState::new(index)))
    })
}

/// Acquires a lock on the global state and returns our custom `AppStateGuard`.
/// This is the primary entry point for all state interactions within the application logic.
pub fn lock_app_state() -> AppStateGuard<'static> {
    let state_arc = get_app_state();
    let guard = state_arc.lock().unwrap();
    AppStateGuard { guard }
}
