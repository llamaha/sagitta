# Project Restructuring Plan: `vectordb-core` to Root

This document outlines the plan to restructure the `vectordb` project, moving `vectordb-core` to the root of the workspace and making `vectordb-cli` a crate within `./crates`.

## Current Structure:
- Root (`./`): Houses `vectordb-cli` (as both a library `vectordb_lib` and binary `vectordb-cli`), and is the workspace root.
- `./crates/vectordb-core/`: Contains the `vectordb_core` library.
- `./crates/vectordb-mcp/`: Contains the `vectordb-mcp` server (library and binary).

## Target Structure:
- Root (`./`): Will house the `vectordb-core` library and be the workspace root.
- `./crates/cli/`: Will house the `vectordb-cli` binary crate.
- `./crates/vectordb-mcp/`: Will remain, with updated dependency paths.

## Phases:

### Phase 1: Configuration Changes (Cargo.toml)

1.  **Root `Cargo.toml` Transformation:**
    *   The content of `crates/vectordb-core/Cargo.toml` will become the basis for the new root `Cargo.toml`.
    *   The package will be `vectordb_core` (library).
    *   A `[workspace]` section will be defined:
        ```toml
        [workspace]
        members = [
            ".",            # vectordb-core (root library)
            "crates/cli",   # New CLI crate
            "crates/vectordb-mcp"
        ]
        resolver = "2"
        ```

2.  **New `crates/cli/Cargo.toml` Creation:**
    *   A new manifest for the `vectordb-cli` package.
    *   Defines a binary `[[bin]] name = "vectordb-cli" path = "src/main.rs"`.
    *   Dependencies:
        *   `vectordb_core = { path = "../../" }`
        *   CLI-specific dependencies (e.g., `clap`, `ctrlc`) migrated from the old root `Cargo.toml`.
        *   Relevant features, dev-dependencies, and build-dependencies from the old root `Cargo.toml`.

3.  **Update `crates/vectordb-mcp/Cargo.toml`:**
    *   Modify the path dependency for `vectordb_core` to `../../` in both `[dependencies]` and `[dev-dependencies]`.

### Phase 2: Code Migration

1.  **Relocate `vectordb-core` Source Code:**
    *   Move all contents from `crates/vectordb-core/src/*` to `src/*` (root).
    *   Delete the empty `crates/vectordb-core/` directory.

2.  **Relocate `vectordb-cli` Source Code:**
    *   Create `crates/cli/src/`.
    *   Move current root `src/bin/vectordb-cli.rs` to `crates/cli/src/main.rs`.
    *   Transfer other CLI-specific modules/files from current root `src/*` (e.g., `src/cli/`, `src/edit/`, relevant parts of `src/lib.rs`) into `crates/cli/src/`.

3.  **Adjust `use` Statements:**
    *   Update `use` statements across all affected Rust files in `src/`, `crates/cli/src/`, and `crates/vectordb-mcp/src/` to reflect new module paths and crate dependencies.
    *   Ensure `vectordb-core`'s `src/lib.rs` correctly defines its public API via `pub use`.

### Phase 3: Build, Test, and Refine

1.  **Build and Check:**
    *   Use `cargo check --all-targets --workspace` and `cargo build --all-targets --workspace` to identify and fix build issues.
2.  **Test:**
    *   Run `cargo test --all-targets --workspace` to ensure all tests pass.
    *   Commit changes after tests pass.
3.  **Iterate:** Address any remaining issues. 