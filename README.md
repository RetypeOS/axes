<p align="center">
  <img src="logo.png" alt="axes Logo" width="200">
</p>

<h1 align="center">axes: The Conductor for Your Development Chaos</h1>

<p align="center">
  <strong>The power of a complex orchestrator. The speed of a simple executor.</strong>
</p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/badge/build-passing-brightgreen" alt="CI/CD Status"></a>
  <a href="https://github.com/retypeos/axes/releases"><img src="https://img.shields.io/badge/version-v0.2.1--beta-blue" alt="Version"></a>
  <a href="https://deepwiki.com/RetypeOS/axes"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License"></a>
</p>

<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./README.md">English</a> •
  <a href="./docs/es/README.md">Español</a>
</p>

---

## Your Workflow Is a Mess. We Fixed It

- **Terminal 1:** `cd frontend && npm run dev`
- **Terminal 2:** `cd backend && source .venv/bin/activate && uvicorn app:main --reload`
- **You, 3 weeks later:** *“Wait... was the command for tests `npm test`, `pytest`, `cargo test`, or `go test ./...`?”*

That doubt, that cognitive load, is friction. It kills your flow. Simple task runners like `make` or `just` give you shortcuts. **`axes` gives you a universal language.**

`axes` is not just another task runner. It is the **command language** that ties your entire stack together. It allows you to compose, parameterize, and standardize workflows for ANY tool, in ANY language. Your `package.json` knows `npm`, your `Makefile` knows `make`. **`axes` is the conductor who knows them all**, turning your chaotic collection of tools into a symphony.

### Who Said You Had to Choose Between Power and Speed?

For years, the choice has been between:

- **Simple Runners (`just`, `make`):** Very fast, but limited. They are glorified alias managers.
- **Complex Orchestrators (`Bazel`, `Gradle`):** Incredibly powerful, but notoriously slow, complex, and rigid.

**`axes` breaks this compromise.** We offer the advanced orchestration capabilities of complex systems at a speed that rivals (and often exceeds) the simplest runners.

| Tool | Hot Script Execution | Orchestration Features |
| :---------  | :-----------------------------: | :-----------------------------: |
| `just`      | **~38 ms**                      |            Basic                |
| `task`      | **~40 ms**                      |          **Advanced**           |
| **`axes`**  | **~35 ms**                      |          **Advanced**           |

*Benchmarks executed on a standard development machine running a simple "hello world" script, observing only the startup, resolution, execution, and shutdown time, obtaining the minimum average time from sets of 200 executions.*

We achieve this through an architecture obsessed with performance.

- **JIT Compilation to AST:** The first time you run a script, `axes` acts as a Just-in-Time compiler. It parses your `axes.toml`, resolves all inheritance and composition, and compiles it into a highly optimized **Abstract Syntax Tree (AST)**.
- **Persistent Binary Cache:** This AST is saved to a binary cache (`.axes/config.cache.bin`).
- **Instant Hot Executions:** Every subsequent run completely skips the costly work. `axes` deserializes the pre-compiled AST from the binary cache—an operation orders of magnitude faster than text parsing—and executes it.

**The result: you pay the orchestration cost only once. You get the speed of a simple runner every time after.**

- ⚙️ **[Complete Architecture Reference (`TECNICAL.md`)](./TECNICAL.md):** If you are interested in delving deeper into the `axes` architecture, the best place is by viewing the code, but this is the second-best place.

---

### The `axes` Philosophy: More Than a Task Runner

`axes` is built on a foundation that simple tools ignore.

- **Orchestration, Not Just Execution:** `axes` understands that projects have relationships. Organize them into trees (`app/api`, `app/web`). Children inherit and override configurations. Define once, use everywhere.
- **Ergonomics, Not Just Shortcuts:** Your scripts become first-class command-line applications.

    ```toml
    # Scripts as Functions: Parameterize, validate, and set default values.
    [scripts]
    deploy = "terraform apply -var 'env=<axes::params::0(default='staging')>'"
    ```

    No more fragile `bash` scripts for parsing arguments.
- **Robustness by Design:** `axes` identifies projects by an immutable `UUID`, not a fragile file path. Rename or move your directories freely—`axes` will never lose track of your projects.

---

### Installation (30 Seconds to a Better Workflow)

`axes` is a single dependency-free binary.

1. Go to the [**`axes` Releases page on GitHub**](https://github.com/RetypeOS/axes/releases).
2. Download the file for your operating system.
3. Unzip it and move the `axes` executable to a directory in your `PATH`.
4. Open a **new terminal** and verify with `axes --version`.

---

### `axes` in Action: A Glimpse of Power

#### 1. Universal and Context-Aware Commands

Run a script in the current directory. The syntax is simple and predictable.

```sh
# Executes the 'build' script defined in the nearest axes.toml
axes build --release
```

#### 2. Effortless Cross-Platform Workflows

Define a command once. It works for your entire team, on any OS.

```toml
[scripts.browse]
desc = "Opens the local documentation in the browser."
windows = "start http://localhost:8080"
macos   = "open http://localhost:8080"
linux   = "xdg-open http://localhost:8080"
```

#### 3. Real-Time Dynamic Composition

Run commands and use their output instantly.

```toml
[scripts]
# Tags a Docker image with the current short git hash
tag_release = "docker tag my-app:latest my-app:<axes::run('git rev-parse --short HEAD')>"
```

#### 4. Immersive Focus Sessions

Dive into a sub-project. `axes` sets up and dismantles your environment for you.

```toml
# in my-app/api/.axes/axes.toml
[options]
at_start = "source .venv/bin/activate" # Executes upon entry
at_exit  = "docker-compose down"       # Executes upon exit
```

```sh
$ axes my-app/api # Starts a session. `at_start` executes automatically.

(axes: my-app/api) $ axes test  # You don't need to repeat the context.
(axes: my-app/api) $ exit       # `at_exit` executes automatically.
```

**Your development environment, on demand.**

---

### Ready to Conduct Your Own Orchestra?

The friction you feel every day is not a requirement. It is a problem with a solution. `axes` is that solution.

- ➡️ **[Quick Start Guide (`GETTING_STARTED.md`)](./GETTING_STARTED.md):** Build your first orchestrated monorepo in 15 minutes.
- 📖 **[Mastering `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** The definitive reference for every feature.
- ⌨️ **[Command Reference (`COMMAND.md`)](./COMMAND.md):** A complete guide to every CLI command.

### Join the Workflow Revolution

`axes` is more than a tool; it is a movement to restore control and consistency to development. Your voice is crucial.

- **Found a Bug or Have a Great Idea:** [**Open an Issue**](https://github.com/RetypeOS/axes/issues)
- **Want to Contribute Code:** Pull Requests are always welcome!

**Install `axes` today. Stop searching for commands. Focus on what truly matters: **bringing your software to life**, and let `axes` worry about the how.**
