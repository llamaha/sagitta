# Vectordb-Core Refactoring Completion Plan

**Goal:** Move the remaining core logic (Edit, Search, Repository Listing Helper) from the main `vectordb-cli` crate (`src/`) into the `vectordb-core` library crate (`crates/vectordb-core/src/`) to finalize the library refactoring.

---

## Phase 1: Move Edit Logic

**Objective:** Relocate the `apply_edit` function and related code to `vectordb-core`.

1.  **Identify Code:**
    *   Core function: `apply_edit` in `src/edit/engine.rs`.
    *   Supporting structs/functions: Review `src/edit/mod.rs` and `src/edit/engine.rs` for necessary helpers.
    *   Dependencies: Note any external crates (`tree-sitter`, etc.) or internal modules (`config`, `error`) used.
2.  **Create Core Module:**
    *   Create file: `crates/vectordb-core/src/edit/mod.rs`.
    *   *(Optional)* Create `crates/vectordb-core/src/edit/engine.rs` if structure warrants it.
3.  **Move Code:**
    *   Transfer `apply_edit` and identified supporting code to the new `crates/vectordb-core/src/edit/` location.
4.  **Update Dependencies:**
    *   Add any necessary external crate dependencies to `crates/vectordb-core/Cargo.toml`.
5.  **Fix Internal Imports:**
    *   Adjust `use` statements within the moved code to use `crate::` paths for dependencies within `vectordb-core`.
6.  **Export API:**
    *   In `crates/vectordb-core/src/lib.rs`:
        *   Add `pub mod edit;`.
        *   Add `pub use edit::apply_edit;`.
7.  **Cleanup Main Crate:**
    *   Remove the moved code from `src/edit/`. Consider leaving the module file (`src/edit/mod.rs`) if it orchestrates CLI-specific aspects, but remove the core engine logic.
8.  **Verify:**
    *   Run `cargo check -p vectordb-core`. Address errors.

---

## Phase 2: Move Search Logic

**Objective:** Relocate the core semantic search function and related code to `vectordb-core`.

1.  **Identify Code:**
    *   Core function: Locate the main semantic search function within `src/vectordb/search/` (e.g., might be in `mod.rs` or `vector.rs`). Let's assume it's `vector_search_internal` for planning.
    *   Core struct: `SearchResult` in `src/vectordb/search/result.rs`.
    *   Supporting modules: `src/vectordb/search/chunking.rs`, `src/vectordb/search/vector.rs`, etc.
    *   Dependencies: Note external crates (`qdrant-client`, `ndarray`, etc.) and internal modules (`embedding`, `config`, `error`).
2.  **Create Core Module:**
    *   Create file: `crates/vectordb-core/src/search/mod.rs`.
    *   Replicate sub-structure as needed (e.g., `crates/vectordb-core/src/search/result.rs`, `crates/vectordb-core/src/search/chunking.rs`).
3.  **Move Code:**
    *   Transfer the core search function(s), `SearchResult`, and supporting modules/code to `crates/vectordb-core/src/search/`.
4.  **Update Dependencies:**
    *   Add necessary external crate dependencies to `crates/vectordb-core/Cargo.toml`.
5.  **Fix Internal Imports:**
    *   Adjust `use` statements within the moved code for `vectordb-core` internal paths.
6.  **Export API:**
    *   In `crates/vectordb-core/src/lib.rs`:
        *   Add `pub mod search;`.
        *   Add `pub use search::{SearchResult, [ActualSearchFunctionName] as search_semantic};` (using the actual name found).
7.  **Cleanup Main Crate:**
    *   Remove the moved code from `src/vectordb/search/`.
8.  **Verify:**
    *   Run `cargo check -p vectordb-core`. Address errors.

---

## Phase 3: Move Repository Listing Helper

**Objective:** Relocate `get_managed_repos_from_config` to `vectordb-core`.

1.  **Identify Code:**
    *   Function: `get_managed_repos_from_config` in `src/cli/repo_commands/list.rs`.
    *   Return struct: Find the definition of the struct returned by this function (e.g., `ManagedRepositories`).
    *   Dependencies: Uses `AppConfig`, `RepositoryConfig`.
2.  **Determine Location:**
    *   Target file: `crates/vectordb-core/src/config.rs` seems appropriate.
3.  **Move Code:**
    *   Move the function definition and its return struct definition to `crates/vectordb-core/src/config.rs`.
    *   Ensure both the function and the struct are public (`pub`).
4.  **Fix Internal Imports:**
    *   Ensure `AppConfig`, `RepositoryConfig` are in scope.
5.  **Export API:**
    *   In `crates/vectordb-core/src/lib.rs`:
        *   Add `pub use config::{get_managed_repos_from_config, [ManagedRepositoriesStructName]};` (using the actual struct name).
6.  **Cleanup Main Crate:**
    *   Remove the function from `src/cli/repo_commands/list.rs`.
7.  **Verify:**
    *   Run `cargo check -p vectordb-core`. Address errors.

---

## Phase 4: Final Integration and Testing

**Objective:** Ensure `vectordb-core` compiles and tests pass, then fix `relay` to use the refactored library.

1.  **Workspace Check:**
    *   Run `cargo check --workspace`.
    *   Run `cargo test --workspace`. Address any errors, likely focusing on `vectordb-core`.
2.  **Fix `relay` Imports/Calls:**
    *   Correct imports in `relay` (`tools/search.rs`, `tools/edit.rs`, `tools/repo/actions/list_repositories.rs`) to use the functions now exported from `vectordb_core`.
    *   Adjust function calls as needed based on the potentially changed signatures of the moved functions.
3.  **Test `relay`:**
    *   Run `cargo test -p relay`. Address any errors specific to `relay`.
4.  **Commit:** Once all tests pass, commit the completed refactoring.

--- 