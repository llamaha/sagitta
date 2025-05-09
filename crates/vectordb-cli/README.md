# `vectordb-cli`

`vectordb-cli` is a command-line interface for `vectordb-core`, allowing you to manage and search indexed code repositories directly from your terminal.

## Installation

`vectordb-cli` is part of the `vectordb` workspace. To build it, navigate to the root of the `vectordb` repository and run:

```bash
cargo build --release --package vectordb-cli
```

The binary will be located at `target/release/vectordb-cli`.

## Prerequisites

Ensure you have set up the ONNX Runtime and any necessary environment variables (like `LD_LIBRARY_PATH`) as described in the main `vectordb-core` [README.md](../../README.md#prerequisites).

## Usage Examples

Here are some common commands:

### Add a Repository
Adds a new repository configuration and clones it if necessary.
```bash
vectordb-cli repo add /path/to/your/repository --name my-repo
# Or clone from URL
vectordb-cli repo add --url https://gitlab.com/user/repo.git --name my-repo
```

### List Repositories
Lists all configured repositories.
```bash
vectordb-cli repo list
```

### Set Active Repository
Sets the default repository for commands that require one.
```bash
vectordb-cli repo use my-repo
```

### Search Code in Repository
Performs a semantic search within a specified repository (or the active one).
```bash
# Search the active repository
vectordb-cli repo query "your search query"

# Search a specific repository by name
vectordb-cli repo query "your search query" --name my-repo

# Search with filters
vectordb-cli repo query "search query" --name my-repo --lang rust --type function --limit 5
```

### Sync Repository
Fetches updates for the repository, indexes changes, and updates the vector store.
```bash
# Sync the active repository
vectordb-cli repo sync

# Sync a specific repository by name
vectordb-cli repo sync --name my-repo
```

### Remove a Repository
Removes a repository configuration and optionally deletes local data.
```bash
vectordb-cli repo remove my-repo
# To also delete local files (use with caution!)
vectordb-cli repo remove my-repo --delete-local
```

*(Note: Many commands operate on the 'active' repository by default. Use `vectordb-cli repo use <name>` to set the active repository.)*

For more detailed command options, you can use the `--help` flag:
```bash
vectordb-cli --help
vectordb-cli repo --help
vectordb-cli repo query --help
# etc.
``` 
