<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./ROADMAP.md">English</a> •
  <a href="./docs/es/ROADMAP.md">Español</a>
</p>

# Project `axes` Roadmap

Welcome to the `axes` roadmap! This document outlines the short-term and long-term vision for the project. It serves as a guide for core developers and a starting point for community members who wish to contribute.

## How to Contribute

Your help is welcome! If you see a task that interests you, especially those marked with `[contribution welcome]`, the ideal process is:

1. Ensure there is no open Pull Request for that task.
2. Open an Issue on GitHub to discuss your approach so we can assign it to you. This prevents duplicate work.
3. Start working on your Pull Request!

## Current Status: `v0.2.0-beta`

`axes` is in its first **Beta** phase. This means:

* **Stable Core:** The main architecture (dispatcher, handlers, interpolator, cache system) is robust and well-tested.
* **`axes.toml` API Defined:** The `axes.toml` syntax, including `[scripts]`, inheritance, and the `<axes::params::...>` system, is feature-complete for v1.0.
* **Ready for Testing:** The tool is ready to be used in real projects. Bugs are expected, and user feedback is crucial during this phase.

---

## Immediate Roadmap (The Path to v1.0)

These are the milestones that will lead us to a stable and polished 1.0 release.

### Milestone 1: The "Premium" User Experience (v0.3.0)

**Goal:** Make daily interaction with `axes` as smooth, fast, and intuitive as possible.

* `[ ]` **Implement Shell Autocompletion:** `[contribution welcome]`
  * **Description:** Integrate `clap_complete` to generate autocompletion scripts for `bash`, `zsh`, `fish`, etc. It must dynamically autocomplete project contexts, actions, and script names.
  * **Value:** The most significant quality-of-life improvement for daily usability.
* `[ ]` **Implement the Welcome TUI:**
  * **Description:** When running `axes` without arguments, launch a read-only TUI (Terminal User Interface) that displays the project tree and allows exploring available scripts.
  * **Value:** Transforms the first impression and greatly facilitates "discoverability" in complex ecosystems.
* `[ ]` **Standardize and Beautify Output:** `[contribution welcome]`
  * **Description:** Create a `ui/printer` module and use a crate like `cli-table` to standardize the output of `info`, `alias list`, etc., into well-formatted tables.
  * **Value:** Provides a cohesive and professional visual identity to the tool.

### Milestone 2: Inheritance Control and Advanced Scripting Logic (v0.4.0)

**Goal:** Give users granular control over inherited configuration and unlock advanced scripting patterns.

* `[ ]` **Implement Private/Public Inheritance with `_`:**
  * **Description:** Modify the `config_resolver` so that keys in `[vars]`, `[env]`, and `[scripts]` that start with an underscore (`_`) are not inherited by child projects.
  * **Value:** Allows encapsulation and the definition of internal "helpers" in a parent project without polluting the children's namespace.
* `[ ]` **Implement Multi-platform Commands in Sequences:**
  * **Description:** Extend the `axes.toml` parser so that within a sequence in `[scripts]`, an individual step can be defined as a multi-platform table.

        ```toml
        # Syntax to support
        deploy = [
            "<axes::scripts::build>",
            { windows = "win-deploy.ps1", linux = "./deploy.sh" },
            "echo 'Deployed!'"
        ]
        ```

  * **Value:** Unlocks the ability to create complex workflows that are, step-by-step, completely cross-platform.

### Milestone 3: Stabilization and Ecosystem (v1.0.0)

**Goal:** Prepare `axes` for its official launch, focusing on robustness and facilitating adoption.

* `[ ]` **Implement `axes validate`:**
  * **Description:** A command that scans the entire `index.bin` for inconsistencies (paths that no longer exist, broken parent links) and offers reports or interactive repairs.
  * **Value:** A crucial diagnostic tool for long-term user confidence.
* `[ ]` **Native Support for `.env` Files:** `[contribution welcome]`
  * **Description:** Add an `[env].load = ".env"` key to `axes.toml` that automatically loads variables from a `.env` file into the execution environment of scripts.
  * **Value:** A highly requested integration that greatly simplifies secret and local configuration management.
* `[ ]` **API Freeze and Final Documentation:**
  * **Description:** Perform a final review of all APIs (CLI and `axes.toml`) and declare them stable for v1.0. Complete and polish all documentation.
  * **Value:** The guarantee of stability that users need to adopt `axes` in production.

---

## Long-Term Ideas (Post-v1.0 / The Future)

These are more ambitious features that will be considered once the core system is stable.

* `[ ]` **Templating Engine (`init` 2.0):** Transform `init` into a complete scaffolding engine that uses templates from `~/.config/axes/templates/`.
* `[ ]` **Session Switching (`axes switch <context>`):** The ability to switch from one project session to another without needing to `exit` and re-enter.
* `[ ]` **Cache Centralization:** Move all cache files (`.axes/*.bin`) to a centralized directory (`~/.config/axes/cache/`) to keep project directories clean.
* `[ ]` **Git Integration:** Add dynamic tokens like `<axes::git::branch>` or `<axes::git::commit_hash>`.

---

## Help Us Test! (Testing Requests for the Beta)

The best way to contribute right now is by testing `axes` in your workflows! We are especially interested in feedback on the following areas:

1. **The Parameter System:** Try creating complex scripts using `<axes::params::...>` in all its variants. Is it intuitive? Do you find any edge cases that don't work?
2. **Script Composition:** Create scripts that call each other (`<axes::scripts::...>`) and use parallel execution (`>`). Try to break cycle detection.
3. **Refactoring Operations:** Use `link`, `rename`, `unregister`, and `delete` (with their flags) on a test monorepo. Is the behavior always as expected? Are the messages clear?
4. **Cancellation with `Ctrl+C`:** Launch a long-running script (e.g., `[scripts] wait = "sleep 30"`) and try to cancel it. Does the tool respond as you expect?

**How to report feedback?**
Please open an Issue in our [GitHub repository](https://github.com/RetypeOS/axes/issues). Any feedback is incredibly valuable!
