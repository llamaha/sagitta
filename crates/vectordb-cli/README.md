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

### Initialize Configuration
Creates a default configuration file if one doesn't exist.
```bash
vectordb-cli init
```

### Add a Repository
Indexes a new code repository.
```bash
vectordb-cli repo add /path/to/your/repository --name my-repo
```

### List Repositories
Lists all indexed repositories.
```bash
vectordb-cli repo list
```

### Search Code
Performs a semantic search across indexed repositories.
```bash
vectordb-cli search "your search query"
```

By default, this searches all repositories. You can specify repositories to search in:
```bash
vectordb-cli search "your search query" --repo my-repo --repo another-repo
```

### Update a Repository
Re-indexes an existing repository to pick up changes.
```bash
vectordb-cli repo update my-repo
```

### Remove a Repository
Removes a repository from the index.
```bash
vectordb-cli repo remove my-repo
```

For more detailed command options, you can use the `--help` flag:
```bash
vectordb-cli --help
vectordb-cli repo --help
vectordb-cli search --help
``` 