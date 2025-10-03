<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./GETTING_STARTED.md">English</a> •
  <a href="./docs/es/GETTING_STARTED.md">Español</a>
</p>

# Getting Started Guide: Your First Orchestrated Monorepo with `axes`

Welcome to `axes`! This guide will take you from zero to a fully functional and orchestrated monorepo. In the next 15-20 minutes, you will learn how to:

* ✅ Install `axes` on your system.
* ✅ Create your first project and sub-projects.
* ✅ Define and execute scripts using the new universal grammar.
* ✅ Leverage variable inheritance between projects.
* ✅ Orchestrate a complex workflow involving multiple projects.
* ✅ Use project sessions for a focused workflow.

By the end of this tutorial, you will understand the fundamental power of `axes` and be ready to apply it to your own projects.

---

## 1. Installation

`axes` is a single binary file with no external dependencies, making its installation very simple.

### Option A: Download the Pre-compiled Binary (Recommended)

This is the fastest way to get started.

1. **Go to the Releases Page:** Open the [official `axes` Releases page on GitHub](https://github.com/RetypeOS/axes/releases).
2. **Download the correct file:** Find the latest version and download the `.zip` or `.tar.gz` file that corresponds to your operating system (Windows, macOS, or Linux).
3. **Unzip the file:** Inside, you will find a single executable: `axes.exe` (on Windows) or `axes` (on macOS/Linux).
4. **Move the executable to your `PATH`:** This is the most important step. To be able to call `axes` from anywhere in your terminal, you must move this file to a directory that is in your system's `PATH` environment variable.

    * **Windows:**
        1. Create a folder, for example, `C:\Program Files\axes`.
        2. Move `axes.exe` to that folder.
        3. Search for "Edit the system environment variables" in the start menu, open the `PATH` editor, and add the path `C:\Program Files\axes` to the list.
    * **macOS / Linux:**
        A common and recommended directory is `/usr/local/bin`. You can move the file with this command in your terminal (you might need `sudo`):

        ```sh
        sudo mv ./axes /usr/local/bin/axes
        ```

5. **Verify the installation:** Open a **new** terminal window (this is important for the `PATH` changes to load) and run:

    ```sh
    axes --version
    ```

    If you see a version number, the installation was successful!

### Option B: Compile from Source Code

If you have the [Rust toolchain](https://www.rust-lang.org/tools/install) installed, you can compile `axes` yourself.

```sh
# 1. Clone the repository
git clone https://github.com/RetypeOS/axes.git

# 2. Navigate to the directory
cd axes

# 3. Compile in release mode (optimized)
cargo build --release
```

The final executable will be located in `./target/release/axes`. You can move this file to your `PATH` as described in Option A.

---

With `axes` installed, you are ready to create your first project. Let's go!

## 2. Our Scenario and Context Navigation

For this tutorial, we will build the structure of a fictional corporate website called "Innovatech." This site will have two main components: a **blog** and an **online store**.

Before starting, it is crucial to understand how `axes` refers to projects. Just as you navigate a file system with `cd`, `axes` navigates its logical project tree using **contexts**. These are used to tell commands like `info`, `tree`, or `start` which project to operate on.

| Context | Description                                                                 | Example (from `.../innovatech-website/blog`) |
| :------ | :-------------------------------------------------------------------------- | :----------------------------------- |
| `name`  | A direct child of the root project (default name is `global`).              | `axes innovatech-website info`       |
| `/`     | The level separator in the hierarchy.                                       | `axes innovatech-website/blog info`  |
| `.`     | The project in the current directory, or the first ancestor found.          | `axes . info` (resolves to `innovatech-website/blog`)    |
| `_`     | The project whose root directory is *exactly* the current directory.        | `axes _ info` (resolves to `innovatech-website/blog`)    |
| `..`    | The parent of the current context project or the first ancestor found.      | `axes .. info` (resolves to `innovatech-website`)  |
| `**`    | (Double asterisk) Resolves to the last project you used in the **entire system.** Useful for quickly returning. | `axes ** start`    |
| `*`     | (Single asterisk) Resolves to the last child you used **of the current parent project**. | `axes mi-super-app/* start`    |
| `alias!`| A custom shortcut you create.                                               | `axes blog! info` (if `blog!` points to our project)  |

Throughout this tutorial, we will use these contexts so you can see how fluid and powerful they are.

### Creating the Container Project

First, create a directory for the entire monorepo and, within it, initialize your root `axes` project.

```sh
mkdir innovatech-website && cd innovatech-website
axes init
```

Accept the default values in the interactive wizard (name: `innovatech-website`, parent: `global`, etc.). Now, customize the generated `axes.toml` to be the base of our monorepo:

```toml
# ./innovatech-website/.axes/axes.toml
version = "1.0.0"
description = "The monorepo for the Innovatech website."

[vars]
company_name = "Innovatech Inc."

[scripts]
check_copyright = "echo \"© $(date +%Y) <axes::vars::company_name>. All rights reserved.\""
```

We have defined a variable and a script that will act as shared configuration for our entire monorepo.

---

## 3. The First Sub-Project: The Blog

Now, let's create the blog as a **child** of `innovatech-website`.

```sh
# Inside innovatech-website/, create and enter the blog directory
mkdir blog && cd blog

# Initialize `axes`, using `..` to refer to the parent (`innovatech-website`)
axes init --parent ..
```

In the wizard, `axes` will interpret `..` as the project in the parent or upper directory and suggest it to you. You are already using context navigation!

To visualize the new structure, go back to the parent directory and run `tree`:

```sh
# From the innovatech-website/ directory
cd ..
axes innovatech-website tree

# Or, more intelligently, from inside `blog/`:
# "Show me my parent's tree"
axes .. tree
```

Both will show:

```text
innovatech-website
└─ blog
```

### Demonstrating Inheritance

Now, open the `axes.toml` inside `blog/` and define a script that uses the inherited configuration:

```toml
# ./innovatech-website/blog/.axes/axes.toml
version = "0.1.0"
description = "The Innovatech blog."

[scripts]
build = "hugo --minify"
# This script COMPOSES the 'check_copyright' script inherited from the parent.
generate_footer = [
    "echo '--- Generating Blog Footer ---'",
    "<axes::scripts::check_copyright>",
    "echo 'Built with <axes::name>'"
]
```

To execute a script in your current project, simply use its name.

```sh
# While in the blog/ directory
axes generate_footer
```

The output will be:

```text
> --- Generating Blog Footer ---
> © 2024 Innovatech Inc.. All rights reserved.
> Built with innovatech-website/blog
```

You have shared configuration and logic cleanly and navigated your project intuitively. Next, we will add more complexity with our online store.

## 4. The Second Sub-Project: The Online Store

Our online store will be the third project in our tree. The process is identical to the blog's.

```sh
# From the root directory (innovatech-website/)
mkdir store && cd store

# Initialize, again specifying the parent with `..`
axes init --parent ..
```

After the wizard, your project tree (`axes innovatech-website tree`) will look like this:

```text
innovatech-website
├─ blog
└─ store
```

Now, let's give the store a more advanced script. Edit the new `axes.toml` in `store/`:

```toml
# ./innovatech-website/store/.axes/axes.toml
version = "1.0.0"
description = "The Innovatech online store."

[scripts]
# This test script accepts a positional parameter.
# `<axes::params::0>` will be replaced by the first argument
# we pass to the script from the command line.
test_module = "pytest tests/test_<axes::params::0>.py"
```

To run a script on a **different** project, you must now use the `run` command explicitly. This removes ambiguity and makes your intent clear.

```sh
# From the innovatech-website/ directory
axes store run test_module payments  # --> will execute `pytest tests/test_payments.py`
axes store run test_module products  # --> will execute `pytest tests/test_products.py`
```

You have created a reusable and parameterizable shortcut, eliminating the need to remember or type long, complex test paths.

> **Go Deeper:** The `axes` parameter system is extremely powerful, allowing flags, default values, and more. To master it, consult our complete guide: **[Mastering `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md)**.

---

## 5. Master Orchestration

We have created individual projects, each with its own scripts. Now, let's bring them together. The true power of `axes` lies in its ability to act as the conductor for your entire ecosystem.

Let's go back to the `axes.toml` of the parent project, `innovatech-website`, to create workflows that control the children.

```toml
# ./innovatech-website/.axes/axes.toml

version = "1.0.0"
description = "The monorepo for the Innovatech website."

[vars]
company_name = "Innovatech Inc."

[scripts]
check_copyright = "echo \"© $(date +%Y) <axes::vars::company_name>. All rights reserved.\""

# NEW! A script that calls the scripts of its children.
build_all = [
    "echo '🚀 Building the entire website...'",
    # The `>` prefix indicates that the command must be executed in PARALLEL.
    # We use the explicit `run` command for clarity and robustness.
    "> axes blog run build",
    "> axes store run build" # Assuming `store` also has a `build` script.
]

# A quality script that runs in sequence.
quality_check = [
    "echo ' linting...'",
    "axes blog run lint",  # Assuming `lint` scripts in the children.
    "axes store run lint",
    "echo '✅ Code quality verified!'"
]
```

With this configuration, you have created single entry points for complex operations across the entire monorepo:

```sh
# From anywhere in your system.
# Builds the blog and the store simultaneously.
axes innovatech-website run build_all

# Runs the linters one after the other.
axes innovatech-website run quality_check
```

And if you only want to execute individually, you just need to call its function:

```sh
# Execute the script only for the blog project.
axes innovatech-website/blog run build

axes */store run build # if you already executed the previous command, '*' indicates that the most recently used project from the parent is returned.
```

You have moved from managing individual commands to orchestrating entire workflows. The complexity of each sub-project is encapsulated, and the parent project provides a simple and powerful API to interact with the whole.

## 6. Immersive Workflow: Session Mode (`start`)

Composing and orchestrating scripts is incredibly powerful. But sometimes, you just want to focus on a single part of your system, like the blog.

For this, `axes` offers **project sessions**.

To enter the `blog` project's context:

```sh
# `start` is the default action if you only provide a context.
# This command is a shortcut for `axes innovatech-website/blog start`
$ axes innovatech-website/blog

--- `axes` session for 'innovatech-website/blog' started. Type 'exit' to leave. ---
# Your terminal prompt might change to reflect the active session.
```

You are now "inside" the `blog` project. `axes` has done two things for you:

1. **Hook Activation:** It has executed the script defined in `[options].at_start` of your `axes.toml`. This is perfect for activating virtual environments or starting necessary services.
2. **Implicit Context:** You no longer need to specify the context. `axes` knows where you are.

Inside the session, your workflow becomes incredibly simple and uses the same universal grammar:

```sh
# The context is now implicit.
(axes: innovatech-website/blog) $ axes build
(axes: innovatech-website/blog) $ axes generate_footer

# ... after a productive working session ...
(axes: innovatech-website/blog) $ exit
```

Upon exiting, `axes` automatically executes the `at_exit` hook, ideal for stopping services (`docker-compose down`) and ensuring no orphaned processes remain.

`axes` sessions eliminate the last barrier of friction, allowing you to focus 100% on your code.

---

## You Have Completed the Tour! What's Next?

Congratulations! You have installed `axes`, built a monorepo from scratch, shared configuration through inheritance, composed complex workflows, and experienced the fluidity of project sessions.

You now have a solid foundation to start using `axes` in your own projects.

The journey doesn't end here. `axes` is a deep tool with many more features designed to make your life easier. To become an expert user, we recommend exploring the rest of our documentation:

* **[Complete Command Reference (`COMMANDS.md`)](./COMMANDS.md):** Want to know everything `init`, `tree`, `link`, or `delete` can do? This is your reference guide.
* **[Mastering `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** The definitive guide to the `axes.toml` syntax. Learn about cross-platform commands, the complete `<axes::params::...>` syntax, and more.
* **[Technical and Contribution Guide (`TECNICAL.md`)](./TECNICAL.md):** If you are curious about how `axes` works internally or want to contribute to the project, this is your starting point.

## Join the Community

`axes` is in **Beta phase** and thrives on feedback from users like you.

* **Found a Bug or Have an Idea:** [**Open an Issue**](https://github.com/RetypeOS/axes/issues)
* **Want to Contribute Code:** Pull Requests are welcome!

Thank you for taking the time to learn `axes`. We look forward to seeing the incredible workflows you will build!
