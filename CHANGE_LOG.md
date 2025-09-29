# `axes` Changelog

This document records all notable changes to `axes` for each version.

## v0.1.8-Alpha: Big refactor on task Engine

This version represents the most significant rewrite of the `axes` core to date. The execution engine and parameter system have been completely redesigned from the ground up to be more robust, efficient, and powerful, laying the foundation for the project's stable future.

### 🔥 Enhancements & New Features

* **New Unified Task Engine:**
  * All executable items (`scripts`, `vars`, `at_start`, `at_exit`, `open_with`) are now processed through a single, consistent "Task" engine.
  * Composition of scripts (`<axes::scripts::...>`) and variables (`<axes::vars::...>`) is now more robust and predictable.

* **Declarative Parameter System (Complete Rewrite):**
  * The argument parser has been replaced with a declarative `ArgResolver`.
  * Scripts now validate all parameters (`required`, etc.) **before** execution, providing clear and immediate errors for missing arguments or conflicts.
  * The syntax for modifiers (`map`, `default`, `alias`) has been standardized, and their behavior is now more intuitive and powerful.

* **Drastically Improved Performance:**
  * A latency of +100ms in the execution of every command has been eliminated. The execution of simple scripts is now **~200% faster** (from ~150ms to ~50ms on average).
  * The subprocess `executor` has been rewritten using `tokio` for high-performance, non-blocking waits.

* **Intelligent `Ctrl+C` Handling:**
  * Pressing `Ctrl+C` now safely interrupts the currently running subprocess without terminating `axes` entirely.
  * This allows for canceling long-running tasks within a sequential script without aborting the entire workflow.

* **Robust Persistent Caching:**
  * Fixed a serialization error (`bincode`) that prevented the lazy expansion cache from working correctly.
  * Subsequent runs of complex scripts are now nearly instantaneous, as they read the pre-expanded `Task` directly from the on-disk cache (`.axes/config.cache.bin`).

* **Contextual Error Traces:**
  * Errors during script expansion now provide a full trace (`Caused by:`), guiding the user to the exact line, token, and root cause of the problem in their `axes.toml`.

* **Parameters for `start` and `open`:**
  * The `axes <ctx> start` and `axes <ctx> open` commands can now accept additional parameters, which are passed to the `at_start`/`at_exit` hooks and `open_with` commands, respectively.

* **New Cache Debugging Tool:**
  * Added a hidden `_cache` subcommand (`axes <ctx> _cache inspect` and `axes <ctx> _cache clear`) to allow developers to inspect and manage the contents of a project's binary cache.

### ⚠ Breaking Changes

* The semantics of execution prefixes (`-`, `>`) have been clarified: they only take effect on the line of a `script` where they are defined. They are not "inherited" when one script is composed by another. The control of execution always belongs to the caller.
* The syntax of parameter tokens in scripts has been refined. Previous documentation may be outdated.
