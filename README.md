<p align="center">
  <img src="https://raw.githubusercontent.com/retypeos/axes/main/logo.png" alt="axes Logo" width="200" style="border-radius: 50%;">
</p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/badge/build-passing-brightgreen" alt="CI/CD Status"></a>
  <a href="https://github.com/retypeos/axes/releases"><img src="https://img.shields.io/badge/version-v0.2.4--beta-blue" alt="Version"></a>
  <a href="https://github.com/retypeos/axes/blob/main/LICENSE"><img src="https://img.shields.io/github/license/retypeos/axes?color=lightgrey" alt="License"></a>
  <a href="https://deepwiki.com/RetypeOS/axes"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
</p>

<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./README.md">English</a> •
  <a href="./docs/es/README.md">Español</a>
</p>

<h1 align="center">axes: The Conductor for Your Development Chaos</h1>

<p align="center">
  <strong>The power of a complex orchestrator. The speed of a simple executor.</strong>
</p>

---

> `axes` is a fast, language-agnostic workflow orchestrator that unifies build, test, and deployment scripts across projects of any size — from single repos to complex monorepos.

## Your Workflow Is a Mess. We Fixed It

- **Terminal 1:** `cd frontend && npm run dev`
- **Terminal 2:** `cd backend && source .venv/bin/activate && uvicorn app:main --reload`
- **You, 3 weeks later:** *“Wait... was the command for tests `npm test`, `pytest`, `cargo test`, or `go test ./...`?”*

That doubt, that cognitive load, is friction. It kills your flow. Simple task runners give you shortcuts. **`axes` gives you a universal language.**

`axes` is a high-performance workflow orchestrator written in Rust. It's not just another task runner; it is a **command language** that standardizes how you build, test, and run any project, from a simple script to a complex polyglot monorepo. It replaces scattered `Makefile`s, `package.json` scripts, and fragile shell scripts with a single, consistent, and blazing-fast interface.

`axes` is the conductor who knows every instrument in your orchestra, turning your chaotic collection of tools into a symphony.

### The End of the Speed vs. Power Compromise

For years, the choice has been a false dichotomy:

- **Simple Runners (`just`, `make`):** Fast to start, but limited. They are glorified alias managers, lacking hierarchy, parameterization, and true orchestration capabilities.
- **Complex Orchestrators (`Bazel`, `Gradle`):** Incredibly powerful, but notoriously slow, complex to configure, and rigid. The startup tax is a constant drag on productivity.

**`axes` shatters this compromise.** We deliver the advanced orchestration features of complex systems at a speed that rivals—and often exceeds—the simplest runners.

| Command | Mean [ms] (± σ) | Min … Max [ms] | Relative Speed |
|:---|:---:|:---:|:---:|
| **`axes --version`** | **19.6 ± 1.8** | 16.6 … 25.3 | **1.00** |
| `just --version` | 24.4 ± 3.5 | 18.7 … 35.1 | 1.25x slower |
| `task --version` | 69.0 ± 9.0 | 54.9 … 90.8 | 3.52x slower |
| | | | |
| **`axes <script>`** | **41.8 ± 1.9** | 38.1 … 45.9 | **1.00** |
| `just <script>` | 44.7 ± 4.0 | 38.0 … 57.9 | 1.07x slower |
| `task <script>` | 79.9 ± 9.3 | 60.9 … 99.2 | 1.91x slower |

*Benchmarks executed on a standard development machine (Windows 11, Intel Core i7, 16GB RAM, SSD NVMe) using `hyperfine`. Each command was run 50 times after a 5-run warmup.*

This isn't magic; it's obsessive engineering.

- **Lazy, Parallel Config Loading:** `axes` intelligently loads only the configuration it needs, in parallel, minimizing startup I/O.
- **Pre-compiled AST Cache:** On first run, your `axes.toml` files are compiled into a highly optimized **Abstract Syntax Tree (AST)**. This AST is then saved to a compact binary cache.
- **Instant Hot Executions:** Every subsequent run skips text parsing entirely. It deserializes the pre-compiled AST from the binary cache—an operation orders of magnitude faster—and executes it instantly.

**The result: you pay the orchestration cost only once. You get the speed of a simple runner every time after.**

- ⚙️ **[Deep Dive into the `axes` Architecture (`TECHNICAL.md`)](./TECNICAL.md):** For those interested in the engineering behind our performance, this is the place to start.

---

### The `axes` Philosophy: More Than Just a Task Runner

`axes` is built on a foundation of principles that simple tools ignore.

#### 1. Orchestration, Not Just Execution

Projects have relationships. `axes` lets you organize them into a logical tree (`app/api`, `app/web`). Children automatically inherit scripts, variables, and environment settings from their parents, which they can override as needed. **Define once, reuse everywhere.**

```sh
# A script defined in the 'global' config is available everywhere.
$ axes my-app/api/db migrate

# The 'build' script in 'my-app/api' can call the 'build' script of its parent.
$ axes my-app/api build
```

#### 2. Ergonomics, Not Just Shortcuts

Your scripts become first-class, self-documenting command-line applications with typed parameters, default values, and validation—all without writing a single line of boilerplate parsing code.

```toml
# in .axes/axes.toml
[scripts]
deploy = "terraform apply -var 'env=<axes::params::0(default='staging', required)>'"
#                                  ^-- A required positional parameter
#                                        with a default value.
```

```sh
axes deploy production  # Runs with env='production'
axes deploy             # Runs with env='staging'
```

#### 3. Robustness by Design

`axes` identifies projects by an immutable **UUID**, not a fragile file path. Rename or move your project directories freely—`axes`'s index is self-healing and will never lose track of your projects. This makes refactoring large monorepos trivial and safe.

---

### Installation (30 Seconds to a Better Workflow)

`axes` is a single, dependency-free binary written in Rust.

1. Go to the [**`axes` Releases page on GitHub**](https://github.com/retypeos/axes/releases).
2. Download the archive for your operating system (`windows-x86_64`, `linux-x86_64`, `macos-x86_64`).
3. Unzip it and move the `axes` executable to a directory in your system's `PATH` (e.g., `/usr/local/bin`, `C:\Windows\System32`).
4. Open a **new terminal** and verify the installation with `axes --version`.

---

### `axes` in Action: A Glimpse of the Power

#### 1. Universal and Context-Aware Commands

The grammar is simple and predictable.

```sh
# Run the 'build' script in the current project context.
$ axes build --release

# Run the 'test' script in a specific sub-project.
$ axes my-app/api test
```

#### 2. Effortless Cross-Platform Workflows

Define a command once. It works for your entire team, on any OS.

```toml
[scripts.browse]
desc    = "Opens the local documentation in the browser."
windows = "start http://localhost:8080"
macos   = "open http://localhost:8080"
linux   = "xdg-open http://localhost:8080"
```

#### 3. Real-Time Dynamic Values

Run commands and use their output as variables in other commands, instantly.

```toml
[scripts]
# Tags a Docker image with the current short git hash
tag_release = "docker tag my-app:latest my-app:<axes::run('git rev-parse --short HEAD')>"
```

#### 4. Immersive Focus Sessions

Dive into a sub-project. `axes` sets up and tears down your environment for you, automatically.

```toml
# in my-app/api/.axes/axes.toml
[options]
at_start = "source .venv/bin/activate" # Executes upon entering the session
at_exit  = "docker-compose down"       # Executes upon exiting the session
```

```sh
$ axes my-app/api start  # Starts a session. `at_start` runs automatically.

(axes: my-app/api) $ axes test  # You are now "inside" my-app/api.
(axes: my-app/api) $ exit       # `at_exit` runs automatically.
```

**Your development environment, on demand.**

---

### Ready to Conduct Your Own Orchestra?

The friction you feel every day is not a requirement. It is a problem with a solution. `axes` is that solution.

- ➡️ **[Quick Start Guide (`GETTING_STARTED.md`)](./GETTING_STARTED.md):** Build your first orchestrated monorepo in 15 minutes.
- 📖 **[Mastering `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** The definitive reference for every feature and syntax.
- ⌨️ **[Command Reference (`COMMANDS.md`)](./COMMANDS.md):** A complete guide to every built-in CLI command (`init`, `register`, `tree`, etc.).

### Join the Workflow Revolution

`axes` is more than a tool; it's a movement to restore control, consistency, and joy to development. Your voice is crucial.

- **Found a Bug or Have a Great Idea:** [**Open an Issue**](https://github.com/retypeos/axes/issues)
- **Want to Contribute Code:** Pull Requests are always welcome! Check our [Contribution Guidelines](./CONTRIBUTING.md).

**Install `axes` today. Stop remembering commands. Start building.**
