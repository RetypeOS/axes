<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./AXES_TOML_GUIDE.md">English</a> •
  <a href="./docs/es/AXES_TOML_GUIDE.md">Español</a>
</p>

# Mastering `axes.toml`: The Definitive Guide

The `axes.toml` file is the brain of each of your projects. This is where you transform chaotic command sequences into clean, reusable, and powerful workflows. This guide is the complete reference for every section and feature you can use.

## The Fundamental Principle: Inheritance

Before diving into the details, remember the most important concept: **inheritance**.

Every `axes` project inherits the complete configuration from its parent. When `axes` needs a value (like a script or variable), it follows a clear search path:

1. It looks in the **current project's** `axes.toml`.
2. If not found, it looks in the **parent's** `axes.toml`.
3. ...and so on, up to the **`global`** project.

The **first value found wins**. This means a child's configuration always takes precedence and can **override** its parents' definitions. Environment variables (`[env]`) are the only exception: they are **merged**, with child values overriding parent values for the same key.

### Anatomy of an `axes.toml`

Here is an example of an `axes.toml` with all the main sections. We will explore them one by one.

```toml
# ==============================================================================
# axes.toml: Complete Reference Guide
# This file serves as an exhaustive example of all features available in `axes`.
# ==============================================================================

# --- 1. Metadata (Optional) ---
# Provides information about the project, visible with `axes info`.
version = "2.0.0"
description = "Backend API for the WebApp project. Provides data endpoints."

# --- 2. Environment Variables ([env]) ---
# These variables are injected as system environment variables into EVERY command executed by `axes` in this context.
[env]
# Ideal for secrets (if defines on exterior ancestor project) or environment constants.
DATABASE_URL = "postgresql://user:pass@localhost:5432/webapp_db"
LOG_LEVEL = "info"

# --- `axes` Variables ([vars]) ---
# Internal variables for reuse within scripts using the `<vars::...>` syntax.
# They promote the DRY (Don't Repeat Yourself) philosophy.
[vars]
image_name = "webapp/api"
# Variables can be dynamic, executing a command in real-time.
git_hash = "<run('git rev-parse --short HEAD')>"

# --- 4. Scripts ([scripts]) ---
# The core of `axes`. Defines the project's workflows.
[scripts]

# Simple form: a single command as a text string.
run = "poetry run uvicorn app.main:app --reload"

# Sequence form: a list of commands executed sequentially.
# Use '#' to print status messages without invoking a shell.
test = [
    "# Running API tests...",
    "poetry run pytest"
]

# Extended form: a dictionary with a description (`desc`) and the command (`run`).
# This improves the output of `axes info` and `axes run` (without arguments).
[scripts.seed_db]
desc = "Populates the database with test data."
run = [
  "# Applying seeds to the database...",
  # `run` can contain cross-platform lines. `axes` will choose the correct one.
  # If the OS-specific one doesn't exist, it falls back to `default`.
  { windows = "psql.exe -U user -d webapp_db -f ./seed.sql", default = "psql -U user -d webapp_db -f ./seed.sql" }
]

# Script with a named parameter (`tag`) that has a default value.
[scripts.build]
desc = "Builds the local Docker image."
run = "docker build . -t <vars::image_name>:<params::tag(default='latest')>"

# Script that delegates argument parsing to the shell using the '$' prefix.
# Allows passing flags and arguments directly to the underlying command.
# Example usage: `axes format --check .` becomes `poetry run ruff format .`
[scripts.format]
desc = "Formats the code using Ruff."
run = "$ poetry run ruff format ."

# A complex script demonstrating composition and command modifiers.
[scripts.deploy]
desc = "Builds and pushes the API Docker image."
run = [
  "# Step 1: Build the image (silent execution, command not printed).",
  "@ <scripts::build>", # <-- Composition: calls another `axes` script.

  "# Step 2: Tag the image with the commit hash (ignores errors if the tag already exists).",
  "- docker tag <vars::image_name>:latest <vars::image_name>:<vars::git_hash>",

  "# Step 3: Push both tags in parallel for maximum speed.",
  "> docker push <vars::image_name>:latest", # <-- The `>` prefix starts a parallel batch.
  "> docker push <vars::image_name>:<vars::git_hash>"
]


# --- 5. Session Options and Hooks ([options]) ---
[options]

# `at_start`: Executes once when starting a session with `axes start`.
# Ideal for activating virtual environments, starting services, etc.
at_start = "poetry install --no-root"

# `at_exit`: Executes upon exiting the session (with `exit`).
# Ideal for stopping services, cleaning temporary files, etc.
at_exit = "# Exiting API session..."

# Configuration for the `axes open` command.
[options.open_with]
# Define "shortcuts" to open the project in different applications.
# `<path>` is a special token that resolves to the project's root path.
editor = "code \"<path>\""
terminal = { windows = "wt -d \"<path>\"", default = "gnome-terminal --working-directory=\"<path>\""}

# `default` specifies which shortcut to use if `axes open` is run without arguments.
default = "editor"
```

---

## 1. Metadata (Optional)

These keys are purely informational and help document your project.

* `version`: The version of your project (e.g., `"1.0.0"`). It is accessible in scripts via the `<version>` token.
* `description`: A brief description of what the project does. It is shown in commands like `info`.

```toml
version = "2.1.0-beta"
description = "The main authentication service."
```

---

## 2. Interpolation Variables `[vars]`

The `[vars]` section is your tool for DRY (Don't Repeat Yourself) code. Define values once and reuse them across multiple scripts via the `<vars::...>` token.

### Variable Definition

Variables must resolve to a single-line value.

**A. Simple Form (String):**

```toml
[vars]
image_name = "my-app/api"
```

**B. Extended Form (Table):**
Use a table to add a description or define platform-specific values. You **must** use the `value` key.

```toml
[vars.binary_path]
desc = "Path to the compiled application binary."
value = { windows = "target\\release\\app.exe", default = "target/release/app" }
```

**Usage:**

```toml
[scripts]
run = "<vars::binary_path> --serve"
```

## 3. Scripts and Workflows `[scripts]`

This is the core of `axes`, where you define your project's tasks. Each key in the `[scripts]` table becomes a command you can run.

### 3.1. Command Syntax

`axes` provides a highly flexible syntax for defining scripts, from simple one-liners to complex, cross-platform workflows.

#### **A. Simple Command (String)**

The most basic form. A single string to be executed.

```toml
[scripts]
test = "cargo test -- --nocapture"
```

#### **B. Command Sequence (Array)**

For multi-step workflows. `axes` executes each command in order and stops if any fail.

```toml
[scripts]
deploy = [
    "# 1. Building assets...",
    "npm run build",
    "# 2. Publishing to server...",
    "scp -r ./dist/* user@server:/var/www/my-app",
]
```

**or:**

```toml
[scripts]
deploy = [
    {default = "# 1. Building assets..."},
    {default = "npm run build"},
    {windows = "# 2. Publishing to server on Windows...", macos = "# 2. Publishing to server on Mac OS...", linux = "# 2. Publishing to server on Linux...", default = "# 2. Publishing to server on another OS..."},
    {default = "scp -r ./dist/* user@server:/var/www/my-app"},
]
```

Each item in the array can be a `String` or a `Platform Block` (see below).

> **Note:** `TOML` files do not allow lists of different types, so if you use this syntax, the entire script must be of dictionary or string type; they cannot be combined.

#### **C. Extended Structure (Table)**

To add a description or use more advanced syntax, define the script as a TOML table.

* **With `run` key:**

    ```toml
    [scripts.lint]
    desc = "Runs the linter to find style issues."
    run = "eslint ." # `run` can be a String or an Array
    ```

* **Direct Platform Keys (for single-line scripts):**
    This is the recommended, ergonomic syntax for cross-platform commands. The `run` key is not needed.

    ```toml
    [scripts.browse]
    desc = "Opens local documentation in the default browser."
    windows = "start http://localhost:8080"
    macos = "open http://localhost:8080"
    linux = "xdg-open http://localhost:8080"
    # `default` is a fallback for other systems.
    default = "echo 'Visit http://localhost:8080 in your browser.'"
    ```

* **A**

* **Table array for complex secuences (multiline scripts):**
    This is the most powerful and explicit syntax. It is ideal for multi-line scripts where one or more lines have platform logic. Use `[[scripts.name.run]]` for each step in the sequence.

    ```toml
    [scripts.browse]
    desc = "Opens local documentation in the default browser."

    [[run]]
    default = "# --- Starting server... please wait... ---"

    [[run]]
    windows = "start http://localhost:8080"
    macos = "open http://localhost:8080"
    linux = "xdg-open http://localhost:8080"
    default = "echo 'Visit http://localhost:8080 in your browser.'"

    [[run]]
    default = "# --- Server opened! ---"
    ```

> **Note** We strongly recommend that you learn the optimal ways to define these structures in TOML; there are other more optimal ways to define this data.

The `desc` field is highly recommended as it improves the output of `axes info` and `axes run`.

### **3.2. Execution Modifiers (Prefixes)**

You can control how each line in a sequence is executed using special prefixes. They can be combined (e.g., `>- @ my_command`).

> **Key Rule:** Modifiers only take effect on the line where they are written. They are **not "inherited"** when a script is composed by another. Execution control always belongs to the "calling" script.

| Prefix | Name                  | Description                                                                                                   |
| :----- | :-------------------- | :------------------------------------------------------------------------------------------------------------ |
| `-`    | **Ignore Errors**     | `axes` will continue to the next command in a sequence even if this one fails (exits with a non-zero code).    |
| `>`    | **Parallel Execution**|  Groups this command with all subsequent `>` commands into a **batch**. `axes` executes all commands in the batch concurrently and **waits for all of them to finish** before proceeding to the next sequential command.       |
| `@`    | **Silent Mode**       | `axes` will not print the command (`→ my_command`) to the console before executing it. Useful for clean output. |
| `#`    | **Echo Mode**         | The entire line is treated as a string to be printed to the console, not as a command to be executed.         |
| `\|`   | **Terminator**        | Explicitly tells the prefix parser to stop. Useful for commands that start with a special character.        |

#### **Examples of Modifiers**

**Ignore Errors (`-`):**

```toml
[scripts]
# Tries to clean the cache, but doesn't fail if the directory doesn't exist.
build = [
    "-rm -rf .cache",
    "npm run build"
]
```

**Parallel Execution (`>`):**

```toml
[scripts]
# Starts the backend and frontend servers simultaneously.
dev = [
    "> axes api dev",
    "> axes frontend dev"
]
```

**Silent & Echo Mode (`@`, `#`):**

```toml
[scripts]
setup = [
    "# --- Setting up environment ---", # This line will be printed.
    "@source ./.env",                  # This command will run, but not be shown.
    "# Environment ready."
]
```

**Terminator (`|`):**

```toml
[scripts]
# The `-v` is a flag for `my_tool`, not a modifier for `axes`.
advanced = ">| -v --some-flag"
```

### 3.3. Script Composition: The Heart of Reusability

One of the most powerful features of `axes` is its ability to build complex scripts from smaller, reusable pieces by expanding tokens **before** execution.

* **Syntax:** `<scripts::other_script_name>`

When `axes` prepares your scripts, it **structurally composes** them. If you call a multi-line script, its commands are inserted directly into the parent's command list.

**Example of a Code Quality Flow:**

```toml
# in `my-app/.axes/axes.toml` (the parent)
[scripts]
# Reusable base scripts
lint = { desc = "Runs the linter.", run = "ruff check ." }
test = { desc = "Runs the test suite.", run = ["pytest tests/unit", "pytest tests/integration"] }

# Composed script that joins the previous ones.
# Execution control (sequential, parallel) belongs to `quality`.
quality = [
    "# Running all quality checks...",
    "<scripts::lint>",
    "> <scripts::test>", # `test` itself is sequential, but `quality` runs it in parallel.
]
```

Running `axes quality` will execute `ruff check .`, and once it finishes, it will launch both `pytest` commands in parallel.

## 4. The Expansion Engine: Supercharging Your Scripts

The feature that ties everything together is its token expansion engine. Any string value in your `axes.toml` can contain special tokens in the format `<...>` that `axes` will process.

Expansion happens lazily, and its results are saved as a pure Abstract Syntax Tree (AST) in a binary cache (`.axes/config.cache.bin`), making subsequent executions extremely fast.

### 4.1. Static Value Tokens

These tokens are resolved to their final values during the expansion (JIT compilation) phase.

#### **Project Metadata Tokens**

| Token             | Expansion Value                                                     |
| :---------------- | :------------------------------------------------------------------ |
| `<name>`          | The full qualified name of the project (e.g., `my-app/api`).        |
| `<path>`          | The absolute physical path to the project root directory.           |
| `<uuid>`          | The project's universal unique identifier.                          |
| `<version>`       | The version defined in the project's `axes.toml`.                   |

#### **Variable Tokens**

* **`<vars::variable_name>`:** Expands to the value of the variable defined in the `[vars]` section.

**Combined Example:**

```toml
# in the parent `my-app`'s `axes.toml`
[vars]
docker_registry = "registry.example.com/my-org"

# in the child `my-app/api`'s `axes.toml`
[scripts]
docker_build = "docker build -t <vars::docker_registry>/<name>:<version> ."
```

### 4.2. Dynamic Execution Token: `<run::(...)>`

Sometimes, you need the **result** of a command to use it in another.

* **Syntax:** `<run('command_to_execute')>`
* **Behavior:** `axes` executes `command_to_execute` **at runtime**, captures its standard output (stdout), cleans it up (removing trailing whitespace), and injects it into the main command.

> **Important:** The output of `run` tokens is **never** cached to ensure the data is always fresh.

**Example: Docker Tagging with Git Hash:**

```toml
[scripts]
tag_release = "docker tag my-app:latest my-app:<run('git rev-parse --short HEAD')>"
```

When running `axes tag_release`:

1. `axes` prepares to execute the `tag_release` script.
2. It encounters the `<run::(...)>` token.
3. It executes `git rev-parse --short HEAD`.
4. The git output (e.g., `a1b2c3d`) is captured.
5. The final command is assembled as `docker build -t my-app:a1b2c3d .` and then executed.

### 4.3. Runtime Parameter Tokens: `<params::...>`

This special family of tokens is not expanded beforehand. They are placeholders that are resolved at the very last moment by the `task_executor`, using the arguments you provide on the command line.

(This is covered in depth in the next section.)

## 5. Scripts as Functions: The Parameter System (`<params::...`)

`axes` doesn't just run scripts; it allows you to define true command-line "functions" that accept arguments in a structured way. This eliminates the need to write complex `bash` scripts to parse flags and parameters.

All parameter logic is controlled through the `<params::...>` namespace and follows a **declarative paradigm**: you define the parameters your script expects, and `axes` validates the user input **before** executing anything.

> **Golden Rule:** If you pass arguments to a script from the command line (`axes my-script arg1 --flag`), that script's `axes.toml` **must** use `<params::...>` tokens to consume them. If any arguments remain unconsumed by any token (and there is no generic `<params>` token), `axes` will return an error.

### 5.1. Positional Parameters

These are arguments passed without a flag. They are accessed by their index (starting at 0).

* **Basic Syntax:** `<params::0>`, `<params::1>`, etc.
* **Behavior:** Replaced by the positional argument at that index. If the argument does not exist and is not required or does not have a `default`, it is replaced by an empty string.

#### **Modifiers for Positionals `(...)`**

* `required`: Execution fails if the argument is not provided.
* `default='value'`: Provides a default value if the argument is not passed in the CLI.
* `map='--new-flag'`: Transforms the positional argument into a flag with a value. If `my-value` is provided, the token expands to `"--new-flag my-value"`.

**Example: A simplified `git commit` script.**

```toml
[scripts]
# Accepts a commit message as the first required positional argument.
commit = "git commit -m \"<params::0(required)>\""
```

**Execution:**

```sh
# The '0' refers to "Fix: ..."
axes commit "Fix: Fix authentication bug"

# Command executed:
# git commit -m "Fix: Fix authentication bug"

# Fails if not provided:
axes commit
# -> Error: Positional argument at index 0 is required but was not provided.
```

### 5.2. Named Parameters (Flags)

You can make your scripts react to flags (`--name`) passed from the CLI.

* **Basic Syntax:** `<params::flag-name>`
* **Default Behavior (Pass-through):** The token looks for the flag in the CLI and reinjects it as is, along with its value if it has one. If not found, it expands to an empty string.

#### **Modifiers for Flags `(...)`**

* `required`: Execution fails if the flag (or its alias) is not present.
* `default='value'`: If the flag is **not provided at all**, this `default` will be used. It also applies if the flag is provided **without a value** (e.g., `command --my-flag`).
* `alias='-a'`: Allows the flag to be recognized by a short alias. `axes` will throw an error if the user provides both the full name and the alias.
* `map='--new-name'`: Replaces the flag name in the output. Very useful for abstracting underlying tools.
* `map=' '`: A special case. Indicates that you only want to inject the **value** of the flag, not the flag name itself. Ideal for injecting values in positions where a flag is not expected.

**Example: A `test` script that can pass a `--marker` flag to `pytest`.**

```toml
[scripts]
# Uses the default pass-through with an alias.
test = "pytest <params::marker(alias='-m')>"
```

**Execution:**

```sh
# Runs all tests
axes test
# Command executed: `pytest`

# Runs only tests marked as 'slow'
axes test --marker slow
# Command executed: `pytest --marker slow`

# Uses the alias
axes test -m smoke
# Command executed: `pytest -m smoke`
```

**Example: A `deploy` script with `map` and `default`.**

```toml
# axes.toml
[scripts]
# The internal script expects --environment, but we expose --env to the user.
# By default, it deploys to 'staging'.
deploy = "terraform apply -var 'env=<params::env(map=' ', default='staging')>'"
```

*Note the use of `map=' '` to inject only the value.*

**Execution:**

```sh
# Uses the default
axes deploy
# Command executed: terraform apply -var 'env=staging'

# Specifies an environment
axes deploy --env production
# Command executed: terraform apply -var 'env=production'
```

### 5.3. The Generic Collector: `<params>`

This is the "collector" token. It is useful when you want to pass a variable number of arguments or flags to an underlying command without having to explicitly define them all.

* **Syntax:** `<params>`
* **Behavior:** Replaced by **all arguments** (positional and named) that **were not consumed** by an explicit token (`::0`, `::flag`, etc.), maintaining their original order.

**Example: A generic `wrapper` for `npm install`.**

```toml
[scripts]
# `add` passes all remaining arguments to `npm install`,
# but also explicitly handles a `--save-dev` flag with a `-D` alias.
add = "npm install <params::save-dev(alias='-D')> <params>"
```

**Execution:**

```sh
# Installs a normal dependency
axes add react
# Command executed: `npm install react`

# Installs a development dependency
axes add -D typescript
# `-D` is consumed by <...::save-dev> and expands to `--save-dev`.
# `typescript` is unconsumed and gets collected by <params>.
# Command executed: `npm install --save-dev typescript`

# Installs multiple dependencies with additional flags
axes add react react-dom --force
# Command executed: `npm install react react-dom --force`
```

By combining these patterns, you can build incredibly rich and robust command-line interfaces for your projects, all within the simplicity of `axes.toml`.

> For a complete guide with detailed examples for every parameter type and modifier, please refer to the **[Argument System Guide (`ARG_PARSER.md`)](./ARG_PARSER.md)**.

## 6. Environment Options and Hooks

### 6.1. Environment Variables `[env]`

Any key-value pair in `[env]` is injected as an environment variable into the script's subprocess. They are inherited and can be overridden.

```toml
[env]
DATABASE_URL = "postgres://user:pass@localhost/db"
APP_ENV = "development"
```

### 6.2. Session and Tooling Options `[options]`

This table controls `axes`'s behavior for sessions, opening projects, and more.

```toml
[options]
# Specifies the shell to use for `axes start`. E.g., "bash", "powershell".
shell = "zsh"

# Template for the interactive session prompt. Supports all `axes` tokens.
prompt = "(<#cyan><name><#reset>) 🚀 "

# A custom root directory for all binary cache files. Supports `~` and env vars.
cache_dir = "~/.axes-caches"
```

#### **Session Hooks: `at_start` and `at_exit`**

These are full `axes` scripts that run automatically when entering (`axes start`) and exiting an interactive session.

```toml
[options]
at_start = { desc = "Activates venv and starts services.", run = [
    "source .venv/bin/activate",
    "docker-compose up -d <params::service(default='db')>"
]}
at_exit = "docker-compose down"
```

#### **`open` Command Configuration: `[options.open_with]`**

Define shortcuts for the `axes <ctx> open` command. Each entry is a **full script definition**, allowing descriptions and platform-specific logic.

```toml
[options.open_with]
# Sets the default action for `axes open`.
default = "editor"

# Each key is an `app_key`.
[options.open_with.editor]
desc = "Opens the project in Visual Studio Code."
run = "code \"<path>\""

[options.open_with.terminal]
desc = "Opens a new terminal in the project root."
windows = "wt -d \"<path>\""
default = "gnome-terminal --working-directory=\"<path>\""
```

---

## Conclusion

You now have a complete overview of the `axes.toml` file. By combining these features, you can build powerful, portable, and self-documenting workflows that will supercharge your development productivity.

➡️ **Next Recommended Reading: [Complete Command Reference (`COMMANDS.md`)](./COMMANDS.md)**
