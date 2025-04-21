# vectordb-cli

A lightweight command-line tool for fast, local code search using semantic retrieval powered by ONNX models and Qdrant. Now with multi-repository and branch-aware indexing!

**Note:** This repository contains both the `vectordb-cli` command-line tool and the underlying `vectordb_lib` library.

## Table of Contents

-   [Features](#features)
-   [Use Cases](#use-cases)
-   [Supported Languages](#supported-languages)
-   [Setup](#setup)
    -   [Local Quickstart Guide](./docs/local_quickstart.md)
    -   [Prerequisites](#prerequisites)
    -   [Qdrant Setup](#qdrant-setup)
    -   [Environment Setup Guides](#environment-setup-guides)
-   [Installation](#installation)
    -   [Compilation Options](./docs/compile_options.md)
-   [Configuration](#configuration)
    -   [Environment Variables](#environment-variables)
    -   [Configuration File (`config.toml`)](#configuration-file-configtoml)
-   [Usage (CLI)](#usage-cli)
    -   [Global Options](#global-options)
    -   [Simple Commands (`simple`)](#simple-commands-simple)
        -   [`simple index`](#simple-index)
        -   [`simple query`](#simple-query)
        -   [`simple clear`](#simple-clear)
    -   [Repository Management (`repo`)](#repository-management-repo)
        -   [`repo add`](#repo-add)
        -   [`repo list`](#repo-list)
        -   [`repo use`](#repo-use)
        -   [`repo remove`](#repo-remove)
        -   [`repo use-branch`](#repo-use-branch)
        -   [`repo sync`](#repo-sync)
        -   [`repo clear`](#repo-clear)
        -   [`repo query`](#repo-query)
        -   [`repo stats`](#repo-stats)
        -   [`repo config`](#repo-config)
-   [Development](#development)

## Features

-   **Semantic Search:** Finds relevant code chunks based on meaning using ONNX models.
-   **Repository Management:** Manage configurations for multiple Git repositories.
-   **Branch-Aware Indexing:** Track and sync specific branches within repositories.
-   **Qdrant Backend:** Utilizes a Qdrant vector database instance for scalable storage and efficient search.
-   **Local or Remote Qdrant:** Can connect to a local Dockerized Qdrant or a remote instance.
-   **Simple Indexing (Default):** Recursively indexes specified directories (can be used alongside repository management).
-   **Configurable:** Supports custom ONNX embedding models/tokenizers and Qdrant connection details via config file or environment variables.
-   **Semantic Code Editing:** Powerful code editing capabilities that leverage its semantic understanding of code:
    - **Semantic element targeting** - Edit entire classes, functions, or methods using semantic identifiers
    - **Line-based precision edits** - Make targeted changes to specific sections of code
    - **Validation-first workflow** - Validate edits before applying them to ensure safety
    - **CLI interface** - Use from the command line

## Use Cases

-   **Debugging Assistance:** Use semantic search to find potentially related code sections when investigating bugs. Combine with LLMs by providing relevant code snippets found through queries for diagnosis, explanation, or generating flow charts.
-   **Code Exploration & Understanding:** Quickly locate definitions, implementations, or usages of functions, classes, or variables across large codebases or multiple repositories, even if you don't know the exact name.
-   **Finding Examples:** Locate examples of how a particular API, library function, or design pattern is used within your indexed code.
-   **Onboarding:** Help new team members find relevant code sections related to specific features or concepts they need to learn.
-   **Automated Code Editing:** Make precise semantic-aware edits to code without manual file editing:
    - Replace entire classes or functions using semantic targeting
    - Add methods to existing classes with line-based targeting
    - Validate edits before applying for safety and reliability
-   **Documentation Search:** Index and search through Markdown documentation alongside code (Note: Current Markdown parsing is basic but will be improved).
-   **Refactoring & Auditing:** Identify code locations potentially affected by refactoring or search for specific patterns related to security or best practices.

## Supported Languages

The CLI uses tree-sitter for Abstract Syntax Tree (AST) parsing to extract meaningful code chunks (like functions, classes, structs) for indexing. This leads to more contextually relevant search results compared to simple line-based splitting.
Here is the current status of language support:

| Language   | Status         | Supported Elements                                                                  |
| :--------- | :------------- | :---------------------------------------------------------------------------------- |
| Rust       | ✅ Supported | functions, structs, enums, impls, traits, mods, macros, use, extern crates, type aliases, unions, statics, consts |
| Ruby       | ✅ Supported | modules, classes, methods, singleton_methods                                        |
| Go         | ✅ Supported | functions, methods, types (struct/interface), consts, vars                        |
| Python     | ✅ Supported | functions, classes, top-level statements                                            |
| JavaScript | ✅ Supported | functions, classes, methods, assignments                                          |
| TypeScript | ✅ Supported | functions, classes, methods, interfaces, enums, types, assignments                |
| Markdown   | ✅ Supported | headings, code blocks, list items, paragraphs                                       |
| YAML       | ✅ Supported | documents                                                                           |
| Other      | ✅ Supported | Whole file chunk (fallback_chunk)                                                 |

Files with unsupported extensions will automatically use the whole-file fallback mechanism.

**Planned Languages:**

Support for the following languages is planned for future releases:

*   Java (`.java`)
*   C# (`.cs`)
*   C++ (`.cpp`, `.h`, `.hpp`)
*   C (`.c`, `.h`)
*   PHP (`.php`)
*   Swift (`.swift`)
*   Kotlin (`.kt`, `.kts`)
*   HTML (`.html`)
*   CSS (`.css`)
*   JSON (`.json`)

## Setup

For new users, the [Local Quickstart Guide](./docs/local_quickstart.md) provides minimal steps to get up and running quickly.

### Prerequisites

-   **Rust:** Required for building the project. Install from [rustup.rs](https://rustup.rs/).
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    # After installing rustup, source the Cargo environment script or restart your terminal
    source "$HOME/.cargo/env"
    ```
-   **Git:** Required for repository management features (`repo add`, `repo sync`, etc.).
-   **Build Tools:** Rust often requires a C linker and build tools.
    -   **Linux (Debian/Ubuntu):**
        ```bash
        sudo apt-get update && sudo apt-get install build-essential git-lfs libssl-dev pkg-config
        ```
    -   **macOS:** Install the Xcode Command Line Tools. If you don't have Xcode installed, running the following command in your terminal will prompt you to install them:
        ```bash
        xcode-select --install
        ```
        Install required packages using Homebrew:
        ```bash
        brew install git-lfs pkg-config
        ```
-   **Qdrant:** A Qdrant instance (v1.7.0 or later recommended) must be running and accessible. See [Qdrant Setup](#qdrant-setup).
-   **ONNX Model Files:** An ONNX embedding model and its corresponding tokenizer files are required. See [Installation](#installation) and [Configuration](#configuration).

### Qdrant Setup

`vectordb-cli` requires a running Qdrant instance. Each managed repository will have its own collection in Qdrant, named `repo_<repository_name>`.

**Option 1: Docker (Recommended for Local Use)**

```bash
docker run -p 6333:6333 -p 6334:6334 \
    -v $(pwd)/qdrant_storage:/qdrant/storage:z \
    qdrant/qdrant:latest
```

This starts Qdrant with the default HTTP/REST port (6333, used for the web UI at http://localhost:6333/dashboard) and gRPC port (6334, used by `vectordb-cli`) mapped to your host. Data will be persisted in the `qdrant_storage` directory in your current working directory.

**Option 2: Qdrant Cloud or Other Deployment**

Follow the instructions for your chosen deployment method. You will need the **URL** (including `http://` or `https://` and the port, typically 6333 for gRPC) and potentially an **API Key** if required by your setup.

### Environment Setup Guides

For specific environment configurations (GPU acceleration), refer to the guides in the `docs/` directory:

-   [docs/CUDA_SETUP.md](./docs/CUDA_SETUP.md) (Linux with NVIDIA GPU)
-   [docs/MACOS_GPU_SETUP.md](./docs/MACOS_GPU_SETUP.md) (macOS with Metal GPU)
-   [docs/CODEBERT_SETUP.md](./docs/CODEBERT_SETUP.md) (Using CodeBERT model - *may be outdated*)
-   [docs/compile_options.md](./docs/compile_options.md) (Compilation options and feature flags)

## Installation

1.  **Clone the Repository:**
    ```bash
    git clone https://gitlab.com/amulvany/vectordb-cli.git
    cd vectordb-cli
    ```

2.  **Prepare ONNX Model & Tokenizer:**
    Download or obtain your desired ONNX embedding model (`.onnx` file) and its tokenizer configuration (`tokenizer.json` and potentially other files like `vocab.txt`, `merges.txt`, etc., usually in a single directory). Place them in a known location. See [Configuration](#configuration) for how to tell the tool where these are.

    **Using the Example Model:** This repository includes an example `all-MiniLM-L6-v2` model in the `onnx/` directory, managed via Git LFS. If you followed the prerequisites and installed Git LFS, Git should handle pulling the model files automatically when you clone or pull updates. If the `.onnx` file in `onnx/model/` is small (a pointer file), you might need to run `git lfs pull` manually.

    **Note:** The tool dynamically detects the embedding dimension from the provided `.onnx` model.

3.  **Build:**
    *   **Standard (CPU):**
        ```bash
        cargo build --release
        ```
    *   **With CUDA GPU Support (Linux):** Ensure you have NVIDIA drivers, the CUDA toolkit, and `cudnn` installed (see [docs/CUDA_SETUP.md](./docs/CUDA_SETUP.md)). Then build with:
        ```bash
        cargo build --release --features ort/cuda
        ```
    *   **With Metal GPU Support (macOS):** (See [docs/MACOS_GPU_SETUP.md](./docs/MACOS_GPU_SETUP.md))
        ```bash
        cargo build --release --features ort/coreml # Or ort/metal if preferred/available
        ```

    **For a complete reference of all build options and feature flags, see [Compilation Options](./docs/compile_options.md).**

4.  **Understanding the Build Process (Linux/macOS):**
    *   The project uses a build script (`build.rs`) to simplify setup.
    *   During the build, this script automatically finds the necessary ONNX Runtime libraries (downloaded by the `ort` crate to `~/.cache/ort.pyke.io/`) including provider-specific libraries (like CUDA `.so` files or macOS `.dylib` files).
    *   It copies these libraries into the final build output directory (`target/release/lib/`).
    *   It sets the necessary RPATH (`$ORIGIN/lib` on Linux, `@executable_path/lib` on macOS) on the `vectordb-cli` executable.
    *   This means you typically **do not** need to manually set `LD_LIBRARY_PATH` (Linux) or `DYLD_LIBRARY_PATH` (macOS).

5.  **Install Binary (Optional):** Symlink the compiled binary to a location in your `PATH`.
    ```bash
    # Example for Linux/macOS to set it up globally
    sudo ln -s $PWD/target/release/vectordb-cli /usr/local/bin
    ```

## Configuration

`vectordb-cli` uses a hierarchical configuration system:

1.  **Command-line Arguments:** Highest priority (e.g., `--onnx-model-path-arg`, `--onnx-tokenizer-dir-arg`).
2.  **Environment Variables:** Second priority.
3.  **Configuration File (`config.toml`):** Lowest priority.

### Environment Variables

-   `QDRANT_URL`: URL of the Qdrant gRPC endpoint (e.g., `http://localhost:6334`). Defaults to `http://localhost:6334` if not set.
-   `QDRANT_API_KEY`: API key for Qdrant authentication (optional).
-   `VECTORDB_ONNX_MODEL`: Full path to the `.onnx` model file.
-   `VECTORDB_ONNX_TOKENIZER_DIR`: Full path to the directory containing the `tokenizer.json` file.

### Configuration File (`config.toml`)

The tool looks for a `config.toml` file in the XDG configuration directory:

*   **Linux/macOS:** `~/.config/vectordb-cli/config.toml`

**Example `config.toml`:**

```toml
# URL for the Qdrant gRPC endpoint
qdrant_url = "http://localhost:6334"

# --- Optional: Qdrant API Key ---
# api_key = "your_qdrant_api_key"

# --- Optional: ONNX Model Configuration ---
# These are only needed if not provided via args or env vars.

# Path to the ONNX model file
onnx_model_path = "/path/to/your/model.onnx"

# Path to the directory containing tokenizer.json
# Note: Key name is `onnx_tokenizer_path`
onnx_tokenizer_path = "/path/to/your/tokenizer_directory"

# --- Optional: Repository Storage Configuration ---
# Base path where all repositories will be stored
# If not provided, uses ~/.local/share/vectordb-cli/repositories
repositories_base_path = "/path/to/your/repositories"

# --- Repository Management ---
# The active repository (used by default for commands like sync, query)
# Set via `repo use <n>`
active_repository = "my-project"

# List of managed repositories
[[repositories]]
name = "my-project"
# Local path where the repository was cloned
local_path = "/home/user/dev/my-project"
# Branches tracked by `repo sync`
tracked_branches = ["main", "develop"]
# The branch currently checked out locally
active_branch = "main" # Updated automatically by `repo use-branch`
# Last commit hash synced for each tracked branch
# Updated automatically by `repo sync`
[repositories.last_synced_commits]
main = "a1b2c3d4e5f6..."
develop = "f6e5d4c3b2a1..."

[[repositories]]
name = "another-repo"
local_path = "/home/user/dev/another-repo"
tracked_branches = ["release-v1"]
active_branch = "release-v1"
[repositories.last_synced_commits]
release-v1 = "deadbeef..."

# ... other repositories ...
```

**Note:** You *must* provide the ONNX model and tokenizer paths via one of these methods (arguments, environment variables, or config file) for commands like `index`, `query`, and `repo sync` to work. The `repositories` section is managed automatically by the `repo` subcommands.

## Usage (CLI)

This section focuses on the `vectordb-cli` command-line tool.

### Global Options

These options can be used with most commands:

-   `-m, --onnx-model <PATH>`: Path to the ONNX model file (overrides config & env var).
-   `-t, --onnx-tokenizer-dir <PATH>`: Path to the ONNX tokenizer directory (overrides config & env var).

### Simple Commands (`simple`)

These commands operate on a default, non-repository-specific Qdrant collection (`vectordb-code-search`).

#### `simple index`

Recursively indexes files in specified directories or specific files into the default collection.

```bash
vectordb-cli simple index <PATHS>... [-e <ext>] [--extension <ext>]
```

-   `<PATHS>...`: One or more file or directory paths to index.
-   `-e <ext>`, `--extension <ext>`: Optional: Filter by specific file extensions (without the dot, e.g., `-e rs`, `-e py`). If omitted, attempts to parse based on known extensions.

#### `simple query`

Performs a semantic search against the default collection.

```bash
vectordb-cli simple query "<query text>" [-l <limit>] [--lang <language>] [--type <element_type>]
```

-   `<query text>`: The natural language query.
-   `-l <limit>`, `--limit <limit>` (Optional): Max number of results (default: 10).
-   `--lang <language>` (Optional): Filter by language (e.g., `rust`, `python`).
-   `--type <element_type>` (Optional): Filter by code element type (e.g., `function`).

#### `simple clear`

Deletes the entire simple index collection (`vectordb-code-search`). This does **not** affect repository indices. Requires confirmation unless `-y` is provided.

```bash
vectordb-cli simple clear [-y]
```
-   `-y`: Confirm deletion without prompting.

### Repository Management (`repo`)

This subcommand group manages configurations for Git repositories, allowing you to index and query specific branches within dedicated Qdrant collections (`repo_<repository_name>`).

#### `repo add`

Clones a Git repository locally (if not already present) and adds it to the managed list.

```bash
vectordb-cli repo add --url <repo-url> [--local-path <path>] [--name <repo-name>] [--branch <branch-name>] [--remote <remote_name>] [--ssh-key <path>] [--ssh-passphrase <passphrase>]
```

-   `--url <repo-url>`: The URL of the Git repository (HTTPS or SSH).
-   `--local-path <path>` (Optional): Local directory to clone into (defaults to `<config_dir>/repos/<repo_name>`).
-   `--name <repo-name>` (Optional): Name for the repository configuration (defaults to deriving from URL).
-   `--branch <branch-name>` (Optional): Initial branch to track (defaults to the repo's default).
-   `--remote <remote_name>` (Optional): Name for the Git remote (defaults to "origin").
-   `--ssh-key <path>` (Optional): Path to the SSH private key file for authentication.
-   `--ssh-passphrase <passphrase>` (Optional): Passphrase for the SSH key.

#### `repo config`

Configure repository management settings.

```bash
vectordb-cli repo config set-repo-base-path <path>
```

-   `<path>`: The directory path where all repositories will be stored by default.

This command sets the global repository storage location. New repositories added with `repo add` will be stored in this directory unless overridden with `--local-path`. Existing repositories will remain at their current locations.

#### `repo list`

Lists all configured repositories, their URLs, local paths, tracked branches, and detected indexed languages. Indicates the active repository with a `*`.

```bash
vectordb-cli repo list
```

Example Output:
```
Managed Repositories:
 * my-project (https://github.com/user/my-project.git) -> /home/user/.config/vectordb-cli/repos/my-project
     Default Branch: main
     Active Branch: main
     Tracked Branches: ["main", "develop"]
     Indexed Languages: rust, markdown
   another-repo (https://github.com/user/another.git) -> /home/user/.config/vectordb-cli/repos/another-repo
     Default Branch: main
     Active Branch: main
     Tracked Branches: ["main"]
     Indexed Languages: python
```

#### `repo use`

Sets a repository as the active one, used by default for other `repo` subcommands like `query`, `sync`, `use-branch`, `clear`, `stats`.

```bash
vectordb-cli repo use <name>
```
-   `<name>`: (Required) The name of the repository configuration to activate.

#### `repo remove`

Removes a repository configuration and its corresponding Qdrant collection (`repo_<name>`). This also removes the local clone by default.

```bash
vectordb-cli repo remove <name> [-y]
```
-   `<name>`: (Required) The name of the repository configuration to remove.
-   `-y`: Skip confirmation prompt.

**This operation is irreversible and deletes the Qdrant data and local clone.**

#### `repo use-branch`

Checks out a specific branch in the active repository locally and adds it to the list of tracked branches for syncing.

```bash
vectordb-cli repo use-branch <branch_name>
```
-   `<branch_name>`: (Required) The name of the branch to check out and track. Fetches from the configured remote if the branch isn't available locally.

#### `repo sync`

Fetches updates from the configured remote for the *currently checked-out, tracked branch* of the active repository (or specified repository). It calculates changes since the last sync and updates the Qdrant index accordingly (adding/modifying/deleting points).

```bash
vectordb-cli repo sync [-n <name>] [--name <name>] [-e <ext>,...] [--extensions <ext>,...] [--force]
```
-   `-n <name>`, `--name <name>` (Optional): Name of the repository to sync. Defaults to the active repository.
-   `-e <ext>,...`, `--extensions <ext>,...` (Optional): Specify file extensions to sync (without the dot, comma-separated or multiple flags: `-e rs,py` or `-e rs -e py`). If omitted, syncs files matching known parsers.
-   `--force` (Optional): Force a full re-index of the specified files for the branch, ignoring the last synced commit state.

#### `repo clear`

Clears the index (Qdrant collection `repo_<repo_name>`) for a specific repository without removing the repository configuration or local clone. Requires confirmation unless `-y` is provided.

```bash
vectordb-cli repo clear [-n <name>] [--name <name>] [-y]
```
-   `-n <name>`, `--name <name>` (Optional): The name of the repository index to clear. Defaults to the *active* repository.
-   `-y`: Confirm deletion without prompting.

**This operation is irreversible.**

#### `repo query`

Performs a semantic search across the indexed data for the *active repository*.

```bash
vectordb-cli repo query "<query text>" [-l <limit>] [--lang <language>] [--type <element_type>]
```
-   `<query text>`: The natural language query.
-   `-l <limit>`, `--limit <limit>` (Optional): Max number of results (default: 10).
-   `--lang <language>` (Optional): Filter by language (e.g., `rust`, `python`).
-   `--type <element_type>` (Optional): Filter by code element type (e.g., `function`).

Results display file paths (relative to the repository root), line numbers, scores, and the relevant code chunk.

#### `repo stats`

Displays statistics (like point count) about the Qdrant collection for the *active repository*.

```bash
vectordb-cli repo stats
```

## Development

The project has unit test coverage and end-to-end testing for key features.

```bash
# Run tests without server features (faster, fewer dependencies)
cargo test

# Run tests including server functionality
cargo test --features server

# Run only ignored tests (many server tests are ignored as they require a running server)
cargo test --features server -- --ignored

# Run clippy
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt
```

Certain tests are conditionally compiled based on feature flags to allow for faster testing during development. Server-specific functionality is guarded behind the `server` feature flag.

## Contributing

(Contribution guidelines)

## License

MIT License
