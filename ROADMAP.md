<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./ROADMAP.md">English</a> •
  <a href="./docs/es/ROADMAP.md">Español</a>
</p>

# Project `axes` Roadmap

Welcome to the `axes` roadmap! This document outlines our ambitious vision for transforming `axes` from a world-class orchestrator into the definitive, intelligent build system for modern development. It serves as a guide for our core mission and a call to action for community contributors.

## How to Contribute

Your expertise is invaluable. If you see a feature that excites you, especially those marked with `[contribution welcome]`, the ideal process is:

1. Check for existing Issues or Pull Requests to avoid duplicate work.
2. Open a new Issue to discuss your implementation strategy. This allows us to align on the architecture and assign the task.
3. Let's build the future of `axes` together!

## Current Status: `v0.3.0-beta` — The "Juggernaut" Architecture Milestone

With version `v0.3.0`, we have successfully re-engineered the core of `axes`. This "Juggernaut" release establishes a new foundation of performance, robustness, and cross-platform consistency.

* **Universal AST & Portable Cache:** The compilation engine has been rewritten. `axes` now generates a **platform-agnostic Abstract Syntax Tree (AST)**, meaning binary cache files are **100% portable** between Windows, macOS, and Linux. This is a game-changing feature for multi-platform teams.
* **Just-In-Time (JIT) Optimization:** A final, in-memory "specialization" step was introduced before execution. This gives us the flexibility of a universal cache with the raw, uncompromising speed of a platform-specific runner. Benchmarks confirm `axes` is now significantly faster than its predecessors and competitors in realistic, high-load scenarios.
* **Enhanced Syntax & Robustness:** The `axes.toml` syntax for scripts and variables is now more powerful, ergonomic, and strictly validated to prevent user errors.
* **Ephemeral Execution (`_`):** It is now possible to run scripts in unregistered projects, a powerful feature for CI/CD and temporary workflows.

---

## The Road to `v1.0` — From Orchestrator to Intelligent Build System

Our path to `v1.0` is focused on building upon the new "Juggernaut" architecture. We will stabilize, enhance the user experience, and then deliver the cornerstone feature of an intelligent build system: artifact caching.

### **Milestone 1: The "Polishing & Stability" Milestone (`v0.4.0`)**

**Goal:** Solidify the `v0.3.0` architecture, refine the user experience, and ensure rock-solid stability. This is our immediate priority.

* `[ ]` **Final Architectural Review & Minor Fixes:**
  * **Description:** Conduct a comprehensive review of all core modules and handlers, applying final optimizations, improving documentation, and fixing any minor bugs discovered after the v0.3.0 refactor.
  * **Value:** Ensures the new foundation is flawless before building major new features upon it.
* `[ ]` **Implement Shell Autocompletion:** `[contribution welcome]`
  * **Description:** Provide dynamic, context-aware autocompletion for `bash`, `zsh`, and `fish`. The engine must intelligently suggest project contexts, aliases, and available scripts for a given context.
  * **Value:** The single most impactful quality-of-life improvement for discoverability and daily usability.
* `[ ]` **Implement the "Orchestrator" TUI (MVP):**
  * **Description:** When running `axes` without arguments, launch a basic, interactive Terminal User Interface. This TUI will visualize the project tree and allow users to browse and select available scripts for the current context.
  * **Value:** Transforms the first impression of `axes` into a premium, guided experience and makes complex monorepos instantly navigable.

### **Milestone 2: The "Intelligent Build" Milestone (`v0.5.0`)**

**Goal:** Transform `axes` from a task *runner* into an efficient build system that avoids redundant work.

* `[ ]` **Implement Artifact Caching (MVP):**
  * **Description:** Introduce a task caching mechanism based on input file checksums. A new `[scripts.my_script.cache]` section in `axes.toml` will allow users to declare `sources` (input files/globs). `axes` will calculate a state checksum and skip script execution if no sources have changed since the last successful run.
  * **Value:** This is the cornerstone feature for developer productivity, saving immense time in daily workflows and CI/CD pipelines by avoiding costly re-compilations, re-testing, and re-packaging.

### **Milestone 3: The "Ecosystem & Stability" Milestone (`v1.0.0`)**

**Goal:** Prepare `axes` for its official, production-ready launch with features that build long-term trust and simplify integration.

* `[ ]` **Implement `axes doctor` command:**
  * **Description:** A comprehensive health check command that finds and offers to fix inconsistencies like broken parent links in the index, projects whose paths no longer exist, and orphaned cache files. This evolves the existing `repair` command.
  * **Value:** A crucial diagnostic and repair tool that builds user confidence.
* `[ ]` **Native Support for `.env` Files:** `[contribution welcome]`
  * **Description:** Add a key like `[env].load = ".env"` to automatically discover and load variables from a specified `.env` file into the script execution environment.
  * **Value:** A highly requested feature that aligns with modern development practices.
* `[ ]` **Final API Freeze and Documentation Overhaul:**
  * **Description:** Conduct a final review of the `axes.toml` syntax and CLI command contracts. Declare them stable for `v1.0`. Ensure all documentation is complete, polished, and full of real-world examples.
  * **Value:** The promise of stability essential for adoption in production environments.

---

## **The Future (Post-`v1.0`)**

These are ambitious, game-changing features that we will explore once the `v1.0` foundation is solid.

* `[ ]` **The `axes` Daemon:** A long-running background process for near-instantaneous build caching and reactivity.
* `[ ]` **Remote Caching:** Sharing the artifact cache across a team or CI farm.
* `[ ]` **Templating Engine (`init 2.0`):** A powerful scaffolding engine to generate new projects from templates.
* `[ ]` **Advanced Scripting Features:** Private scopes (`_` prefix for non-inheritable items), advanced caching keys, and dynamic tokens (`<git::branch>`).

---

## **Call for Testers (Beta Phase)**

The most valuable contribution right now is to **use `axes v0.3.0` and give us feedback**. We are particularly interested in:

1. **Cross-Platform Cache:** If you work in a multi-OS team, try committing your `.axes-cache` directory. Does it work seamlessly for your colleagues?
2. **The `_` Context:** Push the ephemeral execution mode to its limits in your CI pipelines or local tests.
3. **Real-World Monorepos:** Integrate `axes` into one of your existing complex projects. What challenges did you face? What features were missing?

**How to report feedback?**
Please open an Issue in our [GitHub repository](https://github.com/retypeos/axes/issues). Every piece of feedback is a step toward building a better tool.
