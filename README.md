# VectorDB CLI

VectorDB CLI is a command-line client for managing indexed code repositories. It allows developers to index, search, and interact with codebase embeddings directly from the terminal.

vectordb-cli is powered by the [vectordb-core](./crates/vectordb-core/README.md) library, and they are currently packaged together in this repository. This structure facilitates rapid development during these early phases. In the future, `vectordb-core` will be migrated to a standalone library crate, suitable for use as a dependency in other projects.

There is also a [vectordb-mcp](./crates/vectordb-mcp/README.md) that allows this tool to operate as an MCP server.  It's recommended to compile both CLI and MCP as they compliment each other and both are lightweight frontends to the same core library.

## Getting Started

See the [Setup Guide](./docs/SETUP.md) for instructions on building, running, and generating the ONNX model and tokenizer files required for vectordb-cli. The ONNX model and tokenizer are now generated locally using scripts/setup_onnx_model.sh and are not stored in the repository.

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](./LICENSE-MIT) file for details.
