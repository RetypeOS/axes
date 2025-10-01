<p align="center">
  <img src="logo.png" alt="axes Logo" width="200">
</p>

<h1 align="center">axes: The Conductor for Your Development Chaos</h1>

<p align="center">
  <strong>Any Project. Any Language. One Command Language.</strong>
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

## Your Workflow is Broken

- **Terminal 1:** `cd frontend && npm run dev`
- **Terminal 2:** `cd backend && source .venv/bin/activate && uvicorn app:main --reload`
- **You, 3 weeks later:** *“Wait... was the test command `npm test`, `pytest`, `cargo test`, or `go test ./...`?”*

That micro-pause, that cognitive load, is friction. It kills your flow. Tools like `make` or `just` give you shortcuts. **`axes` gives you a universal language.**

`axes` is not just another task runner. It's the **command language** that unites your entire stack. It allows you to compose, parameterize, and standardize workflows for ANY tool, in ANY language, across ANY project structure. Your `package.json` knows `npm`, your `Makefile` knows `make`. **`axes` knows them all.** It's the conductor that turns your chaotic collection of tools into a symphony. `axes` It is the conductor who tells them what to do, using simple, consistent, and powerful commands that **YOU** define, and which travel with your repository, allowing new users to onboard in an absolutely simple and standard way.

### Why `axes`? Because Speed is Not Enough

Simple task runners are fast. But modern development is not just about running one command quickly. It's about managing complexity across dozens of them.

Imagine a monorepo:

**THE CHAOS (BEFORE `axes`):**

```sh
# To spin up everything...
(terminal 1) $ cd frontend && npm run dev
(terminal 2) $ cd backend && source .venv/bin/activate && flask run
(terminal 3) $ cd docs && hugo server
```

**THE ORCHESTRA (WITH `axes`):**

```toml
# in ./.axes/axes.toml
[scripts]
# The '>' indicates parallel execution.
dev = [
    "> axes frontend dev", # Calls the `dev` script of the `frontend` project
    "> axes backend dev",  # Calls the `dev` script of the `backend` project
    "> axes docs dev"      # And the `dev` script of the `docs` project
]
```

From now on, anyone on your team, on any machine, runs the entire environment with **one universal command**:

```sh
axes dev
```

You've just converted tribal knowledge into versioned infrastructure. Onboarding a new developer went from hours to seconds.

---

### The `axes` Philosophy: More Than a Task Runner

`axes` is built on a foundation that simple task runners ignore.

- **Orchestration, not just Execution:** `axes` understands that projects have relationships. Organize them in trees (`app/api`, `app/web`). Children inherit and override variables and scripts. Define once, use everywhere. This is DRY on a whole new level.
- **Ergonomics, not just Shortcuts:** Your scripts become first-class command-line applications.

    ```toml
    # Scripts as Functions: Parameterize, validate, and set defaults.
    [scripts]
    deploy = "terraform apply -var 'env=<axes::params::0(default='staging')>'"
    ```

    No more fragile `bash` scripts to parse arguments.
- **Performance, without Compromise:** Written in Rust, `axes` is architected for speed. A JIT-style caching engine compiles your workflows into a binary format. The first run pays the price of orchestration; **every subsequent run is nearly instantaneous.** You get the power of a complex system at the speed of a simple one.

---

### Installation (30 Seconds to a Better Workflow)

`axes` is a single, dependency-free binary.

1. Go to the [**`axes` Releases page on GitHub**](https://github.com/RetypeOS/axes/releases).
2. Download the archive for your operating system.
3. Unzip it and move the `axes` executable to a directory in your system's `PATH`.
4. Open a **new terminal** and verify with `axes --version`.

---

### `axes` in Action: A Glimpse of the Power

While you're searching an old `README`, others are already orchestrating.

#### 1. Universal, Context-Aware Commands

Run a script in the current directory. The syntax is simple and predictable.

```sh
# Runs the 'build' script defined in the nearest axes.toml
axes build --release
```

#### 2. Effortless Cross-Platform Workflows

Define a command once. It works for your entire team, on any OS.

```toml
[scripts.browse]
desc = "Opens local documentation in the browser."
windows = "start http://localhost:8080"
macos   = "open http://localhost:8080"
linux   = "xdg-open http://localhost:8080"
```

#### 3. Dynamic, Real-time Composition

Execute commands and use their output on the fly.

```toml
[scripts]
# Tag a Docker image with the current short git hash
tag_release = "docker tag my-app:latest my-app:<axes::run('git rev-parse --short HEAD')>"
```

#### 4. Immersive Focus Sessions

Dive into a sub-project. `axes` sets up and tears down your environment for you.

```toml
# in my-app/api/.axes/axes.toml
[options]
at_start = "source .venv/bin/activate" # Executes upon entry
at_exit  = "docker-compose down"       # Executes upon exit
```

```sh
$ axes my-app/api # Starts a session. `at_start` runs automatically.

(axes: my-app/api) $ axes test  # No need to repeat the context.
(axes: my-app/api) $ exit       # `at_exit` runs automatically.
```

**Your development environment, on demand.**

---

### Ready to Conduct Your Own Orchestra?

The friction you feel every day is not a requirement. It's a problem with a solution. `axes` is that solution.

- ➡️ **[Quick Start Guide (`GETTING_STARTED.md`)](./GETTING_STARTED.md):** Build your first orchestrated monorepo in 15 minutes.
- 📖 **[Mastering `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** The definitive reference for every feature.
- ⌨️ **[Command Reference (`COMMAND.md`)](./COMMAND.md):** A complete guide to every CLI command.

### Join the Workflow Revolution

`axes` is more than a tool; it's a movement to restore control and consistency to development. Your voice is crucial.

- **Found a Bug or Have a Great Idea:** [**Open an Issue**](https://github.com/RetypeOS/axes/issues)
- **Want to Contribute Code:** Pull Requests are always welcome!

**Install `axes` today. Stop searching for commands. Forget the small problems. Focus on what truly matters: **bringing your software to life**, and let `axes` worry about the how.**
