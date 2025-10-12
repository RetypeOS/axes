<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./COMMAND.md">English</a> •
  <a href="./docs/es/COMMAND.md">Español</a>
</p>

# Complete Command Reference

This document is the definitive reference guide for every command available in the `axes` CLI. For a guided tutorial, refer to the [**Getting Started Guide (`GETTING_STARTED.md`)**](./GETTING_STARTED.md).

## Universal Grammar and Shortcuts

`axes` uses a simple and predictable universal grammar to interpret your commands. It follows a clear set of rules to determine the action you want to perform.

### **The Grammar Rules (in order of priority):**

1. **`axes <context> <action> [args...]`**
    * If the **second** argument is a known system action (`info`, `delete`, `start`, etc.), the first argument is treated as the project context.
    * *Example:* `axes my-app info`

2. **`axes <action> [args...]`**
    * If the **first** argument is a known system action, `axes` executes it. This is used for global commands (`init`, `tree`, `alias`) or when using an implicit context.
    * *Example (Global):* `axes tree --all`
    * *Example (Implicit Context):* `axes info` (equivalent to `axes . info`)

3. **`axes <virtual_script_path> [params...]` (Default)**
    * If neither of the above rules match, `axes` assumes you are trying to **run a script**. This is a shortcut for `axes <ctx> run <script_name>`.
    * *Example:* `axes build --release` => `run build script of current project` (`axes run build --release`)
    * *Example:* `axes api/build --release` => `run build script of child 'api' from actual project` (`axes api run build --release`)

### **Context System:**

* **Explicit Context:** `axes my-app/api ...`
* **Implicit Context (`.`):** For commands that require a context (`info`, `run`, `start`, etc.), if you don't provide one, `axes` automatically uses `.` (the project in the current directory or its first ancestor).
* **Navigation:** You can use `..` to refer to a parent project, `*` for the last-used child, and aliases like `g!` for the global project.

---

## Project Lifecycle Management

These commands are used to create, register, and delete projects from the `axes` index. They are **destructive** or **global** and follow the `axes <action> [args...]` or `axes <context> <action> [args...]` syntax.

### `init`

(Alias: `new`)

Initializes `axes` in the current directory, creating an `.axes/` structure and registering the project.

#### **Syntax**

```sh
axes init [options...]
```

#### **Arguments and Flags**

| Flag                   | Description                                                                              | Required |
| :--------------------- | :--------------------------------------------------------------------------------------- | :-------- |
| `--parent <context>`   | The context of the project that will be the parent of the new one. Defaults to `global`. | No        |
| `--name <name>`        | The name for the new project. If not provided, the directory name is used.               | No        |
| `--version <ver>`      | The initial version for the project (e.g., `1.0.0`).                                     | No        |
| `--description <desc>` | A brief description for the project.                                                     | No        |
| *...and others*        | `init` accepts more flags to pre-configure `[vars]` and `[env]`.                         | No        |

#### **Usage Examples**

```sh
# Initializes a project in the current directory, and starts the wizard asking for unindicated parameters.
cd my-project
axes init

# Initializes a project specifying its parent by context, and the rest of the parameters will be resolved automatically.
cd my-service
axes init --parent my-monorepo --autosolve

# Initializes a project with all details from the command line
axes init --name my-api --parent .. --version "1.0-beta" --description "The main API."
```

---

### `register`

(Alias: `reg`)

Registers a directory that **already contains** an `.axes/` configuration in the global `axes` index. It is useful for incorporating projects existing or repairing a broken registration.

#### **Syntax**

```sh
axes register [<path>] [--autosolve]
```

#### **Arguments and Flags**

| Argument/Flag      | Description                                                                                         | Required |
| :------------------ | :-------------------------------------------------------------------------------------------------- | :-------- |
| `<path>`            | The path to the project to be registered. If omitted, the current directory is used.                | No        |
| `--autosolve`       | Non-interactive mode. The operation will fail if it encounters any conflict (e.g., an existing UUID).| No        |

#### **Usage Examples**

```sh
# Registers the project in the current directory interactivel
axes register

# Registers a project located at another path
axes register ../another-project-with-axes
```

---

### `unregister`

(Alias: `unreg`)

Removes one or more projects from the `axes` index. **This action does NOT delete any files**, it just makes `axes` "forget" the projects.

#### **Syntax**

```sh
axes <context> unregister [--recursive] [--reparent-to <new_parent>]
```

#### **Default Behavior**

By default, `unregister` is **not recursive**. It only unregisters the project specified in `<context>`, and its direct children are re-parented to the root project (usually `global`) to avoid breaking the graph.

#### **Arguments and Flags**

| Flag                     | Description                                                                                               | Required |
| :----------------------- | :-------------------------------------------------------------------------------------------------------- | :-------- |
| `--recursive`            | Recursive mode. Unregisters the project specified AND **all its descendants**. No re-parenting occurs.      | No        |
| `--reparent-to <parent>` | Instead of moving children to the root, moves them to the specified `<new_parent>`. Not compatible with `--recursive`. | No        |

#### **Usage Examples**

```sh
# Unregisters `legacy-service`, its children will now be children of `global`.
axes my-app/legacy-service unregister

# Unregisters `prototype` and all its sub-projects.
axes prototype unregister --recursive

# Unregisters the `frontend-v1` "container", moving its children to `frontend-v2`.
axes frontend-v1 unregister --reparent-to frontend-v2
```

---

### `delete`

(Alias: `del`)

☢️ **DESTRUCTIVE ACTION.** Deletes the project's `.axes/` directory (and optionally its children's) AND unregisters it from the index.

#### **Syntax**

```sh
axes <context> delete [--recursive]
```

#### **Default Behavior**

Like `unregister`, `delete` is **not recursive by default** to prevent accidental data loss. It only deletes the `.axes/` of the specified project, and its children are re-parented to the root project.

#### **Arguments and Flags**

| Flag          | Description                                                                                  | Required |
| :------------ | :------------------------------------------------------------------------------------------- | :-------- |
| `--recursive` | Recursive mode. Deletes the `.axes/` of the specified project AND **all its descendants**.   | No        |

#### **Usage Examples**

```sh
# Deletes the identity of `old-service`, preserving its children.
axes old-service delete

# Completely removes the `experiment` project and everything it contains from the `axes` ecosystem.
axes experiment delete --recursive
```

## Inspection and Navigation

These commands help you visualize and understand the structure of your project tree and the configuration of each one. They are read-only and completely safe operations.

### `tree`

(Alias: `ls`)

Shows a visual representation of the registered project tree, starting from the root or a specific project.

#### **Syntax**

```sh
axes [<context>] tree [-p, --paths] [-u, --uuids] [--all]
```

#### **Behavior**

* If `<context>` is provided, it shows the subtree from that project.
* If omitted, it shows the entire tree from the project in the current directory (`.`). To see the full tree, use `axes global tree` or `axes g! tree`.

#### **Arguments and Flags**

| Argument/Flag      | Description                                                                 | Required  |
| :------------------ | :------------------------------------------------------------------------- | :-------- |
| `<context>`         | The project from which to start showing the tree.                          | No        |
| `-p`, `--paths`     | Shows the absolute physical path of each project.                          | No        |
| `-u`, `--uuids`     | Shows the unique UUID of each project.                                     | No        |
| `--all`             | A shortcut to show all available information (`--paths` and `--uuids`).    | No        |
| `-d`, `--depth <DEPTH>` | Limit the depth of the tree display                                    | No        |
| `--check`           | Check if the project paths exist on the filesystem                         | No        |

#### **Usage Examples**

```sh
# Shows the complete project tree
axes tree

# Shows the sub-tree of the `my-app` monorepo
axes my-app tree

# Shows the complete tree with paths and UUIDs, useful for debugging
axes tree --all

# Shows the tree of the current project's parent
axes .. tree -p
```

---

### `info`

Shows a complete summary of a project's **final and merged** configuration, including metadata, inherited scripts, and variables.

#### **Syntax**

```sh
axes [<context>] info
```

#### **Arguments and Flags**

| Argument    | Description                                   | Required  |
| :---------- | :-------------------------------------------- | :-------- |
| `<context>` | The project whose information to display.     | No        |

#### **Usage Examples**

```sh
# Shows the root project information
axes global info

# Shows the complete configuration of the API service, including
# the variables and scripts it has inherited from `my-app`.
axes my-app/api info
```

The `info` output is your best tool for debugging why a script behaves a certain way or where a specific variable comes from.

---

### `alias`

Manages shortcuts (aliases) for your project context paths. Aliases are global and allow you to quickly access deeply nested projects.

#### **Syntax**

```sh
axes [<ctx>] alias <subcommand> [arguments...]
```

#### **`alias` Subcommands**

| Argument    | Description                                      | Require                    |
| :---------- | :----------------------------------------------- | :------------------------- |
| `set`       | Sets a new alias or updates an existing one.     | `<alias> <target_context>` |
| `list`      | Lists all defined aliases.                       | Nothing                    |
| `remove`    | Removes an alias.                                | `<alias>`                  |
| `check`     | Verifies all aliases, reporting any broken links | Nothing                    |

**`list`**
(Alias: `ls`)
Shows a table of all defined aliases. This is the default subcommand if none is specified.

* **Syntax:** `axes alias [list]`

**`set`**
Creates a new alias or updates an existing one.

* **Syntax:** `axes alias set <alias_name> <target_context>`
* **Arguments:**
  * `<alias_name>`: The name of the shortcut (e.g., `api`, `frontend`). Do not include the `!`.
  * `<target_context>`: The full path of the project the alias will point to.

**`remove`**
(Alias: `rm`)
Deletes an alias.

* **Syntax:** `axes alias rm <alias_name>`
* **Arguments:**
  * `<alias_name>`: The name of the shortcut to delete.

#### **Important Notes**

* Aliases are used by appending an `!` at the end. For example, if you create `axes alias set api my-monorepo/services/main-api`, you can use it with `axes api! info`.
* The alias `g!` is a special default alias that always points to the root project. It can be modified or deleted, but a warning will be displayed.

#### **Usage Examples**

```sh
# List all aliases
axes alias

# Create a shortcut for a nested service
axes alias set api my-monorepo/services/api-v2

# Use the new alias
axes api! test

# Remove an alias
axes alias rm api
```

## Project Interaction and Execution

These are the main commands you will use in your daily workflow to run tasks, start environments, and open your projects.

### `run`

Executes a script defined in the `[scripts]` section of a project's `axes.toml`. This is the most powerful and versatile command in `axes`.

#### **Syntax**

```sh
axes [<context>] run <script_name> [parameters...]
# Or:
axes [<context>]/<script_name> [parameters...]
```

#### **Arguments and Flags**

| Argument          | Description                                                                  | Required  |
| :---------------- | :--------------------------------------------------------------------------- | :-------- |
| `<context>`       | The project context in which the script will be executed. (Implicitly `.` if omitted). | No       |
| `<script_name>`   | The name of the script to execute (the key under the `[scripts]` table).     | Yes       |
| `[parameters...]` | Any additional arguments that will be passed to the script.                  | No        |

#### **Key Functionality**

The `run` command is orchestrated by a powerful text expansion engine. Inside your scripts, you can use a special `<...>` syntax to:

* **Include variables:** `<vars::my_variable>`
* **Compose other scripts:** `<scripts::other_script>`
* **Execute commands and substitute their output:** `<run('git rev-parse --short HEAD')>`
* **Pass CLI parameters in a structured way:** `<params::0>`, `<params::fl(map='--flag', default='some', required)>`, `<params>`

> **Note:** The scripting and parameter system is the deepest feature of `axes`. For a complete guide with examples, see **[Mastering `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md)**.

#### **Usage Examples**

```sh
# Run 'build' script in current project (using shortcut)
axes build

# Executes the 'build' script in the `my-app/frontend` project
axes my-app/frontend run build

# Executes the 'test' script and passes a parameter
# (assuming `test` uses `<params>` or `<params::0>`)
axes my-app/api/test tests/unit/test_auth.py

# Executes a script passing a flag
# (assuming `deploy` uses `<params::production>`)
axes my-app/deploy --production
```

---

### `start`

Starts an interactive and persistent shell session within a project's context. It is the ideal tool for focused work.

#### **Syntax**

```sh
axes [<context>] start [parameters...]
```

#### **Arguments and Flags**

| Argument          | Description                                                                         | Required |
| :---------------- | :---------------------------------------------------------------------------------- | :-------- |
| `<context>`       | The project in which to start the session. (Implicitly `.` if omitted).             | No       |
| `[parameters...]` | Any additional arguments that will be passed to the `at_start` and `at_exit` hooks. | No       |

#### **Session Behavior**

When running `start`, `axes` does the following:

1. **Resolves and Validates Parameters:** `axes` analyzes the provided `[parameters...]` and validates them against the `<params::...>` definitions found in the `at_start` and `at_exit` hooks.
2. **Executes the `at_start` Hook:** The `at_start` script is executed, injecting the resolved parameters.
3. **Starts the Shell:** The interactive shell is launched with all `[env]` variables injected.

Once inside, you can execute `axes` commands without specifying the context. When exiting the session with `exit`, the `at_exit` hook is executed, which also receives the same `[parameters...]` resolved at the start of the session.

#### **Usage Examples**

```sh
# Starts a simple session in the API service
axes my-app/api start

# Assuming an `at_start` like: "docker-compose up -d <params::service>"
# Starts a session and specifies which service to spin up
axes my-app/api start --service web
```

---

### `open`

Opens a project's root directory using a pre-configured application.

#### **Syntax**

```sh
axes [<context>] open [<app_key>] [parameters...]
```

#### **Arguments and Flags**

| Argument         | Description                                                                          | Required |
| :---------------- | :----------------------------------------------------------------------------------- | :-------- |
| `<context>`       | The project to be opened. (Implicitly `.` if omitted).                               | No       |
| `[<app_key>]`     | The key of the application to use (e.g., `code`). If omitted, the `default` key is used. | No       |
| `[parameters...]` | Any additional arguments that will be passed to the `app_key` script. | No       |

#### **Configuration**

Applications are defined in the `[options.open_with]` section of your `axes.toml`. Since v0.1.8, each entry is a **complete script** that can be a string, a sequence, or a table with a description. [WARN: Pending to Change...]

```toml
[options.open_with]
# Simple string shortcut
edit = "code \"<path>\""

# Shortcut with description and that accepts parameters
terminal = { desc = "Opens a terminal in a subfolder.", run = "wt -d \"<path>/<params::0(default='.')>\"" }

# Sets the default action
default = "edit"
```

#### **Usage Examples**

```sh
# Opens the current project with the default application (implicitly axes . open)
axes open

# Explicitly opens the `my-app/api` project in the file explorer
# (Assuming a 'files' key is defined)
axes my-app/api open files

# Uses the 'terminal' shortcut and passes a parameter to open in the 'src' subdirectory
axes my-app/frontend open terminal src
```

## Project Tree Refactoring

These commands allow you to modify the structure of your `axes` ecosystem, changing the relationships between projects and their names. These are powerful operations that update the global `axes` index.

### `link`

Changes the parent of an existing project, moving it to a new location in the logical tree. This operation is purely structural and does not move any files on your disk.

#### **Syntax**

```sh
axes <child_context> link <new_parent_context>
```

#### **Arguments and Flags**

| Argument                   | Description                                       | Required  |
| :------------------------- | :------------------------------------------------ | :-------- |
| `<child_context>`          | The project you want to move.                     | Yes       |
| `<new_parent_context>`     | The project that will become its new parent.      | Yes       |

#### **Safety Validations**

`link` is a safe operation. `axes` will prevent any action that could corrupt the project graph, failing with a clear error if you attempt to:

* **Create a cycle:** Moving a project to become its own descendant (e.g., `axes A link A/B`).
* **Create a name collision:** Moving a project to a new parent that already has a child with the same name.

#### **Usage Examples**

```sh
# `legacy-service` was a child of `global`, now it will be a child of `monorepo-v2`.
axes legacy-service link monorepo-v2

# Moves the `admin-panel` to be a child of the `api` service instead of `frontend`.
axes my-app/frontend/admin-panel link my-app/api
```

---

### `rename`

Changes the name of a project. This is the name used in context paths, not the name of the directory on disk.

#### **Syntax**

```sh
axes <context> rename <new_name>
```

#### **Arguments and Flags**

| Argument          | Description                                   | Required  |
| :---------------- | :-------------------------------------------- | :-------- |
| `<context>`       | The project you want to rename.               | Yes       |
| `<new_name>`      | The new name for the project.                 | Yes       |

#### **Naming Rules**

The `<new_name>` must follow certain rules to ensure stability:

* **Cannot contain spaces** or path characters (`/`, `\`).
* **Cannot be a reserved name** for navigation (e.g., `.` , `..`, `*`).

`axes` will also warn you if you attempt to use names that, while valid, are not recommended (e.g., starting with `-`). Renaming the root project `global` is allowed but will require an additional confirmation due to its importance.

#### **Usage Examples**

```sh
# Renames a project from `api-v1` to `api-legacy`.
axes my-app/api-v1 rename api-legacy

# The new context to access it will now be `my-app/api-legacy`.
axes my-app/api-legacy info
```

---
This document provides a complete reference of the available commands. To learn how to write powerful workflows, the next recommended reading is the **[`axes.toml` Guide](./AXES_TOML_GUIDE.md)**.
