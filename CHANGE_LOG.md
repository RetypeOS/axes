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
  * A latency of +100ms in the execution of every command has been eliminated. The execution of simple scripts is now **~200% faster** (from ~150ms to ~40ms on average).
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

## v0.1.9-Alpha: Grammar Unification & Dynamic Execution

This version introduces a significant architectural refactoring of the command dispatcher and re-introduces the powerful `<axes::run::...>` dynamic execution token. The focus has been on creating a more predictable, robust, and ergonomic command-line experience.

### 🔥 Enhancements & New Features

* **Re-introduced Dynamic Execution with `<axes::run::...>`:**
  * The powerful `<axes::run::...>` token is back, allowing scripts to execute commands and substitute their standard output in-place.
  * It supports two distinct, unambiguous syntaxes:
    * **Script Reference:** `<axes::run::script_name>` executes another `axes` script and substitutes its output. The expansion engine validates that the script exists and is dependency-safe (no cycles) before execution.
    * **Literal Command:** `<axes::run('command string')>` executes an arbitrary shell command.
  * **Real-time Execution:** To ensure data is always fresh (e.g., getting a git hash), the output of `run` tokens is **never** persisted in the on-disk cache. The command is executed every time the parent script is run.

### 🏛️ Architecture & Refactorings

* **Universal Command Grammar:**
  * The core CLI dispatcher (`bin/axes.rs`) has been completely rewritten. It no longer has separate logic for "session" vs "non-session" mode.
  * A single, predictable set of rules now governs how commands are interpreted, dramatically simplifying the architecture. The primary grammars are `axes <context> <action>` and `axes <action> [args...]`, with a default fallback to `run`.

* **Implicit Context as Default:**
  * Commands that operate on a project (like `info`, `run`, `start`) now default to the implicit context (`.`) if no explicit context is provided.
  * This fixes the regression where running scripts inside a session (e.g., `axes build`) had stopped working. It now works seamlessly.

* **Increased Safety for Destructive Commands:**
  * To prevent accidental operations, destructive or refactoring commands (`delete`, `rename`, `link`, `unregister`) have been hardened and **now require an explicit project context**. They will no longer fall back to the implicit context.

* **Standardized Handler Signature:**
  * All command handlers have been refactored to a universal signature, decoupling them from the dispatcher and simplifying the overall architecture.

### Bug Fixes

* **Fixed Composite Token Expansion:** Corrected a critical regression in the expansion engine that prevented composite tokens like `<axes::scripts::...>` and `<axes::vars::...>` from being resolved, which caused widespread failures in scripts using composition.
* **Corrected Expansion Order:** Fixed a bug where simple static tokens (`<axes::path>`, `<axes::name>`, etc.) were not being expanded before the final parsing stage, leading to "malformed token" errors.

### ⚠ Breaking Changes v0.1.9

* **CLI Grammar Unification:** The command-line syntax has been made stricter and more predictable.
  * The shortcut `axes <context> <script_name>` is **no longer supported**.
  * To run a script on a specific, explicit context, the `run` command is now required: `axes <context> run <script_name>`.
  * The primary way to run a script in the current context (either from the filesystem or a session) is now the simpler, universal `axes <script_name> [args...]`.
