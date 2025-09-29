<p align="center">
  <img src="logo.png" alt="axes Logo" width="200">
</p>

<h1 align="center">axes: The Conductor for Your Development Chaos</h1>

<p align="center">
  <strong>Any Project. Any Language. One Command Language.</strong>
</p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/badge/build-passing-brightgreen" alt="CI/CD Status"></a>
  <a href="#"><img src="https://img.shields.io/badge/version-v0.2.0--beta-blue" alt="Version"></a>
  <a href="https://deepwiki.com/RetypeOS/axes"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License"></a>
</p>

<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./README.md">English</a> •
  <a href="./docs/es/README.md">Español</a>
</p>

---

## Does Your Workflow Look Like This?

- **Terminal 1:** `cd frontend && npm run dev`
- **Terminal 2:** `cd backend && source .venv/bin/activate && uvicorn app:main --reload`
- **You, 3 weeks later:** *“Wait... was the command for tests `npm test`, `pytest`, `cargo test`, or `go test ./...`?”*

That micro-pause, that cognitive load when switching projects, is friction that accumulates. It steals your `flow`. It steals your productivity. **Other tools give you shortcuts. `axes` gives you a language.**

`axes` is not another package manager, nor an alternative to `Docker` or `make`. It is the **command language** that unites them all. `axes` allows you to compose, parameterize, and standardize workflows involving ANY tool in your technology stack. Your `package.json` knows how to run `npm`, your `Makefile` knows how to run `make`, and your `docker-compose.yml` knows how to run `Docker`. But who knows how to run them **all together**? **`axes` is that missing intelligence.** It is the conductor who tells them what to do, using simple, consistent, and powerful commands that **YOU** define, and which travel with your repository, allowing new users to onboard in an absolutely simple and standard way.

### Why `axes`? Why Now?

The development world is a chaos of tools. Each project has its own command dialect. `axes` introduces an `esperanto` for your terminal.

Imagine a `monorepo` with a frontend, a backend, and a documentation service:

**THE CHAOS (BEFORE):**

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
    "> axes frontend dev", # Calls the `dev` script of the `frontend` child
    "> axes backend dev",  # Calls the `dev` script of the `backend` child
    "> axes docs dev"      # Calls the `dev` script of the `docs` child
]
```

From now on, any team member, on any machine, spins up the entire environment with **a single universal command**:

```sh
axes . dev
```

You have converted tribal knowledge into versioned infrastructure. Onboarding new developers has just gone from hours to seconds.

---

### The `axes` Philosophy

- **Abstraction, not Replacement:** `axes` is not a new package manager. Use the tools you already love.
- **Convention over Configuration (Your Convention):** Define your own standard commands (`dev`, `test`, `lint`, `deploy`) and use them across all your projects, regardless of the underlying technology.
- **Hierarchy and Inheritance (Maximum DRY):** Organize projects into trees (`my-app/api`, `my-app/frontend`). Children inherit and can override variables and scripts from their parents. Define once, use everywhere.
- **OS Agnostic (True Portability):** Define workflows that work seamlessly across Windows, macOS, and Linux. `axes` takes care of executing the correct command for each platform.
- **Infrastructure as Code:** Your `axes.toml` lives in Git. Your workflows evolve with your code.

---

### Installation (30 Seconds to Get Started)

`axes` is a single dependency-free binary.

1. Go to the [**`axes` Releases page on GitHub**](https://github.com/RetypeOS/axes/releases).
2. Download the file for your operating system.
3. Unzip it and move the `axes` executable to a directory in your `PATH`.
4. Open a **new terminal** and verify with `axes --version`.

---

### `axes` in Action: A Glimpse of Power

Don't fall behind. While you're searching the `README` of an old project, others are already orchestrating.

#### 1. Scripts as CLI Functions

Define parameters, default values, and validation directly in your `.toml`.

```toml
[scripts]
deploy = "terraform apply -var 'env=<axes::params::0(default='staging')>'"
```

```sh
axes . deploy                # -> terraform apply -var 'env=staging'
axes . deploy production     # -> terraform apply -var 'env=production'
```

#### 2. Effortless Cross-Platform Orchestration

Define a command once, and it will work across your entire team.

```toml
[scripts.browse]
desc = "Opens the local documentation in the browser."
windows = "start http://localhost:8080"
macos = "open http://localhost:8080"
linux = "xdg-open http://localhost:8080"
```

```sh
# One command to rule them all.
$ axes . browse
```

No more `if (os == "win32")` in your scripts. `axes` gives you an operating system abstraction layer.

#### 3. Composition and Reuse

Build complex workflows from simple pieces.

```toml
[scripts]
build = "npm run build"
test = "npm run test"
quality = ["<axes::scripts::test>", "<axes::scripts::build>"]
```

```sh
axes . quality  # Executes the tests and THEN the build.
```

#### 4. Immersive Focus Sessions

Dive into a sub-project. `axes` sets up your environment for you.

```toml
# in my-app/api/.axes/axes.toml
[options]
at_start = "source .venv/bin/activate" # Executes upon entry
at_exit = "docker-compose down"       # Executes upon exit
```

```sh
$ axes my-app/api # Starts the session. `at_start` executes automatically.

(axes: my-app/api) $ axes test  # You don't need to repeat the context.
(axes: my-app/api) $ exit       # `at_exit` executes automatically.
```

**Your development environment, on demand.**

#### 4. Performance Oriented

`axes` is written in Rust and aims to offer all these powerful features with the minimum possible resource expenditure. With a **lazy and persistent cache**, the first execution of a complex script might take longer, but subsequent executions will be infinitely faster and imperceptible, making CI/CD infinitely more powerful.

---

### Ready to Conduct Your Own Orchestra?

The friction you feel every day is not a requirement of software development. It is a problem with a solution. `axes` is that solution.

- ➡️ **[Quick Start Guide (`GETTING_STARTED.md`)](./GETTING_STARTED.md):** Your step-by-step tutorial to build your first orchestrated monorepo in 15 minutes.
- 📖 **[Mastering `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** The definitive reference for every feature.
- ⌨️ **[Command Reference (`COMMAND.md`)](./COMMAND.md):** A complete guide to every CLI command.

### Join the Workflow Revolution

`axes` is more than a tool; it's a movement to restore control and consistency to developers. But we can't do it alone.

Whether you are a novice programmer seeking order in your personal projects, a senior developer optimizing your company's `CI/CD`, or an independent team needing a common language, your voice matters.

- **Found a Bug or Have a Great Idea:** [**Open an Issue**](https://github.com/RetypeOS/axes/issues)
- **Want to Contribute Code:** Pull Requests are welcome!

**Install `axes` today. Stop searching for commands. Forget the small problems. Focus on what truly matters: **bringing your software to life**, and let `axes` worry about the how.**
