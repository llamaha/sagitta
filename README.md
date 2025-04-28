# VectorDB CLI

VectorDB CLI is a command-line client for managing indexed code repositories. It allows developers to index, search, and interact with codebase embeddings directly from the terminal.

vectordb-cli is powered by the [vectordb-core](./crates/vectordb-core/README.md) library, and they are currently packaged together in this repository. This structure facilitates rapid development during these early phases. In the future, `vectordb-core` will be migrated to a standalone library crate, suitable for use as a dependency in other projects.

## Documentation

Detailed documentation can be found in the [docs](./docs) directory:

- [Setup Guide](./docs/SETUP.md) 
- [Edit Feature](./docs/edit_feature.md)
- [Local Quickstart](./docs/local_quickstart.md)
- [Library Quickstart](./docs/library_quickstart.md)
- [Compile Options](./docs/compile_options.md) 
- [CUDA Setup](./docs/CUDA_SETUP.md)
- [MacOS GPU Setup](./docs/MACOS_GPU_SETUP.md)
- [CodeBERT Setup](./docs/CODEBERT_SETUP.md)

## Getting Started

See the [Setup Guide](./docs/SETUP.md) for instructions on building and running the project.

## Contributing

Please refer to [CONTRIBUTING.md](./CONTRIBUTING.md) (TODO: Create this file) for contribution guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](./LICENSE-MIT) file for details.