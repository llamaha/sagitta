# vectordb-cli

A lightweight command-line tool for fast, local code search using semantic retrieval powered by ONNX models and Qdrant. Now with multi-repository and branch-aware indexing!

**Note:** This repository contains both the `vectordb-cli` command-line tool and the underlying `vectordb_lib` library.

## Table of Contents

-   [Features](#features)
-   [Use Cases](#use-cases)
-   [Supported Languages](#supported-languages)
-   [Setup](#setup)
    -   [Prerequisites](#prerequisites)
    -   [Qdrant Setup](#qdrant-setup)
    -   [Environment Setup Guides](#environment-setup-guides)
-   [Installation](#installation)
-   [Configuration](#configuration)
    -   [Environment Variables](#environment-variables)
    -   [Configuration File (`config.toml`)](#configuration-file-configtoml)
-   [Usage (CLI)](#usage-cli)
    -   [Global Options](#global-options)
    -   [Repository Management (`repo`)](#repository-management-repo)
        -   [`repo add`](#repo-add)
        -   [`repo list`](#repo-list)
        -   [`repo use`](#repo-use)
        -   [`repo remove`](#repo-remove)
        -   [`repo use-branch`](#repo-use-branch)
        -   [`repo sync`](#repo-sync)
    -   [`index`](#index)
    -   [`query`](#query)
    -   [`stats`](#stats)
    -   [`list`](#list)
    -   [`clear`](#clear)
-   [Library (`vectordb_lib`)](#library-vectordb_lib)

## Features

-   **Semantic Search:** Finds relevant code chunks based on meaning using ONNX models.
-   **Repository Management:** Manage configurations for multiple Git repositories.
-   **Branch-Aware Indexing:** Track and sync specific branches within repositories.
-   **Qdrant Backend:** Utilizes a Qdrant vector database instance for scalable storage and efficient search.
-   **Local or Remote Qdrant:** Can connect to a local Dockerized Qdrant or a remote instance.
-   **Simple Indexing (Legacy):** Recursively indexes specified directories (can be used alongside repository management).
-   **Configurable:** Supports custom ONNX embedding models/tokenizers and Qdrant connection details via config file or environment variables.

## Use Cases

-   **Debugging Assistance:** Use semantic search to find potentially related code sections when investigating bugs. Combine with LLMs by providing relevant code snippets found through queries for diagnosis, explanation, or generating flow charts.
-   **Code Exploration & Understanding:** Quickly locate definitions, implementations, or usages of functions, classes, or variables across large codebases or multiple repositories, even if you don't know the exact name.
-   **Finding Examples:** Locate examples of how a particular API, library function, or design pattern is used within your indexed code.
-   **Onboarding:** Help new team members find relevant code sections related to specific features or concepts they need to learn.
-   **Building AI Coding Tools:** Integrate the `vectordb_lib` library into your own AI-powered development tools, agents, or custom workflows.
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

This starts Qdrant with the default gRPC port (6333) and HTTP/REST port (6334) mapped to your host. Data will be persisted in the `qdrant_storage` directory in your current working directory.

**Option 2: Qdrant Cloud or Other Deployment**

Follow the instructions for your chosen deployment method. You will need the **URL** (including `http://` or `https://` and the port, typically 6333 for gRPC) and potentially an **API Key** if required by your setup.

### Environment Setup Guides

For specific environment configurations (GPU acceleration), refer to the guides in the `docs/` directory:

-   [docs/CUDA_SETUP.md](./docs/CUDA_SETUP.md) (Linux with NVIDIA GPU)
-   [docs/MACOS_GPU_SETUP.md](./docs/MACOS_GPU_SETUP.md) (macOS with Metal GPU)
-   [docs/CODEBERT_SETUP.md](./docs/CODEBERT_SETUP.md) (Using CodeBERT model - *may be outdated*)

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

-   `QDRANT_URL`: URL of the Qdrant gRPC endpoint (e.g., `http://localhost:6333`). Defaults to `http://localhost:6333` if not set.
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

# --- Repository Management ---
# The active repository (used by default for commands like sync, query)
# Set via `repo use <name>`
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

These can be used with most commands:

-   `--qdrant-url`: Override Qdrant URL.
-   `--qdrant-api-key`: Provide Qdrant API key.
-   `--onnx-model-path-arg`: Override path to ONNX model file.
-   `--onnx-tokenizer-dir-arg`: Override path to ONNX tokenizer directory.

### Repository Management (`repo`)

Manages the Git repositories known to `vectordb-cli`.

#### `repo add`

Adds a new repository to the configuration and clones it locally.

```bash
vectordb-cli repo add <url> [--name <name>] [--branch <branch>] [--remote <remote_name>] [--ssh-key <path>] [--ssh-passphrase <passphrase>]
```

-   `<url>`: The Git URL of the repository (e.g., `https://github.com/user/repo.git` or `git@github.com:user/repo.git`).
-   `--name <name>` (Optional): A short name to refer to this repository. If omitted, it's derived from the URL (e.g., `repo` from `repo.git`).
-   `--branch <branch>` (Optional): The specific branch to track initially. If omitted, the repository's default branch is used.
-   `--remote <remote_name>` (Optional): The name of the Git remote to use for fetching updates (e.g., `upstream`). Defaults to `origin`.
-   `--ssh-key <path>` (Optional): Path to the SSH private key file (e.g., `~/.ssh/id_rsa`) to use for authentication with this repository.
-   `--ssh-passphrase <passphrase>` (Optional): Passphrase for the SSH private key, if it is encrypted. Requires `--ssh-key`.

The command creates a Qdrant collection named `repo_<name>` for this repository.
It automatically determines the default branch (or uses the one provided via `--branch`), sets it as the `active_branch`, and adds it to the `tracked_branches` list.
The new repository is set as the active repository.

After adding, run `vectordb-cli repo sync <name>` to fetch the initial branch contents and index them.

#### `repo list`

Lists all configured repositories, their URLs, local paths, tracked branches, and detected indexed languages.

```bash
vectordb-cli repo list
```

Output indicates the active repository with a `*`.

```
Managed Repositories:
 * my-project (https://github.com/user/my-project.git) -> /home/user/dev/my-project
     Default Branch: main
     Tracked Branches: ["develop", "main"]
     Indexed Languages: rust, markdown
   another-repo (https://github.com/user/another.git) -> /home/user/dev/another-repo
     Default Branch: main
     Tracked Branches: ["main"]
     Indexed Languages: python
```

#### `repo use`

Sets a repository as the active one, used by default for commands like `query`, `sync`, `use-branch`.

```bash
vectordb-cli repo use my-cool-project
```

**Arguments:**
-   `name`: (Required) The name of the repository configuration to activate.

#### `repo remove`

Removes a repository configuration and optionally deletes its corresponding Qdrant collection.

```bash
# Remove configuration only
vectordb-cli repo remove another

# Remove configuration AND delete Qdrant collection (requires confirmation)
vectordb-cli repo remove another --delete-collection
```

**Arguments:**
-   `name`: (Required) The name of the repository configuration to remove.
-   `--delete-collection`: If set, deletes the `repo_<name>` collection from Qdrant.

#### `repo use-branch`

Checks out a specific branch in the active repository locally and adds it to the list of tracked branches for syncing.

```bash
# Assuming 'my-cool-project' is the active repo:
# Checkout 'develop' branch and track it
vectordb-cli repo use-branch develop

# Checkout and track a feature branch
vectordb-cli repo use-branch feature/new-thing
```

**Arguments:**
-   `name`: (Required) The name of the branch to check out and track. Fetches from `origin` if the branch isn't available locally.

#### `repo sync`

Fetches updates from the `origin` remote for the currently checked-out, tracked branch of the active repository (or specified repository). It calculates the changes since the last sync and updates the Qdrant index accordingly (adding new/modified files, deleting removed/renamed files).

```bash
# Sync the active repository's current branch
vectordb-cli repo sync

# Sync a specific repository (uses its currently checked-out tracked branch)
vectordb-cli repo sync my-cool-project
```

**Arguments:**
-   `name`: Optional name of the repository to sync. Defaults to the active repository.

**Note:** Currently only fetches from the configured remote (`origin` by default) and primarily supports SSH key authentication (via `--ssh-key` in `repo add` or system defaults like `ssh-agent`). Support for other credential types (HTTPS tokens, etc.) is planned.

**Manual Testing for SSH:** To test SSH key authentication, try adding a private repository using its SSH URL (`git@...`) and provide the path to your corresponding private key using `--ssh-key`. Ensure your key doesn't require a passphrase for automated testing, or provide it with `--ssh-passphrase` (not recommended for security). Running `repo sync` should then succeed if authentication works.

### `index` (Legacy Directory Indexing)

Indexes files directly from specified directories into a *single, shared* Qdrant collection (`vectordb-code-search` by default - this is separate from repository collections). This is useful for indexing codebases not managed as Git repositories.

```bash
# Index a single directory
vectordb-cli index /path/to/your/code

# Index multiple directories
vectordb-cli index /path/to/projectA /path/to/projectB

# Index specific file types (e.g., Rust and Markdown)
vectordb-cli index /path/to/project -t rs md
```

**Arguments:**
-   `dirs`: (Required) One or more directory paths to index.
-   `-t, --type`: Optional file extensions to include (without dots).
-   `--chunk-max-length`: Max lines per text chunk (default: 512).
-   `--chunk-overlap`: Lines of overlap between chunks (default: 64).

### `query`

Performs a semantic search across the indexed data for the active repository, specified repositories, or all repositories.

```bash
vectordb-cli query "<query text>" [-r <repo_name>...] [--all-repos] [-b <branch>] [-l <limit>] [--lang <language>] [--type <element_type>]
```

-   `<query text>`: The natural language query to search for.
-   `-r <repo_name>`, `--repo <repo_name>` (Optional): Specify one or more repository names to search within. Conflicts with `--all-repos`.
-   `--all-repos` (Optional): Search across all configured repositories. Conflicts with `--repo`.
-   `-b <branch>`, `--branch <branch>` (Optional): Filter results by a specific branch name within the target repository/repositories.
-   `-l <limit>`, `--limit <limit>` (Optional): Maximum number of results to return (default: 10).
-   `--lang <language>` (Optional): Filter results by programming language (e.g., `rust`, `python`).
-   `--type <element_type>` (Optional): Filter results by code element type (e.g., `function`, `struct`).

If neither `--repo` nor `--all-repos` is provided, the search defaults to the currently active repository.

Results are displayed with file paths (relative to the repository root for repo searches, absolute for legacy index searches), line numbers, scores, and the relevant code chunk.

### `stats`

Displays statistics about a specific Qdrant collection. Defaults to the active repository's collection.

```bash
# Show stats for the active repository's collection
vectordb-cli stats

# Show stats for a specific repository collection
vectordb-cli stats --collection repo_my-cool-project

# Show stats for the legacy collection
vectordb-cli stats --collection vectordb-code-search
```

**Arguments:**
-   `--collection`: Optional collection name. Defaults to the active repository's collection or `vectordb-code-search`.

### `list`

Lists unique root directories indexed *within a specific collection*. Defaults to the active repository's collection.

```bash
# List indexed roots for the active repository (should be just the repo root)
vectordb-cli list

# List indexed roots for a specific repository collection
vectordb-cli list --collection repo_my-cool-project

# List indexed roots for the legacy collection
vectordb-cli list --collection vectordb-code-search
```

**Arguments:**
-   `--collection`: Optional collection name. Defaults to the active repository's collection or `vectordb-code-search`.

### `clear`

Removes data from a specific Qdrant collection based on indexed directory paths. Defaults to the active repository's collection.

```bash
# Clear data originating from a specific path within the active repo collection (requires confirmation)
# (Note: `repo sync` is the preferred way to manage repo data)
vectordb-cli clear /path/to/active/repo/subdirectory

# Clear data for a path within the legacy collection (requires confirmation)
vectordb-cli clear /path/to/indexed/dir --collection vectordb-code-search

# Clear ALL data from a specific collection (requires confirmation)
vectordb-cli clear --all --collection repo_my-cool-project
```

**Arguments:**
-   `dirs`: Optional directory paths whose indexed data should be removed.
-   `--all`: Remove all data from the specified collection.

## Library (`vectordb_lib`)

This crate also provides the `vectordb_lib` library, which contains the core logic for configuration, code parsing, embedding management, and interacting with the vector database.

While the CLI provides a convenient interface, you can use the library programmatically for more custom integrations.

*   **Quickstart Guide:** [docs/library_quickstart.md](./docs/library_quickstart.md)
*   **API Documentation:** [https://docs.rs/vectordb-cli](https://docs.rs/vectordb-cli)

See the crate-level documentation within the library (`src/lib.rs`) for a conceptual example and overview of the main components like `EmbeddingHandler`.

**Important Runtime Dependency:**

Users of the `vectordb_lib` library must ensure the ONNX Runtime shared libraries are available when running their application. This is because the library itself does not bundle these dependencies.

Refer to the [ONNX Runtime installation guide](https://onnxruntime.ai/docs/install/) for instructions on how to install the runtime system-wide, or ensure the necessary shared library files (`.so`/`.dylib`/`.dll`) are discoverable via the system's library path (e.g., using `LD_LIBRARY_PATH` on Linux).

## Development

(Include instructions for setting up the dev environment, running tests, etc.)

```bash
# Run tests
cargo test

# Run clippy
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt
```

## Contributing

(Contribution guidelines)

## License

MIT License
