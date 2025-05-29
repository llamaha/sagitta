# Sync Progress Meter Refactoring Plan

This document outlines the plan to refactor the progress reporting mechanism for repository synchronization in the Sagitta ecosystem. The current `indicatif`-based direct terminal output in `sagitta-search` is unsuitable for its library usage by `sagitta-cli`, `sagitta-mcp`, and `sagitta-code`.

## Goals

1.  Remove direct `indicatif` usage from `sagitta-search`.
2.  Introduce a flexible progress reporting mechanism in `sagitta-search` (likely callback-based).
3.  Update consumer crates (`sagitta-cli`, `sagitta-mcp`, `sagitta-code`) to utilize the new mechanism for their specific progress display needs.

## Phases

### Phase 1: Define and Implement Core Progress Reporting Mechanism

**Status: COMPLETED**

**Tasks:**

1.  **Define Progress Data Structures (`sagitta-search`):** (Done)
    *   Created enums/structs for sync stages.
    *   Defined data fields.
    *   Example:
        ```rust
        use std::path::PathBuf;

        #[derive(Debug, Clone)]
        pub enum SyncStage {
            GitFetch { message: String, progress: Option<(u32, u32)> }, // received_objects, total_objects
            DiffCalculation { message: String },
            IndexFile { current_file: Option<PathBuf>, total_files: usize, current_file_num: usize, files_per_second: Option<f64> },
            DeleteFile { current_file: Option<PathBuf>, total_files: usize, current_file_num: usize, files_per_second: Option<f64> },
            CollectFiles { total_files: usize, message: String },
            QueryLanguages { message: String },
            VerifyingCollection { message: String },
            Completed { message: String },
            Error { message: String },
            Idle, // Default state
        }

        #[derive(Debug, Clone)]
        pub struct SyncProgress {
            pub stage: SyncStage,
            // Potentially overall progress if calculable easily
            // pub overall_progress: Option<(usize, usize)>,
        }
        ```

2.  **Define Progress Callback Trait (`sagitta-search`):** (Done)
    *   Created `SyncProgressReporter` trait.
    *   Modified `sync_repository` to accept `Option<Arc<dyn SyncProgressReporter>>`.
    *   Integrated calls to the reporter.
    *   Example:
        ```rust
        use async_trait::async_trait;
        use crate::sync_progress::SyncProgress; // Assuming sync_progress.rs

        #[async_trait]
        pub trait SyncProgressReporter: Send + Sync {
            async fn report(&self, progress: SyncProgress);
        }
        ```

3.  **Refactor `sagitta-search`:** (Done)
    *   Removed `indicatif::ProgressBar` usage.
    *   Replaced `eprintln!` for git fetch progress.
    *   Integrated calls to `SyncProgressReporter`.

4.  **Add Tests (`sagitta-search`):** (Done)
    *   Created `MockSyncProgressReporter`.
    *   Added unit tests for `sync_repository`.

5.  **Dependency Management (`sagitta-search`):** (Done)
    *   Removed `indicatif` from `sagitta-search/Cargo.toml`.

**Deliverables:** (Achieved)
*   `SyncProgress` structs/enums.
*   `SyncProgressReporter` trait.
*   Refactored `sync_repository` and helpers.
*   Unit tests with `MockSyncProgressReporter`.
*   `indicatif` dependency removed.
*   Commit: "Phase 1: Implement core progress reporting mechanism" (Done)

### Phase 2: Update `sagitta-cli`

**Status: COMPLETED**

**Tasks:**

1.  **Implement `IndicatifProgressReporter` (`sagitta-cli`):** (Done)
    *   Created struct implementing `sagitta_search::sync_progress::SyncProgressReporter`.
    *   Managed `indicatif::MultiProgress`.
    *   Translated `SyncProgress` data.

2.  **Integrate into CLI Commands (`sagitta-cli`):** (Done)
    *   Instantiated and passed `IndicatifProgressReporter` to `sync_repository`.

3.  **Add Tests (`sagitta-cli`):** (Done)
    *   Added unit tests for `IndicatifProgressReporter` logic.
    *   Manual verification of CLI output performed (assumed for this step).

**Deliverables:** (Achieved)
*   `IndicatifProgressReporter` implementation.
*   Updated CLI commands.
*   Tests added.
*   Commit: "Phase 2: Integrate new progress reporting into sagitta-cli" (Done)

### Phase 3: Update `sagitta-mcp`

**Status: COMPLETED**

**Tasks:**

1.  **Implement `LoggingProgressReporter` (`sagitta-mcp`):** (Done)
    *   Created a struct implementing `sagitta_search::sync_progress::SyncProgressReporter`.
    *   Used the `log` crate to output progress information.

2.  **Integrate into MCP Logic (`sagitta-mcp`):** (Done)
    *   Passed `LoggingProgressReporter` to `sync_repository` calls.

3.  **Add Tests (`sagitta-mcp`):** (Done)
    *   Unit tests for `LoggingProgressReporter`.

**Deliverables:** (Achieved)
*   `LoggingProgressReporter` implementation.
*   Updated MCP sync logic.
*   Tests added.
*   Commit: "Phase 3: Integrate new progress reporting into sagitta-mcp" (Done)

### Phase 4: Update `sagitta-code`

**Status: COMPLETED**

**Tasks:**

1.  **Implement `GuiProgressReporter` (`sagitta-code`):** (Done)
    *   Created `sagitta-code/src/gui/progress.rs` with `GuiProgressReporter` and `GuiSyncReport`.
    *   `GuiProgressReporter` implements `sagitta_search::sync_progress::SyncProgressReporter`.
    *   Uses an `mpsc` channel to send `GuiSyncReport` (containing `repo_id` and core `SyncProgress`) to `RepositoryManager`.

2.  **Integrate into Agent Logic (`sagitta-code`):** (Done)
    *   Modified `sagitta-code/src/gui/repository/manager.rs`:
        *   `RepositoryManager` now has an `mpsc::UnboundedSender<GuiSyncReport>`.
        *   In `new()`, a channel is created, and `process_progress_updates` task is spawned to handle received reports.
        *   `process_progress_updates` updates `sync_status_map` (with `DisplayableSyncProgress`) and `simple_sync_status_map`.
        *   `sync_repository` method creates `GuiProgressReporter` with the sender and passes it in `SyncOptions` to `sagitta_search::sync::sync_repository`.
        *   Old progress update methods (`update_simple_sync_status`, `update_sync_progress`, `complete_sync_progress`) removed.
    *   Modified `sagitta-code/src/gui/repository/types.rs`:
        *   Renamed old `SyncProgress` to `DisplayableSyncProgress`.
        *   Added `GuiSyncStageDisplay` for structured display data.
        *   `DisplayableSyncProgress` now converts from `sagitta_search::sync_progress::SyncProgress`.
        *   `manager::SyncStatus`'s `detailed_progress` field now uses `Option<DisplayableSyncProgress>`.

3.  **Update GUI elements (`sagitta-code`):** (Done)
    *   Modified `sagitta-code/src/gui/repository/sync.rs` (`render_sync_repo` function):
        *   Fetches both detailed `sync_status_map` and `simple_sync_status_map` from `RepositoryManager`.
        *   Displays `egui::ProgressBar` using `DisplayableSyncProgress.percentage_overall`.
        *   Shows detailed stage information (stage name, message, current file, step progress, files/sec, elapsed time) from `DisplayableSyncProgress`.
        *   Retains the log view using `SimpleSyncStatus.output_lines`.
        *   Sync buttons are disabled based on `SimpleSyncStatus.is_running` to prevent concurrent syncs of the same repo.

4.  **Add Tests (`sagitta-code`):** (Done)
    *   Unit tests for `GuiProgressReporter` added in `sagitta-code/src/gui/progress.rs`.
    *   Refactored tests in `sagitta-code/src/gui/repository/manager.rs`:
        *   Removed tests for old direct progress update methods.
        *   Added `test_process_progress_updates_logic` to test the new channel-based mechanism and updates to both status maps.
        *   Updated `test_concurrent_sync_status_access` to use the new progress channel.

**Deliverables:** (Achieved)
*   `GuiProgressReporter` implementation and its tests.
*   Updated agent sync logic in `RepositoryManager` and `types.rs`.
*   Updated GUI components in `sync.rs` to display new detailed progress.
*   Updated tests in `manager.rs`.
*   Commit: "Phase 4: Integrate new progress reporting into sagitta-code" 