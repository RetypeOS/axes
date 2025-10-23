//! # System Interaction Layer
//!
//! This module provides abstractions for interacting with the underlying operating system.
//! It serves as a boundary between the core application logic and the specifics of process
//! management, shell environments, and configuration files.
//!
//! ## Modules
//!
//! - **`executor`**: A robust, high-performance engine for spawning and managing external
//!   processes. It handles graceful cancellation (`Ctrl+C`), platform-specific command
//!   execution (e.g., `cmd.exe` on Windows), and output capturing.
//! - **`shell`**: Manages the lifecycle of an interactive project session (`axes start`). It's
//!   responsible for generating init scripts, setting up the environment, and launching the
//!   user's configured shell.
//! - **`shells_config`**: Handles the loading and parsing of the `shells.toml` file, which
//!   defines the shells available for `axes start` sessions.

pub mod executor;
pub mod shell;
pub mod shells_config;
