# `sagitta-cli`

`sagitta-cli` is a command-line interface for `sagitta-search`, allowing you to manage and search indexed code repositories directly from your terminal.

## Installation

`sagitta-cli` is part of the `sagitta` workspace. To build it, navigate to the root of the `sagitta` repository and run:

```bash
cargo build --release --package sagitta-cli
```

The binary will be located at `target/release/sagitta-cli`.

## Prerequisites

Ensure you have set up the ONNX Runtime and any necessary environment variables (like `LD_LIBRARY_PATH`) as described in the main `sagitta-search` [README.md](../../README.md#prerequisites).

## Usage Examples

Here are some common commands:

### Add a Repository
Adds a new repository configuration and clones it if necessary.
```bash
sagitta-cli repo add /path/to/your/repository --name my-repo
# Or clone from URL
sagitta-cli repo add --url https://gitlab.com/user/repo.git --name my-repo
```

### List Repositories
Lists all configured repositories.
```bash
sagitta-cli repo list
```

### Set Active Repository
Sets the default repository for commands that require one.
```bash
sagitta-cli repo use my-repo
```

### Search Code in Repository
Performs a semantic search within a specified repository (or the active one).
```bash
# Search the active repository
sagitta-cli repo query "your search query"

# Search a specific repository by name
sagitta-cli repo query "your search query" --name my-repo

# Search with filters
sagitta-cli repo query "search query" --name my-repo --lang rust --type function --limit 5
```

### Sync Repository
Fetches updates for the repository, indexes changes, and updates the vector store.
```bash
# Sync the active repository
sagitta-cli repo sync

# Sync a specific repository by name
sagitta-cli repo sync --name my-repo
```

### Remove a Repository
Removes a repository configuration and optionally deletes local data.
```bash
sagitta-cli repo remove my-repo
# To also delete local files (use with caution!)
sagitta-cli repo remove my-repo --delete-local
```

### Initialize Configuration
Creates a new configuration file at the default location, backing up any existing config and generating a unique tenant_id. Run this before using other commands if you don't have a config yet.
```bash
sagitta-cli init
```

*(Note: Many commands operate on the 'active' repository by default. Use `sagitta-cli repo use <name>` to set the active repository.)*

For more detailed command options, you can use the `--help` flag:
```bash
sagitta-cli --help
sagitta-cli repo --help
sagitta-cli repo query --help
# etc.
``` 
