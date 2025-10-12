# generar_axes_toml.py
# Genera un archivo axes.toml con 1000 scripts adicionales de ejemplo

BASE_HEADER = """# axes.toml for the 'axes' project
# This file defines the complete development, testing, and release workflow for `axes` itself.
# It is designed to be a best-practice example of a clean and maintainable configuration.

# === Project Metadata ===
version = "0.2.3-beta"
description = "The development workflow configuration for the `axes` project itself."

# === Development Scripts ===
[scripts]
hello_2 = "# Hello, <params::0(default='World')>"
test_rec = "echo <scripts::test_rec2>"
test_rec2 = "bbbbbb<scripts::test_rec>"

# --- Core Development Workflows ---
build = { desc = "Builds the project in debug mode with fast compilation.", run = "cargo build <params>" }
build_release = { desc = "Builds the project in release mode for distribution.", run = "cargo build --release" }
run = { desc = "Runs the project, passing all arguments to the binary.", run = "cargo run -- <params>" }
test = { desc = "Runs all unit and integration tests.", run = "cargo test" }

# --- Code Quality ---
check = { desc = "Checks the project for errors without compiling.", run = "cargo check" }
lint = { desc = "Lints the code for style and correctness issues.", run = "cargo clippy -- -D warnings <params>" }
fmt = { desc = "Checks if the code is formatted according to project style.", run = "cargo fmt --all -- --check <params>" }
fmt_fix = { desc = "Formats the code automatically.", run = "cargo fmt --all <params>" }

quality = { desc = "Runs all quality checks in sequence (fmt, check, lint, test).", run = [
    "<scripts::fmt>",
    "<scripts::check>",
    "<scripts::lint>",
    "<scripts::test>",
]}

_flamgraph_exec = { windows = "start flamegraph.svg", macos = "open flamegraph.svg", linux = "xdg-open flamegraph.svg", default="echo 'Flamegraph generated at flamegraph.svg'" }

# --- Artifacts & Distribution ---
clean = { desc = "Removes the target directory and build artifacts.", run = "cargo clean <params>" }
doc = { desc = "Builds and opens the project documentation in the browser.", run = "cargo doc --open <params>" }


"""

VARS_ENV = """

[scripts.install]
desc = "Builds in release and copies the artifact to a local installation path."
windows = [
    "# <vars::install_welcome>",
    "<scripts::build_release>",
    "powershell -Command \\"New-Item -ItemType File -Path '<vars::install_path_win>' -Force | Out-Null; Copy-Item -Path '.\\\\target\\\\release\\\\axes.exe' -Destination '<vars::install_path_win>'\\"",
    "# Completed.!"
]
linux = [
    "<scripts::build_release>",
    "install -m 755 -D \\"<path>/target/release/axes\\" \\"<vars::install_path_nix>\\""
]
macos = [
    "<scripts::build_release>",
    "install -m 755 -D \\"<path>/target/release/axes\\" \\"<vars::install_path_nix>\\""
]

[scripts.flamegraph]
desc = "Generates a performance flamegraph for a given command and opens the result."
run = [
    "cargo flamegraph --bin axes -- <params>",
    "<scripts::_flamgraph_exec>"
]

[scripts.Hyperfine]
desc = ""
run = "hyperfine 'axes hello' 'just hola' 'task hello' --warmup 5 --runs 50 --export-markdown benchmark_results.md"

# === Interpolation Variables ===
[vars]
install_welcome = "Installing 'axes' on: '<vars::install_path_win>'..."
install_path_win = "<params::0(default='.\\\\bin\\\\axes.exe')>"
install_path_nix = "<params::0(default='./bin/axes')>"

# === Environment Variables ===
[env]
RUST_LOG = "debug"
RUST_BACKTRACE = "1"
CARGO_PROFILE_RELEASE_DEBUG = "true"
"""

def generar_axes_toml(n=1000, archivo="axes.toml"):
    with open(archivo, "w", encoding="utf-8") as f:
        # Escribir cabecera base
        f.write(BASE_HEADER)
        f.write("\n\n")
        # Generar scripts adicionales
        for i in range(1, n+1):
            f.write(f"script_{i:04d} = \"echo Script número {i:04d} ejecutado\"\n")
        f.write("\n")
        # Escribir sección de variables y entorno
        f.write(VARS_ENV)
    print(f"Archivo '{archivo}' generado con {n} scripts adicionales.")

if __name__ == "__main__":
    generar_axes_toml(100000)