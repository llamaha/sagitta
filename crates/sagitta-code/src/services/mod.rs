pub mod file_watcher;
pub mod commit_generator;
pub mod auto_commit;
pub mod sync_orchestrator;
pub mod auto_title_updater;

pub use file_watcher::{FileWatcherService, FileWatcherConfig, FileChangeEvent, FileChangeType};
pub use commit_generator::CommitMessageGenerator;
pub use auto_commit::{AutoCommitter, CommitResult, RepositoryState};
pub use sync_orchestrator::{SyncOrchestrator, SyncResult, RepositorySyncStatus};
pub use auto_title_updater::{AutoTitleUpdater, AutoTitleConfig, ConversationUpdateEvent};