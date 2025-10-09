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

## Current Status: `v0.2.0-beta` — The Performance Milestone

With version `v0.2.0`, we have achieved a critical goal: **peak performance**.

* **Optimized Core Engine:** The script expansion engine has been rewritten from the ground up. It now uses a highly efficient **direct structural composition** algorithm, making it one of the fastest orchestrators available.
* **Universal Grammar:** The CLI is now more predictable and ergonomic, with a unified grammar that works seamlessly inside and outside of project sessions.
* **Foundation for the Future:** With the core architecture now stable and lightning-fast, we are ready to build the next generation of intelligent features on this solid foundation.

---

## The Road to `v1.0` — From Orchestrator to Intelligent Build System

Our immediate goal is to evolve `axes` beyond simple command execution. We will introduce intelligence, efficiency, and an unparalleled user experience, culminating in a `v1.0` release that sets a new standard.

### **Milestone 1: The "Intelligent Build" Milestone (`v0.3.0`)**

**Goal:** Transform `axes` from a task *runner* into an efficient build system that avoids redundant work. This is our highest priority.

* `[ ]` **Implement Artifact Caching (MVP):**
  * **Description:** Introduce a task caching mechanism based on input file checksums, similar to `Task` or `Turborepo`. A new `[scripts.my_script.cache]` section in `axes.toml` will allow users to declare `sources` (input files/globs). `axes` will calculate a state checksum and skip the execution if no sources have changed since the last successful run.
  * **Value:** This is the single most impactful feature for developer productivity. It will save minutes to hours in daily workflows and CI/CD pipelines by avoiding costly re-compilations, re-testing, and re-packaging. It elevates `axes` into the "intelligent build system" category.

### **Milestone 2: The "Premium DX" Milestone (`v0.4.0`)**

**Goal:** Polish the daily user experience to make `axes` not just powerful, but a joy to use.

* `[ ]` **Implement Shell Autocompletion:** `[contribution welcome]`
  * **Description:** Provide dynamic, context-aware autocompletion for shells (`bash`, `zsh`, `fish`). The completion engine must intelligently suggest project contexts, aliases, available scripts for a given context, and even script parameters.
  * **Value:** The most significant quality-of-life improvement for discoverability and daily usability.
* `[ ]` **Implement the "Orchestrator" TUI:**
  * **Description:** When running `axes` without arguments, launch an interactive Terminal User Interface. This TUI will act as a project dashboard, visualizing the project tree, allowing users to browse available scripts (with descriptions), and trigger them directly.
  * **Value:** Transforms the first impression of `axes` into a premium, guided experience and makes complex monorepos instantly navigable.

### **Milestone 3: The "Power User" Milestone (`v0.5.0`)**

**Goal:** Unlock advanced scripting patterns and give users granular control over their workflows.

* `[ ]` **Implement Private Scopes (`_` prefix):**
  * **Description:** Keys in `[vars]`, `[env]`, and `[scripts]` that start with an underscore (`_`) will not be inherited by child projects.
  * **Value:** Allows true encapsulation. Parent projects can define internal helper scripts and variables without polluting the namespace of their children.
* `[ ]` **Advanced Artifact Caching:**
  * **Description:** Extend the caching system to support `generates` (output files) and dynamic `key`s based on variables. This allows caching to be influenced by environment, platform, or parameters.

        ```toml
        [scripts.build.cache]
        sources = ["src/**/*.js"]
        generates = ["dist/bundle.js"]
        key = "<vars::node_version>-<params::profile>"
        ```

  * **Value:** Enables highly sophisticated and reliable caching strategies for complex build matrices.

### **Milestone 4: The "Ecosystem & Stability" Milestone (`v1.0.0`)**

**Goal:** Prepare `axes` for its official, production-ready launch.

* `[ ]` **Implement `axes validate` / `axes doctor`:**
  * **Description:** A command that performs a health check on the entire `axes` ecosystem. It will find and offer to fix inconsistencies like broken parent links in the index, projects whose paths no longer exist, and corrupted cache files.
  * **Value:** A crucial diagnostic and repair tool that builds long-term user trust and confidence.
* `[ ]` **Native Support for `.env` Files:** `[contribution welcome]`
  * **Description:** Add a key like `[env].load = true` to automatically discover and load variables from a `.env` file into the script execution environment.
  * **Value:** A highly requested feature that aligns with modern development practices and simplifies configuration management.
* `[ ]` **Final API Freeze and Documentation Overhaul:**
  * **Description:** Conduct a final review of the `axes.toml` syntax and CLI command contracts. Declare them stable for `v1.0`. Ensure all documentation is complete, polished, and full of examples.
  * **Value:** The promise of stability that is essential for adoption in production environments.

---

## **The Future (Post-`v1.0`)**

These are ambitious, game-changing features that we will explore once the `v1.0` foundation is solid.

* `[ ]` **The `axes` Daemon:** A long-running background process that can watch the filesystem and provide near-instantaneous build caching and reactivity, similar to modern frontend tools.
* `[ ]` **Remote Caching:** The ability to share the artifact cache across a team or CI farm, enabling massive speedups in collaborative environments.
* `[ ]` **Templating Engine (`init 2.0`):** Transform `axes init` into a powerful scaffolding engine (`cookiecutter`-style) that can generate new projects from predefined, version-controlled templates.
* `[ ]` **Dynamic Tokens:** Integrate with tools like Git to provide dynamic, real-time tokens like `<git::branch>` or `<git::is_dirty>`.

---

## **Call for Testers (Beta Phase)**

The most valuable contribution right now is to **use `axes` and give us feedback**. We are particularly interested in:

1. **Performance:** How does `axes v0.2.0` feel in your daily workflow? Are there any commands that still feel slow?
2. **The Parameter System:** Push `<params::...>` to its limits. Create complex, parameterized scripts. Is the behavior always intuitive?
3. **Real-World Monorepos:** Try integrating `axes` into one of your existing monorepo projects. What challenges did you face? What features were missing?

**How to report feedback?**
Please open an Issue in our [GitHub repository](https://github.com/RetypeOS/axes/issues). Every piece of feedback is a step toward building a better tool.
