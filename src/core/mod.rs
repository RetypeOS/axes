//! # Core
//!
//! This module contains the main logic of the application, including caching, color management,
//! commons, compiler, config loader, context resolver, graph display, index manager,
//! onboarding manager, parameters, paths, and task executor.

/// The cache module, responsible for caching configurations and other data.
pub mod cache;

/// The color module, which provides utilities for colorizing output.
pub mod color;

/// The commons module, which contains common utilities and data structures.
pub mod commons;

/// The compiler module, responsible for compiling `axes.toml` files into a serializable format.
pub mod compiler;

/// The config_loader module, which loads and resolves configurations.
pub mod config_loader;

/// The context_resolver module, which resolves the current context of the application.
pub mod context_resolver;

/// The graph_display module, which provides utilities for displaying graphs.
pub mod graph_display;

/// The index_manager module, which manages the global index of projects.
pub mod index_manager;

/// The onboarding_manager module, which manages the onboarding process for new users.
pub mod onboarding_manager;

/// The parameters module, which handles the parsing and resolution of parameters.
pub mod parameters;

/// The paths module, which provides utilities for working with file paths.
pub mod paths;

/// The task_executor module, which executes tasks defined in the configuration.
pub mod task_executor;
