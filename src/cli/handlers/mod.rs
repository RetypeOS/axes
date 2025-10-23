//! # Command Handlers Module
//!
//! This module acts as a central directory for all the sub-modules that implement the
//! logic for `axes` commands. Each file within this module corresponds to a specific
//! command (e.g., `run.rs` handles `axes run`, `init.rs` handles `axes init`).
//!
//! ## Module Structure
//!
//! - **`mod.rs`**: This file, which simply declares all other handler modules to make them
//!   accessible to the rest of the application.
//! - **`<command>.rs`**: Each file contains the primary `handle` function, which serves as the
//!   entry point for that command's logic, along with any necessary helper functions, `clap`
//!   argument structs, and other implementation details specific to that command.
//! - **`commons.rs`**: A special module that contains shared utility functions and data
//!   structures used by multiple handlers to promote code reuse (DRY) and ensure consistent
//!   behavior across the application.

pub mod alias;
pub mod commons;
pub mod debug_cache;
pub mod delete;
pub mod info;
pub mod init;
pub mod link;
pub mod open;
pub mod register;
pub mod rename;
pub mod repair;
pub mod run;
pub mod start;
pub mod tree;
pub mod unregister;
