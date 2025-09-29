<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./AXES_TOML_GUIDE.md">English</a> •
  <a href="./docs/es/AXES_TOML_GUIDE.md">Español</a>
</p>

# Mastering `axes.toml`: The Definitive Guide

The `axes.toml` file is the brain of each of your projects. This is where you transform chaotic command sequences into clean, reusable, and powerful workflows. This guide is the complete reference for every section and feature you can use.

## The Fundamental Principle: Inheritance

Before diving into the details, remember the most important concept: **inheritance**.

Every `axes` project inherits the complete configuration from its parent project. When `axes` executes a command in the context of `my-app/api`, it first reads the `axes.toml` of `my-app/api`, then "merges" the configuration of `my-app` below it, and finally that of `global`.

This means a child project can:

* **Use** variables and scripts defined in its parents.
* **Override** variables and scripts to specialize behavior.

> **Merge Rule:** The child's configuration always takes precedence. If `my-app` defines `[vars] version = "1.0"` and `my-app/api` defines `[vars] version = "1.1"`, the value for `api` will be `1.1`.

### Anatomy of an `axes.toml`

Here is an example of an `axes.toml` with all the main sections. We will explore them one by one.

```toml
# --- Metadata (Optional) ---
version = "1.0.0"
description = "An example project."

# --- Environment Variables for every execution ---
[env]
NODE_ENV = "development"

# --- Variables to reuse in scripts ---
[vars]
dist_dir = "dist/"

# --- Scripts and Workflows ---
[scripts]
build = "npm run build -- --output <axes::vars::dist_dir>"
serve = "npm run serve"

# --- Options and Hooks ---
[options]
# Executes when starting a session with `axes . start`
at_start = "nvm use 18"
# Executes when exiting the session
at_exit = "echo 'Cleaning up session...'"

# Configuration for the `axes . open` command
[options.open_with]
editor = "code \"<axes::path>\""
default = "editor"
```

---

## 1. Metadata (Optional)

These keys are purely informational and help document your project.

* `version`: The version of your project (e.g., `"1.0.0"`). It is accessible in scripts via the `<axes::version>` token.
* `description`: A brief description of what the project does. It is shown in commands like `info`.

```toml
version = "2.1.0-beta"
description = "The main authentication service."
```

---

## 2. Interpolation Variables `[vars]`

The `[vars]` section is your best tool for following the **DRY (Don't Repeat Yourself)** principle. Define values here once and reuse them in multiple scripts.

**Definition:**

```toml
[vars]
output_dir = "build/release"
compiler_flags = "--optimization-level 3 -DNDEBUG"
```

**Usage:**
To use a variable, use the syntax `<axes::vars::variable_name>`. `axes` will replace the token with the variable's value before executing the command.

```toml
[scripts]
# Uses the variables defined above.
build = "c++ <axes::vars::compiler_flags> -o <axes::vars::output_dir>/app main.cpp"
```

Variables can also compose each other and use other `axes` tokens:

```toml
[vars]
# The artifact directory depends on the project name.
artifact_dir = "artifacts/<axes::name>"
# The final file name is composed of another variable.
final_zip = "<axes::vars::artifact_dir>/<axes::name>.zip"
```

## 3. Scripts and Workflows `[scripts]`

This is the main section of `axes`. A "script" is a named entry point for a task you want to perform. Each key in the `[scripts]` table defines a command you can run with `axes <ctx> <script_name>`.

`axes` offers an incredibly flexible syntax, allowing you to define everything from a simple alias to a complex cross-platform workflow.

### 3.1. Command Syntax

You can define a command in several ways, from the simplest to the most complete.

#### **A. Simple Command (String)**

The most basic form. `axes` will treat it as the default command for your current operating system.

```toml
[scripts]
# Checks the code for errors without compiling.
check = "cargo check"

# Starts a simple development server.
serve = "python -m http.server 8000"
```

#### **B. Command Sequence (Array of Strings)**

For workflows that require multiple steps, define the script as a list of strings. `axes` will execute each command in order and stop if any of them fail (unless you use modifiers).

```toml
[scripts]
# A complete build and deploy flow for a static web application.
deploy = [
    "echo 'Cleaning up previous builds...'",
    "rm -rf ./dist",
    "echo 'Building the application...'",
    "npm run build",
    "echo 'Deploying to the server...'",
    "scp -r ./dist/* user@server:/var/www/my-app",
    "echo '🚀 Deployment complete!'"
]
```

#### **C. Extended Structure (Table)**

To add a description or define cross-platform behavior, use a TOML table.

* **With Description:**

    ```toml
    [scripts]
    lint = { desc = "Runs the linter to find style issues.", run = "eslint ." }
    test = { desc = "Runs the complete test suite.", run = ["npm run test:unit", "npm run test:e2e"] }
    ```

    The `desc` will be shown in commands like `axes . info`. The `run` key can be a string or an array, as in the previous cases.

* **Cross-Platform:**
    Define a single script that behaves differently depending on the operating system. `axes` will automatically select the correct command.

    ```toml
    [scripts.browse]
    desc = "Opens the local documentation in the default browser."
    windows = "start http://localhost:8080"
    macos = "open http://localhost:8080"
    linux = "xdg-open http://localhost:8080"
    # `default` is used if the current OS does not match any of the above.
    default = "echo 'Visit http://localhost:8080 in your browser.'"
    ```

### 3.2. Execution Modifiers (`-` and `>`)

You can control how each line in a sequence is executed using special prefixes.

> **Key Rule:** Modifiers only take effect on the line where they are written. They are **not "inherited"** when a script is composed by another. Execution control always belongs to the "calling" script.

#### **Ignore Errors with `-`**

Normally, if a command in a sequence fails, the entire sequence stops. Sometimes, you want a command to run but you don't care if it fails. Prefix that command with `-` so that `axes` ignores its exit code and continues with the next step.

```toml
[scripts]
# Tries to clean the cache, but doesn't fail if the directory doesn't exist.
build = [
    "-rm -rf .cache",
    "npm run build"
]
```

Here, if `rm` fails, `axes` will continue and run `npm run build`.

#### **Parallel Execution with `>`**

If you prefix a command with `>` in a sequence, `axes` launches it and immediately continues with the next, without waiting for it to finish. This is ideal for starting long-running processes like development servers or watchers.

```toml
[scripts]
# Starts the backend and frontend servers simultaneously.
dev = [
    "> axes api dev",
    "> axes frontend dev"
]
```

When running `axes . dev`, `axes` will launch the `dev` script of `api` and, an instant later, the `dev` script of `frontend`. `axes` will wait for all processes launched in parallel to finish before concluding the main task.

### 3.3. Script Composition: The Heart of Reusability

One of the most powerful features of `axes` is the ability to build complex scripts from smaller, reusable pieces.

* **Syntax:** `<axes::scripts::other_script_name>`

When `axes` expands your scripts, it will replace this token with the **pure text content** of the referenced script.

**Example of a Code Quality Flow:**

```toml
# in `my-app/.axes/axes.toml` (the parent)
[scripts]
# Reusable base scripts
lint = { desc = "Runs the linter.", run = "ruff check ." }
test = { desc = "Runs the test suite.", run = "pytest" }

# Composed script that joins the previous ones.
# Execution control (sequential) belongs to `quality`.
quality = [
    "echo '🚀 Running all quality checks...'",
    "<axes::scripts::lint>",
    "<axes::scripts::test>",
    "echo '✅ All good!'"
]
```

Now, a simple `axes my-app quality` runs `ruff check .` and then `pytest`. If tomorrow you decide that `lint` should run in parallel, you would modify `quality`:

```toml
# Modifying `quality` so that `lint` doesn't block (hypothetical example)
quality = [
    "> <axes::scripts::lint>",
    "<axes::scripts::test>"
]
```

The `>` is applied to the *result* of the `<axes::scripts::lint>` expansion. The original definition of `lint` does not change and can still be used sequentially in other scripts.

## 4. The Expansion Engine: Giving Superpowers to Your Scripts

The feature that ties everything together in `axes` is its token expansion engine. Any string value in your `axes.toml` (in `scripts`, `vars`, `options`, etc.) can contain special tokens in the format `<axes::...>` that `axes` will process before executing the command.

This system allows you to create dynamic, composable, and context-aware workflows. Expansion happens lazily, and its results are saved in a binary cache (`.axes/config.cache.bin`), making subsequent executions extremely fast.

### 4.1. Static Tokens (Metadata and Variables)

These tokens resolve to simple text values and are injected before anything else.

#### **Project Metadata Tokens**

These tokens give you access to the intrinsic information of the project.

| Token             | Expansion Value                                                     | Usage Example                                                  |
| :---------------- | :------------------------------------------------------------------ | :-------------------------------------------------------------- |
| `<axes::name>`    | The full qualified name of the project.                             | `echo 'Building <axes::name>...'` -> `Building my-app/api...`             |
| `<axes::path>`    | The physical path (absolute and clean) to the project root directory. | `docker build -t app . -f "<axes::path>/Dockerfile"`                             |
| `<axes::uuid>`    | The project's universal unique identifier.                          | `aws s3 cp ... s3://bucket/<axes::uuid>/`                                        |
| `<axes::version>` | The version defined in the project's `axes.toml`.                   | `echo 'Deploying version <axes::version>'` -> `Deploying version 1.2.0-beta`         |

#### **Variable Tokens**

These tokens allow you to inject the values you have defined in the `[vars]` and `[env]` sections.

* **`<axes::vars::variable_name>`:** Expands to the value of the variable defined in the `[vars]` section. `axes` will look for the variable in the current project's `axes.toml` and then move up the inheritance tree until it finds it.
* **`<axes::env::VARIABLE_NAME>`:** Expands to the value of the variable defined in `[env]`. It works the same as `vars` at the inheritance level.

**Combined Example:**

```toml
# in the parent `my-app`'s `axes.toml`
[vars]
docker_registry = "registry.example.com/my-org"

# in the child `my-app/api`'s `axes.toml`
[scripts]
# Builds and tags a Docker image with the project name and the parent's registry.
docker_build = "docker build -t <axes::vars::docker_registry>/<axes::name>:<axes::version> ."
```

### 4.2. Composition Tokens (Scripts and Nested Variables)

This is one of the most powerful features. You can build complex workflows from smaller pieces.

* **`<axes::scripts::other_script_name>`:** `axes` will replace this token with the **pure text content** of the `other_script_name` script (already resolved for your platform). Execution prefixes (`-`, `>`) of the nested script **are not inherited**; execution control always belongs to the script making the call.

**Example of a Code Quality Flow:**

```toml
# in `my-app/.axes/axes.toml` (the parent)
[vars]
python_files = "./src"

[scripts]
lint = "pylint <axes::vars::python_files>"
test = "pytest <axes::vars::python_files>"

# Composed script that joins the previous ones.
quality = [
    "echo '🚀 Running all quality checks...'",
    "<axes::scripts::lint>",
    "<axes::scripts::test>",
    "echo '✅ All good!'"
]
```

A simple `axes my-app quality` runs a complete workflow. If you decide the linter is optional, you only modify `quality`: `"-<axes::scripts::lint>"`.

### 4.3. Execution and Substitution: `<axes::run::...>`

Sometimes, you need the **result** of a command to use it in another. The `<axes::run::...>` token allows you to do exactly that.

* **`<axes::run::command_to_execute>`:** `axes` will execute `command_to_execute`, capture its standard output (stdout), clean it up (removing trailing spaces and newlines), and inject it into the main command.

**Example: Docker Tagging with Git Hash:**

```toml
[scripts]
# A private script to get the version.
_get_git_version = "git rev-parse --short HEAD"

# Builds the Docker image, using the output of the previous script as the tag.
# Note how we compose an <axes::scripts::...> inside an <axes::run::...>.
build_and_tag = "docker build -t my-app:<axes::run::<axes::scripts::_get_git_version>> ."
```

When running `axes . build_and_tag`:

1. `axes` sees the `<axes::run::...>` token and first expands its content.
2. `<axes::scripts::_get_git_version>` expands to `"git rev-parse --short HEAD"`.
3. `axes` executes `git rev-parse --short HEAD`.
4. The git output (e.g., `a1b2c3d`) is captured.
5. The final command is built as `docker build -t my-app:a1b2c3d .` and executed.

## 5. Scripts as Functions: The Parameter System (`<axes::params::...`)

`axes` doesn't just run scripts; it allows you to define true command-line "functions" that accept arguments in a structured way. This eliminates the need to write complex `bash` scripts to parse flags and parameters.

All parameter logic is controlled through the `<axes::params::...>` namespace and follows a **declarative paradigm**: you define the parameters your script expects, and `axes` validates the user input **before** executing anything.

> **Golden Rule:** If you pass arguments to a script from the command line (`axes . my-script arg1 --flag`), that script's `axes.toml` **must** use `<axes::params::...>` tokens to consume them. If any arguments remain unconsumed by any token (and there is no generic `<axes::params>` token), `axes` will return an error.

### 5.1. Positional Parameters

These are arguments passed without a flag. They are accessed by their index (starting at 0).

* **Basic Syntax:** `<axes::params::0>`, `<axes::params::1>`, etc.
* **Behavior:** Replaced by the positional argument at that index. If the argument does not exist and is not required or does not have a `default`, it is replaced by an empty string.

#### **Modifiers for Positionals `(...)`**

* `required`: Execution fails if the argument is not provided.
* `default='value'`: Provides a default value if the argument is not passed in the CLI.
* `map='--new-flag'`: Transforms the positional argument into a flag with a value. If `my-value` is provided, the token expands to `"--new-flag my-value"`.

**Example: A simplified `git commit` script.**

```toml
[scripts]
# Accepts a commit message as the first required positional argument.
commit = "git commit -m \"<axes::params::0(required)>\""
```

**Execution:**

```sh
# The '0' refers to "Fix: ..."
axes . commit "Fix: Fix authentication bug"

# Command executed:
# git commit -m "Fix: Fix authentication bug"

# Fails if not provided:
axes . commit
# -> Error: Positional argument at index 0 is required but was not provided.
```

### 5.2. Named Parameters (Flags)

You can make your scripts react to flags (`--name`) passed from the CLI.

* **Basic Syntax:** `<axes::params::flag-name>`
* **Default Behavior (Pass-through):** The token looks for the flag in the CLI and reinjects it as is, along with its value if it has one. If not found, it expands to an empty string.

#### **Modifiers for Flags `(...)`**

* `required`: Execution fails if the flag (or its alias) is not present.
* `default='value'`: If the flag is **not provided at all**, this `default` will be used. It also applies if the flag is provided **without a value** (e.g., `command --my-flag`).
* `alias='-a'`: Allows the flag to be recognized by a short alias. `axes` will throw an error if the user provides both the full name and the alias.
* `map='--new-name'`: Replaces the flag name in the output. Very useful for abstracting underlying tools.
* `map=''`: A special case. Indicates that you only want to inject the **value** of the flag, not the flag name itself. Ideal for injecting values in positions where a flag is not expected.

**Example: A `test` script that can pass a `--marker` flag to `pytest`.**

```toml
[scripts]
# Uses the default pass-through with an alias.
test = "pytest <axes::params::marker(alias='-m')>"
```

**Execution:**

```sh
# Runs all tests
axes . test
# Command executed: `pytest`

# Runs only tests marked as 'slow'
axes . test --marker slow
# Command executed: `pytest --marker slow`

# Uses the alias
axes . test -m smoke
# Command executed: `pytest -m smoke`
```

**Example: A `deploy` script with `map` and `default`.**

```toml
# axes.toml
[scripts]
# The internal script expects --environment, but we expose --env to the user.
# By default, it deploys to 'staging'.
deploy = "terraform apply -var 'env=<axes::params::env(map='', default='staging')>'"
```

**Execution:**

```sh
# Uses the default
axes . deploy
# Command executed: terraform apply -var 'env=staging'

# Specifies an environment
axes . deploy --env production
# Command executed: terraform apply -var 'env=production'
```

### 5.3. The Generic Collector: `<axes::params>`

This is the "collector" token. It is useful when you want to pass a variable number of arguments or flags to an underlying command without having to explicitly define them all.

* **Syntax:** `<axes::params>`
* **Behavior:** Replaced by **all arguments** (positional and named) that **were not consumed** by an explicit token (`::0`, `::flag`, etc.), maintaining their original order.

**Example: A generic `wrapper` for `npm install` that also defines `--save-dev`.**

```toml
[scripts]
# `add` passes all remaining arguments to `npm install`.
# `add_dev` first defines `--save-dev`, and then passes the rest.
add = "npm install <axes::params::save-dev(alias='-D')> <axes::params>"
```

**Execution:**

```sh
# Installs a normal dependency
axes . add react
# Command executed: `npm install react`

# Installs a development dependency
axes . add -D typescript
# `-D` is consumed by <...::save-dev> and expands to `--save-dev`.
# `typescript` is consumed by <axes::params>.
# Command executed: `npm install --save-dev typescript`

# Installs multiple dependencies with additional flags
axes . add react react-dom --force
# Command executed: `npm install react react-dom --force`
```

By combining these patterns, you can build incredibly rich and robust command-line interfaces for your projects, all within the simplicity of `axes.toml`.

## 6. Environment Options and Hooks

In addition to scripts, `axes` allows you to define configurations that affect how all commands are executed and how interactive sessions behave.

### 6.1. Environment Variables `[env]`

Any key-value pair you define in the `[env]` section will be injected as an environment variable into the subprocess where your scripts are executed. This is ideal for setting up credentials, database URLs, or behavior flags for your tools. `[env]` variables are inherited and merged from parent to child.

```toml
# in the root project `my-app`'s `axes.toml`
[env]
DATABASE_URL = "postgres://user:pass@localhost/db"
APP_ENV = "development"

# in the child `my-app/api-tests`'s `axes.toml`
[env]
# Overrides the parent variable only for this test context.
APP_ENV = "testing"
```

### 6.2. Session Options and Hooks `[options]`

The `[options]` section allows you to customize the behavior of the `start` and `open` commands.

#### **Session Hooks: `at_start` and `at_exit`**

These are scripts that run automatically when entering and exiting an interactive session (`axes <ctx> start`).

* **`at_start`**: A command (or sequence) that executes **before** you get terminal control in a session. Perfect for activating virtual environments, setting session variables, or starting services.
* **`at_exit`**: A command (or sequence) that executes **after** you exit the session. Ideal for cleanup tasks.

**Important:** Since v0.1.8, `at_start` and `at_exit` are **full scripts**. They can be sequences, have descriptions, and most importantly, **accept parameters** passed to the `start` command.

#### **Example: A Python Environment with Docker and Parameters**

```toml
[options]
at_start = { desc = "Activates venv and spins up the DB.", run = [
    "source .venv/bin/activate",
    "docker-compose up -d <axes::params::service(default='db')>"
]}
at_exit = { desc = "Stops and removes containers.", run = "docker-compose down" }
```

**Execution:**

```sh
# Starts the session and spins up the default 'db' service
axes . start

# Starts the session and specifies which service to spin up
axes . start --service web
```

#### **Shell Customization: `shell`**

By default, `axes` attempts to use your system's default shell. You can force the use of a specific shell for a project.

```toml
[options]
# Uses zsh for this project.
shell = "zsh"
```

#### **`open` Command Configuration: `[options.open_with]`**

This sub-section allows you to define shortcuts for the `axes <ctx> open` command. Like session hooks, each shortcut is a **full script** and can accept parameters.

**Complete Example:**

```toml
[options.open_with]
# `edit` shortcut to open in VS Code.
edit = { desc = "Opens the project in VS Code.", run = "<axes::vars::editor_cmd> \"<axes::path>\"" }

# `files` shortcut for the file explorer.
files = { desc = "Opens the directory in the file explorer.", run = "explorer \"<axes::path>\"" } # `explorer` on Windows, `open` on macOS, `xdg-open` on Linux

# `terminal` shortcut that accepts a parameter to open a subfolder.
terminal = "wt -d \"<axes::path>/<axes::params::0(default='.')>\"" # `wt` is Windows Terminal

# Defines `edit` as the default action when running `axes . open`.
default = "edit"

[vars]
editor_cmd = "code"
```

**Execution:**

```sh
# Opens the project with the default editor ('edit')
axes . open

# Opens the file explorer
axes . open files

# Opens a new terminal in the 'src' subdirectory
axes . open terminal src
```

With this configuration in your `global` project, all your projects will inherit these very useful `open` shortcuts.

---

## Conclusion

You now have the complete knowledge to write powerful and well-structured `axes.toml` files. You have learned to:

* Define **variables** to reuse values.
* Create simple, sequential, and cross-platform **scripts**.
* Use the **`<axes::...>` expansion engine** to compose scripts and use metadata.
* Create **parameterizable scripts** that act as CLI functions.
* Configure the **execution environment** and **session hooks**.

The next step is to explore the reference for all CLI commands to see how they interact with your projects.

➡️ **Next Recommended Reading: [Complete Command Reference (`COMMANDS.md`)](./COMMANDS.md)**