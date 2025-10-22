// src/state.rs

use crate::core::index_manager;
use crate::models::GlobalIndex;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// Represents the state of the global index.
/// It holds the current state and, optionally, a snapshot of the original state
/// before the first mutation occurred.
enum IndexState {
    /// The state is clean, no mutations have been requested yet.
    Pristine(GlobalIndex),
    /// A mutation has been requested. We now hold both the original snapshot
    /// and the current, mutable state.
    Dirty {
        original: GlobalIndex,
        current: GlobalIndex,
    },
}

/// The main application state wrapper.
pub struct AppState {
    state: IndexState,
}

impl AppState {
    fn new(index: GlobalIndex) -> Self {
        Self {
            state: IndexState::Pristine(index),
        }
    }

    /// Checks if the state needs to be saved by comparing the current state
    /// against the original snapshot, if one exists.
    pub fn needs_saving(&self) -> bool {
        match &self.state {
            IndexState::Pristine(_) => false, // Never modified, no need to save.
            IndexState::Dirty { original, current } => original != current,
        }
    }

    /// Provides read-only access to the current index state.
    pub fn index(&self) -> &GlobalIndex {
        match &self.state {
            IndexState::Pristine(index) => index,
            IndexState::Dirty { current, .. } => current,
        }
    }
}

/// A custom MutexGuard that manages the state transition from Pristine to Dirty.
pub struct AppStateGuard<'a> {
    guard: MutexGuard<'a, AppState>,
}

// Implement Deref for easy read-only access.
impl<'a> Deref for AppStateGuard<'a> {
    type Target = GlobalIndex;

    fn deref(&self) -> &Self::Target {
        self.guard.index()
    }
}

// Implement DerefMut for controlled mutable access. This is where the magic happens.
impl<'a> DerefMut for AppStateGuard<'a> {
    fn deref_mut(&mut self) -> &mut GlobalIndex {
        // Ensure the state is transitioned to Dirty before handing out a mutable reference.
        // This is the core of the "journaling" logic.
        if let IndexState::Pristine(_) = self.guard.state {
            // This is the first request for mutable access.
            // Atomically swap the Pristine state with a new Dirty state.
            self.guard.state = match std::mem::replace(&mut self.guard.state, IndexState::Pristine(GlobalIndex::default())) {
                IndexState::Pristine(index) => IndexState::Dirty {
                    original: index.clone(), // The one and only clone operation happens here!
                    current: index,
                },
                _ => unreachable!(),
            };
        }

        // Now we are guaranteed to be in the Dirty state.
        match &mut self.guard.state {
            IndexState::Dirty { current, .. } => current,
            _ => unreachable!(),
        }
    }
}

static APP_STATE: OnceLock<Arc<Mutex<AppState>>> = OnceLock::new();

pub fn get_app_state() -> &'static Arc<Mutex<AppState>> {
    APP_STATE.get_or_init(|| {
        let index =
            index_manager::load_and_ensure_global_project().expect("Failed to load global index.");
        Arc::new(Mutex::new(AppState::new(index)))
    })
}

// This function now returns a guard that implements Deref and DerefMut transparently.
pub fn lock_app_state() -> AppStateGuard<'static> {
    let state_arc = get_app_state();
    let guard = state_arc.lock().unwrap();
    AppStateGuard { guard }
}