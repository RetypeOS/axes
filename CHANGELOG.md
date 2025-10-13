# `axes` Changelog

This document records all notable changes to `axes` for each version.

---

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

---

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

---

## v0.2.0-beta: Performance Overhaul & Architectural Tuning

This version marks a significant leap forward in performance, bringing `axes`'s hot-execution speed nearly on par with minimalist task runners like `just`. This was achieved through a deep refactoring of the core expansion engine, along with key architectural improvements.

### 🔥 Performance & Optimizations

* **Drastic Performance Improvement:**
  * The core script expansion engine has been completely rewritten to use a **direct structural composition** algorithm, eliminating a major performance bottleneck caused by unnecessary string allocations and re-parsing.
  * Internal latency has been reduced to **~17-20ms**.
  * Hot script execution (with a warm cache) is now consistently in the **~35-37ms** range, making the overhead of `axes` virtually imperceptible for most tasks.

### 🏛️ Architecture & Refactorings

* **Unified Command Grammar & Dispatcher:**
  * The CLI dispatcher has been refactored to use a universal, predictable grammar, removing the distinction between "session" and "non-session" modes for command interpretation.
  * This simplifies the user experience: `axes <script_name>` now works universally to run a script in the current context.
* **Session-Aware Context Resolution:**
  * The `context_resolver` is now fully session-aware. Relative paths (`.`, `..`, `sub-project`) are correctly resolved from the active session project, while absolute contexts (aliases like `g!`, `global`) provide a reliable "escape hatch".
* **Re-introduction of `<axes::run::...>`:**
  * The dynamic execution token is back with a robust, unambiguous syntax: `<axes::run('literal command')>`.
  * The `::script_name` variant has been temporarily disabled to prevent complex re-entrancy issues, ensuring stability.

### ⚠ Breaking Changes

* **CLI Grammar Unification:**
  * The shortcut `axes <context> <script_name>` is **no longer supported**. To run a script on an explicit, different context, the `run` command is now required: `axes <context> run <script_name>`.

---

## v0.2.1-beta: Performance Optimization and Architectural Refinement

This release represents a monumental leap in performance and robustness, culminating a series of deep refactorings of the expansion engine and command grammar. `axes` is now faster, smarter, and significantly more intuitive. We have achieved the goal of offering advanced orchestration capabilities with speed that rivals and surpasses the most minimalist task runners.

### 🔥 Performance and Optimizations

* **Elite Performance — Now Faster than `just`:**
  * The script expansion engine has been completely rewritten to use a **Direct Structural Composition (Pure AST)** algorithm. This eliminates a critical bottleneck caused by unnecessary memory allocations and text re-parsing, resolving a major performance regression.
  * Internal latency on "hot" runs (with cache) has been reduced to **~17-20ms**.
  * Simple script execution on a hot run is now consistently around **~35ms**, beating benchmarks of tools like `just` (~38ms).

### 🏛️ Architecture and Refactorings

* **"Pure AST" Expansion Architecture:**
  * The engine no longer expands scripts into raw text. Instead, it "compiles" `axes.toml` files into a pure **Abstract Syntax Tree (AST)** (`Task`), where all tokens (`<axes::path>`, `<axes::params::...>`, etc.) are preserved in their symbolic form.
  * The binary cache now stores this AST, making deserialization nearly instantaneous.
  * The `task_executor` acts as an "interpreter" or "renderer" that converts the AST into an executable command at runtime.
* **Explicit Resolver Entry Points:**
  * The use of "synthetic keys" (e.g., `options.open_with.key`) has been eliminated. There are now explicit functions (`resolve_script_task`, `resolve_open_with_task`, `resolve_hook_task`), making the architecture more robust and decoupled.

### ✨ Enhancements and New Features

* **Improved Execution Prefixes:**
  * The prefix parser has been rewritten to be more robust, efficient (no allocations), and to handle combinations like `>-`.
  * The `@` prefix has been added to run a command in **silent mode** (the `→ command...` output is suppressed).
  * The `#` prefix has been added to treat a line as a **direct console print** (`println!`), avoiding the overhead of executing an `echo` subprocess.
* **Handling of `--` Separator:** Argument parsing has been fixed to correctly handle the `--` separator, ensuring that arguments passed to underlying commands (e.g., `cargo clippy -- -D warnings`) are preserved intact.

### 🐛 Critical Bug Fixes

* **Fixed Positional Parameters Bug:** A fundamental bug in the `ArgResolver` that caused positional parameters (`<axes::params::0>`) to fail resolution when used multiple times or through composite variables has been corrected. The parameter system is now fully robust.
* **Corrected Prefix Propagation:** A bug that prevented execution prefixes (`-`, `>`, etc.) from being applied to scripts in `open_with`, `at_start`, and `at_exit` has been fixed. All script types now behave consistently.

### ⚠ Breaking Changes

* *(No further breaking changes since `v0.2.0-beta`)*
  * The **Universal Command Grammar** is maintained: The shortcut `axes <context> <script>` is no longer supported. You must use the explicit form `axes <context> run <script>`.

--- START OF FILE ROADMAP.en.md ---

## v0.2.2-beta: Core Refactoring and Lazy Loading Architecture

This release represents the final transition to a fully lazy, concurrent, and pre-compiled AST-based configuration loading architecture. The goal has been to eliminate I/O from the "hot path" of execution, resulting in drastic performance improvements in `Cache Hit` scenarios.

### 🏛️ Architecture and Refactorings

* **Lazy Loading Architecture:**
  * The `ResolvedConfig` facade (`Facade Pattern`) has been introduced to postpone the loading and parsing of `axes.toml` files until they are strictly necessary.
  * `axes` initialization no longer performs disk I/O for configuration, reducing startup latency for commands that do not require configuration (e.g., `axes --version`).

* **Concurrent Layer Loading (`ConfigLoader`):**
  * The `ConfigLoader` now uses a thread pool (`rayon`) to load and compile all configuration layers of a project hierarchy in parallel.
  * `Arc<OnceLock<...>>` (`LayerPromise`) is used as a synchronization mechanism to guarantee that each layer is compiled only once, even under concurrent requests, eliminating race conditions.

* **Single-Layer Caching:**
  * The monolithic cache model (`config.cache.bin`) has been replaced with a granular caching system. Each `axes.toml` in the hierarchy generates its own binary cache file.
  * Cache invalidation is now much more precise: a change in a sub-project's `axes.toml` only invalidates its own cache, not those of its parents or siblings.

* **Unification of the `Task` Model:**
  * The distinction between `CacheableValue::Raw` and `Expanded` has been removed. The cache now exclusively stores the pre-compiled AST representation (`Task`), optimizing `Cache Hit` performance by eliminating all runtime parsing.

### 🐛 Critical Bug Fixes

* **Fixed Serialization Bug in `[options]`:** A fundamental crash preventing the correct serialization/deserialization with `bincode` of `Command` structures within `[options]` (`at_start`, `open_with`, etc.) has been corrected. This was solved by compiling *all* `Command`s into `Task`s before writing them to the cache, ensuring a 100% robust and stable cache.

---

## v0.2.3-beta: Handler Consistency and CLI Robustness

With the core architecture stabilized, this release focused on refactoring all command handlers (`handlers`) to align with the new centralized state model and improve the CLI's robustness and user experience.

### 🏛️ Architecture and Refactorings

* **Centralized State Management:**
  * All handlers (`delete`, `link`, `rename`, `init`, etc.) have been rewritten to operate exclusively on the mutable `&mut GlobalIndex` reference provided by `main`.
  * Redundant index loading and saving logic (`load_global_index`, `save_global_index`) has been removed from within the handlers, eliminating race conditions and unnecessary disk accesses.

* **Argument Grammar Unification:**
  * The way handlers interpret arguments from the universal dispatcher has been standardized. Now, all handlers use `clap` to define and parse their positional arguments and flags, respecting the priority of the `axes <ctx> <action>` grammar versus `axes <action> [args...]`.

### ✨ Enhancements and New Features

* **`dry-run` Functionality:** The `run` and `start` commands now support the `--dry-run` flag, which displays a detailed execution plan of the commands that would run, without performing any action.
* **Command Discoverability:**
  * `axes run` (without arguments) now lists all available scripts in the current context, indicating which ones are inherited.
  * `axes open --list` now displays all actions configured under `[options.open_with]`.
* **`repair` Command:** A new maintenance command `axes repair` has been added. It scans the file system for projects, detects path inconsistencies in the `GlobalIndex`, and allows for safe correction with `--fix`.
* **Improved `tree` Diagnostics:** The `axes tree` command now includes `--check` flags (to verify path existence), `--depth`, and displays the aliases associated with each project.

### 🐛 Bug Fixes

* **Correct Help Output:** A bug where invoking `--help` on any command resulted in an error exit code (1) has been fixed. Informative outputs like `--help` and `--version` now exit with a success code (0).
* **`default` Parameter Logic:** A logical error where the `default_value` of a named parameter (flag) was always applied, even if the flag was not provided by the user, has been corrected. The `default` now only acts if the flag is present.

---

## v0.2.4-Beta: Syntax Ergonomics and Execution Flexibility

This version introduces significant improvements to the ergonomics of the `axes.toml` syntax and the flexibility of the execution engine, responding to advanced use cases.

### ✨ Enhancements and New Features

* **Simplified Token Syntax:**
  * The entire dynamic token syntax from `<axes::token>` has been simplified to `<token>` (e.g., `<path>`, `<vars::db_host>`).
  * An escaping mechanism (`\<token>`) has been added to treat a token as a literal string of text.

* **Argument Delegation to Shell (`$`):**
  * A new command prefix `$` has been introduced. A line marked with `$` (e.g., `$ pytest`) will receive **all arguments** provided by the user on the CLI, allowing the underlying shell to interpret them (`$1`, `$2`, `$@`, etc.).
  * This feature coexists with `axes` tokens, allowing for hybrid workflows and maximum compatibility with external tools.

* **`literal` Parameter Modifier:**
  * A new modifier `literal` has been added to the parameter syntax (e.g., `<params::path(literal)>`).
  * This modifier forces the resolved parameter value to be wrapped in quotes (`"..."`) before being inserted into the final command, ensuring values with spaces (like file paths) are treated as a single argument.

* **ANSI Color Tokens (`<#color>`):**
  * Support for color tokens (e.g., `<#green>`, `<#red>`, `<#reset>`) has been added, which resolve to their corresponding ANSI escape codes. This allows for custom-formatted and readable output directly from `axes.toml` in print commands (`#`).

### 🏛️ Architecture and Refactorings

* **Cross-Platform Binary Support:** Officially supported and pre-compiled binaries are now offered for **Linux** and **macOS** (x86_64 and Apple Silicon), in addition to Windows.
* **Parameter Parser Robustness:** A subtle bug in the `ArgResolver` that prevented the correct detection of named parameter aliases that included the `-` prefix (e.g., `alias='-ab'`) has been fixed. The system now canonically handles all forms of flags and aliases.

---

## v0.2.5-beta: Engine Hardening and Multi-Platform Compatibility

This release focuses on hardening the `axes` core, eliminating silent failures, and ensuring predictable, consistent behavior across all supported operating systems (Windows, macOS, Linux).

### 🐛 Critical Bug Fixes

* **Fixed Silent Broken Reference Failures:**
  * **Problem:** References to non-existent scripts or variables (e.g., `<scripts::non_existent_script>`) were silently ignored during execution, resulting in incomplete or incorrect commands without generating any error.
  * **Root Cause:** The task flattening logic (`ResolvedConfig::flatten_task`) resolved references, but if `get_script()` or `get_var()` returned `None`, it would simply omit the component in the final AST.
  * **Solution:** Strict validation has been implemented at the point of flattening. Now, if a reference cannot be resolved in the project hierarchy, execution stops immediately with an explicit and contextual error (`Broken Reference: The script '<scripts::...>' was not found...`), drastically improving the reliability and debugging of `axes.toml` files.

### 🏛️ Architecture and Refactorings

* **Windows Compatibility Hardening:**
  * **Problem:** Session initialization scripts (`at_start`) could fail on Windows if environment variables contained special `cmd.exe` metacharacters (like `&`, `|`, `<`, `>`).
  * **Solution:** A robust escaping function (`escape_for_cmd_set`) has been implemented in `system/shell.rs` that correctly sanitizes environment variable values before writing them to the temporary `.bat` script. This ensures that interactive sessions (`axes start`) are reliable on Windows, even with complex environment values.
* **Portability Audit:** A thorough review of the codebase has been conducted, confirming the robustness of the file system abstractions (`dunce`, `PathBuf`) and command execution, ensuring the core behavior is identical across platforms.

---

## v0.2.6-beta: Developer Experience (DX) and Parameter Flexibility

This version represents a qualitative leap in Developer Experience (DX), with a complete overhaul of the CLI help interface and a significant increase in the flexibility of the parameter system.

### ✨ Enhancements and New Features

* **Complete CLI Help Overhaul (`--help`):**
  * **Problem:** The output of `axes --help` was generic and uninformative due to `axes`'s universal grammar.
  * **Architectural Solution:** The help system has been completely refactored using the `help_template` attribute in `clap`.
        1. A **semantic template** has been created in `locales/en.toml` that defines the structure and content of the entire help output, using style markers (e.g., `<title>`, `<cmd>`, `<hl>`) instead of color codes.
        2. A "renderer" function (`build_help_string`) has been implemented in `cli/mod.rs`. At runtime, this function detects if the terminal supports colors (using `colored::control`) and dynamically replaces the semantic markers with the appropriate ANSI escape codes or with empty strings.
  * **Impact:** The result is a fully customized, visually appealing, educational help output that gracefully degrades to plain text in unsupported environments (like CI logs), offering a first-class user experience.

### 🏛️ Architecture and Refactorings

* **Literal Behavior for `map` Modifier in Parameters:**
  * **Problem:** The `map` modifier on flag-type parameters (e.g., `<params::name(map='--new')>`) always added a space between the mapped value and the argument value, preventing common formats like `--key=value` or `--keyVALUE`.
  * **Solution:** The logic in `ArgResolver::new` has been refactored. The behavior of `map` is now **literal concatenation**. The `map` value acts as an exact prefix to the argument value. If a space is desired, it must be explicitly included in the `map` string (e.g., `map='--param '`).
  * **Impact:** This refactoring grants the user full and predictable control, drastically increasing flexibility for integration with any underlying command-line tool.
