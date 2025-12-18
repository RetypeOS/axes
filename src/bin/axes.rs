//! # `axes` Main Entry Point
//!
//! This binary file serves as the main entry point for the `axes` application.
//! Its primary responsibilities are minimal and well-defined:
//!
//! 1.  **Initialization**: It initializes logging (in debug builds) and parses command-line
//!     arguments using `clap`.
//! 2.  **Execution**: It orchestrates the core application logic by calling `run_app`, which
//!     in turn delegates to the command dispatcher.
//! 3.  **Error Handling**: It provides a centralized, user-friendly error reporting mechanism
//!     for the entire application, handling specific cases like `clap`'s help/version exits
//!     and Ctrl+C interruptions.
//! 4.  **State Persistence**: After the application logic has completed, it efficiently checks
//!     if the global state has been modified (using the journaling state manager) and, if so,
//!     saves the updated state back to disk.
//!
//! This lean structure ensures that the application's entry point is simple and focused on
//! orchestration, while the complex logic is delegated to other modules.

use anyhow::Result;
use axes::{
    cli::{dispatcher, Cli}, core::index_manager, dev_utils, state::get_app_state, system::executor
};
use clap::Parser;
use colored::*;

/// The main entry point of the `axes` application.
fn main() {
    let _timer_total = dev_utils::BlockTimer::new("Total Execution");
    let cli = Cli::parse();

    #[cfg(debug_assertions)]
    {
        env_logger::init();
    }

    // --- Application Logic Execution ---
    // The core logic is now encapsulated and called here.
    let _timer_logic = dev_utils::BlockTimer::new("Core App Logic");
    if let Err(e) = run_app(cli) {
        // --- Graceful Error Handling (Unchanged) ---
        if let Some(clap_err) = e.downcast_ref::<clap::Error>()
            && !clap_err.use_stderr()
        {
            clap_err.print().expect("Failed to print clap help/version");
            std::process::exit(0);
        }

        if let Some(exec_err) = e.downcast_ref::<executor::ExecutionError>()
            && matches!(exec_err, executor::ExecutionError::Interrupted { .. })
        {
            eprintln!();
            std::process::exit(130); // Standard exit code for Ctrl+C
        }

        eprintln!("\n{}: {}", "Error".red().bold(), e);
        let mut causes = e.chain().skip(1);
        if let Some(cause) = causes.next() {
            eprintln!("\nCaused by:");
            eprintln!("   0: {}", cause);
            for (i, cause) in causes.enumerate() {
                eprintln!("   {}: {}", i + 1, cause);
            }
        }
        std::process::exit(1);
    }

    // --- State Saving Logic (Now highly efficient) ---
    // We only acquire the lock once at the very end.
    let state_arc = get_app_state();
    let state_guard = state_arc
        .lock()
        .expect("The main AppState mutex is poisoned, indicating a catastrophic failure.");

    // The MutexGuard allows us to access the methods of the inner AppState directly.
    // We call AppState::needs_saving() via the guard.
    if state_guard.needs_saving() {
        let _timer_save = dev_utils::BlockTimer::new("State Saving");
        // We call AppState::index() via the guard to get a reference to the GlobalIndex
        // that index_manager::save_global_index expects.
        if let Err(e) = index_manager::save_global_index(state_guard.index()) {
            eprintln!(
                "\n{}: Failed to save updated global index: {}",
                "Critical Error".red().bold(),
                e
            );
            std::process::exit(1);
        }
        log::debug!("Global index was modified and has been saved.");
    }
}

/// A wrapper function that contains the core application logic.
///
/// It acquires a lock on the global application state and passes it to the
/// command dispatcher. This cleanly separates the application's "business logic"
/// from the state persistence and error handling concerns in `main`.
///
/// # Arguments
/// * `cli` - The parsed command-line arguments.
fn run_app(cli: Cli) -> Result<()> {
    // Get a mutable guard to the global index.
    // This guard will automatically set the dirty flag if any handler mutates the index.
    let mut index_guard = axes::state::lock_app_state();

    // Delegate entirely to the new dispatcher.
    dispatcher::dispatch(cli.args, &mut index_guard)
}
