<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./ARG_PARSER.md">English</a> •
  <a href="./docs/es/ARG_PARSER.md">Español</a>
</p>

# Argument System Guide: Scripts as CLI Functions

The `axes` scripting engine allows you to do much more than execute static commands. It allows you to define **scripts that act as command-line functions**, accepting arguments in a structured, declarative, and validated manner.

This guide explains in depth how the new and robust `axes` parsing system works and how to use the `<params::...>` token family to create flexible and powerful workflows.

## The Paradigm: Pre-Definition and Validation

Unlike traditional shell scripts where you have to manually parse `$1` and `$2` (and often fragilely), `axes` adopts a declarative paradigm. You define the parameters your script expects directly where you use them.

Before executing a single line of your script, `axes` performs a complete analysis:

1. **Discovers** all parameter definitions (`<params::...>`) in your script.
2. **Parses** the arguments you provided on the command line.
3. **Validates** that the provided arguments match the definitions, checking requirements, aliases, and conflicts.

Only if this validation is successful does `axes` proceed to assemble and execute your commands. This eliminates an entire class of errors and ensures predictable behavior.

> **The Golden Rule:** If, at the end of the analysis, there are remaining CLI arguments that were not consumed by any explicit token (`<params::0>`, `<params::flag>`, etc.) and the script does not include the generic `<params>` token, `axes` will throw an error to prevent unexpected behavior.

---

## 1. `axes` Pre-Parsing

Before your tokens come into play, `axes` performs a simple pre-parsing of the arguments you pass in the terminal. It classifies them into two types:

* **Named Arguments (Flags):** Any token that starts with a hyphen (`-` or `--`), such as `--target` or `-v`. `axes` detects whether a flag is followed by a value (e.g., `--target linux`) or if it is a boolean flag without a value (e.g., `--force`).
* **Positional Arguments:** All other tokens. They are identified by their position (0, 1, 2, ...).

With these arguments classified, your script definitions can begin to work.

---

## 2. Positional Parameters

Positional arguments are accessed by their numerical index, starting at `0`.

### Syntax and Modifiers `(...)`

You can add a configuration block in parentheses to refine the behavior of a parameter.

* **Basic Syntax:** `<params::0>`, `<params::1>`, etc.
* **Modifiers:**
  * `required`: Execution fails if the argument is not provided.
  * `default='value'`: Provides a default value if the argument is not passed in the CLI.
  * `map='--new-flag '`: Transforms the positional argument into a flag with a value. If the user types `command my-value`, and the token is `<params::0(map='--target ')>`, the injected result will be `"--target my-value"`.
  * `literal`: Wrap entire final value into literal, `... "this is a positional value" ...`.

#### **Examples (Positional)**

**Script to greet (with `default`):**

```toml
# axes.toml
[scripts]
greet = "echo 'Hello, <params::0(default='World')>!'"
```

```sh
axes . greet          # -> echo 'Hello, World!'
axes . greet Valeria  # -> echo 'Hello, Valeria!'
```

**Script to create a file (with `required`):**

```toml
# axes.toml
[scripts]
create_file = "touch <params::0(required)>"
```

```sh
axes . create_file src/index.js  # -> touch src/index.js
axes . create_file               # -> Error: Positional argument at index 0 is required but was not provided.
```

**`lint` script (with `map`):**
This pattern is extremely useful for creating more readable interfaces.

```toml
# axes.toml
[scripts]
# Lints a path, converting the positional argument into a --path flag.
lint = "eslint <params::0(map='--path ', default='src/')>"
```

```sh
# Execution 1: Uses the default value
axes . lint
# Command executed: `eslint --path src/`

# Execution 2: Specifies a path
axes . lint tests/
# Command executed: `eslint --path tests/`
```

---

## 3. Named Parameters (Flags)

Parameter tokens can also search for and consume flags (`--name`) from the command line.

### Syntax and Default Behavior

* **Basic Syntax:** `<params::flag-name>`
* **Behavior (Pass-through):** By default, a flag token looks for the corresponding flag in the CLI and reinjects it as is, along with its value if it has one.
  * If run with `--flag-name value`, the token expands to `"--flag-name value"`.
  * If run with `--flag-name` (no value), it expands to `"--flag-name"`.
  * If the flag is not provided, the token expands to an empty string.

### Modifiers for Flags `(...)`

* `required`: Fails if the flag (or its alias) is not present.
* `default='value'`: If the flag is provided **without a value**, this `default` will be used. It is also used if the flag is **not provided at all**.
* `alias='-a'`: Allows the flag to be recognized by a short alias. `axes` will throw an error if the user attempts to use both (`--flag-name` and `-a`) at the same time.

* `map='--new-name'`: Replaces the flag name in the output.

* `map=''` (an empty string): A powerful special case. Indicates that you only want to inject the **value** of the flag, not the flag name itself.

* `literal`: Wrap entire final value into literal, `--flag "this is a flag value"`.

#### **Examples (Named)**

**`build` script with `release` mode (Simple Pass-through):**

```toml
# axes.toml
[scripts]
build = "cargo build <params::release>"
```

```sh
axes . build            # -> cargo build
axes . build --release  # -> cargo build --release
axes . build --another-param  # -> Error: Unexpected arguments were provided. The script does not define a generic `<params>` token to accept them.
#Provided unhandled arguments: --another-param
```

**`test` script with alias:**

```toml
# axes.toml
[scripts]
test = "pytest <params::marker(alias='-m')>"
```

```sh
axes . test --marker slow   # -> pytest --marker slow
axes . test -m smoke        # -> pytest --marker smoke
axes . test -m smoke --marker slow # -> Error: Conflict: Both flag '--marker' and its alias '-m' were provided.
```

**Another possible use cases:**

**`copy-file` script with multiple flags and defaults:**

```toml
# axes.toml
[scripts]
copy = "rsync <params::files-from(alias='-f', default='list.txt')> <params::destination(alias='-d', required)>"
```

```sh
# Uses the default for --files-from
axes copy -f --destination ./backup
# -> rsync --files-from list.txt --destination ./backup

# Overrides the default using the alias
axes copy -f file.txt -d ./backup
# -> rsync --files-from file.txt --destination ./backup

# Fails if the required destination is missing
axes copy
# -> Error: Flag '--destination' is required but was not provided.
```

**`deploy` script with `map` and `required`:**

```toml
# axes.toml
[scripts]
# The internal script expects --environment, but we want to expose --env to the user.
deploy = "terraform apply <params::env(map='--environment ', required)>"
```

```sh
axes . deploy --env staging      # -> terraform apply --environment staging
axes . deploy                    # -> Error: Flag '--env' is required but was not provided.
```

**`docker` script with `map=''` for value extraction:**
This is an advanced pattern for injecting values into places where a flag is not valid.

```toml
# axes.toml
[scripts]
# The image tag is passed as a flag but is injected as a positional value.
docker_tag = "docker tag my-image:latest my-org/my-image:<params::tag(map='', default='latest', required)>"
```

```sh
# Execution 1: Uses the default
axes docker_tag --tag
# Command executed: `docker tag my-image:latest my-org/my-image:latest`

# Execution 2: Specifies the tag
axes docker_tag --tag v1.2.0
# Command executed: `docker tag my-image:latest my-org/my-image:v1.2.0`
```

---

## 4. The Generic Collector: `<params>`

This is the "collector" token. It is useful when you want to pass a variable number of arguments or flags to an underlying command without having to explicitly define them all.

* **Syntax:** `<params>`
* **Behavior:** Replaced by **all arguments** (positional and named) that **were not consumed** by an explicit token (`::0`, `::flag`, etc.), maintaining their original order.

### **Example: A generic `wrapper` for `cargo run`**

```toml
# axes.toml
[scripts]
# Passes all undefined arguments directly to the binary.
run = "cargo run -- <params>"
# Allows an optional --release flag, and passes the rest.
run_release = "cargo run <params::release> -- <params>"
```

```sh
# Execution 1: Passing arguments to the binary (Use '/' because run is a system argument, that's equivalent to `axes ./run ...`)
axes /run --input /data/file.txt --verbose
# Command executed: `cargo run -- --input /data/file.txt --verbose`

# Execution 2: Using the script with release
axes run_release --input /data/file.txt --release
# `release` is consumed by <params::release> and expands to `--release`.
# `--input /data/file.txt` is consumed by <params> and expands to itself.
# Command executed: `cargo run --release -- --input /data/file.txt`
```

---

## 5. Advanced Use Case: Building a Composable CLI

`axes` is powerful enough to build complex command-line interfaces by composing variables and parameters. This example creates a flexible `git-log` script.

**Goal:** Create a `log` script that:

1. Defaults to a pretty, one-line format.
2. Accepts an optional `--author="<name>"` flag.
3. Accepts an optional `--count=<number>` flag to limit the number of commits.
4. Accepts a `--stat` flag to show file statistics.

**`axes.toml`:**

```toml
[vars]
# We define a base format that can be reused or modified.
_log_format = "--pretty=format:'%C(yellow)%h %C(cyan)%an %C(green)%s'"

[scripts]
# Our main `log` script composes all the pieces together.
log = "git log <vars::_log_format> <params::author(map='--author=')> <params::count(alias='-n', map='-n ')> <params::stat>"

This single line of `axes.toml` creates a remarkably powerful script:

**How it works:**

-   **`<vars::_log_format>`:** Injects the base format string.
-   **`<params::author(map='--author=')>`:**
    -   Looks for a `--author` flag in the CLI (e.g., `axes log --author="John Doe"`).
    -   The `map='--author='` ensures that if a value is provided, it's injected as `--author=John Doe`. The `=` is crucial here. If the flag is not provided, this token expands to an empty string.
-   **`<params::count(alias='-n', map='-n ')>`:**
    -   Looks for `--count` or `-n` (e.g., `axes log -n 5`).
    -   The `map='-n '` ensures it's injected as `-n 5` (with a space).
-   **`<params::stat>`:**
    -   This is a simple "pass-through" flag. If `axes log --stat` is run, this token expands to `--stat`. Otherwise, it's an empty string.

**Example Executions:**

```sh
# Simple log with the default pretty format
$ axes log
# -> git log --pretty=format:'%C(yellow)%h %C(cyan)%an %C(green)%s'

# Limit to 5 commits and show stats
$ axes log -n 5 --stat
# -> git log --pretty=format:'%C(yellow)%h %C(cyan)%an %C(green)%s' -n 5 --stat

# Filter by author
$ axes log --author="Jane Doe"
# -> git log --pretty=format:'%C(yellow)%h %C(cyan)%an %C(green)%s' --author="Jane Doe"
```

This demonstrates how `axes` can build a sophisticated and validated CLI wrapper around any existing tool with just a few lines of declarative configuration, completely eliminating the need for complex shell scripting.

➡️ **For a truly advanced, real-world example, see how `axes` uses its own engine to run its [benchmarking suite](./examples/stress_tests/.axes/axes.toml).**

---

By combining these patterns, you can build and/or modify command-line interfaces for your scripts that are as powerful, readable, and safe as those of any native tool, all from the simplicity of your `axes.toml`.
