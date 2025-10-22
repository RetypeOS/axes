// src/bin/axes.rs

use anyhow::Result;
use axes::{
    cli::{Cli, dispatcher},
    core::index_manager,
    state::get_app_state,
    system::executor,
};
use clap::Parser;
use colored::*;

/// The main entry point of the `axes` application.
fn main() {
    let cli = Cli::parse();

    #[cfg(debug_assertions)]
    {
        // Keep env_logger initialization here.
        env_logger::init();
    }

    // --- Application Logic Execution ---
    // The core logic is now encapsulated and called here.
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
    let state_guard = state_arc.lock().unwrap();

    // The check is now a cheap, instantaneous boolean read. No cloning!
    if state_guard.needs_saving() {
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

/// A new function to contain the application logic, separating it from state management.
fn run_app(cli: Cli) -> Result<()> {
    // Get a mutable guard to the global index.
    // This guard will automatically set the dirty flag if any handler mutates the index.
    let mut index_guard = axes::state::lock_app_state();

    // Delegate entirely to the new dispatcher.
    dispatcher::dispatch(cli.args, &mut index_guard)
}
