use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use sagitta_search::RepositoryConfig;
use tokio::sync::mpsc;
pub use sagitta_search::sync_progress::SyncProgress as CoreSyncProgress;
use sagitta_search::sync_progress::SyncStage as CoreSyncStage;

/// Enum representing the different tabs in the repository panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoPanelTab {
    List,
    Add,
    Sync,
    Query,
    SearchFile,
    ViewFile,
    Branches,
}

impl Default for RepoPanelTab {
    fn default() -> Self {
        Self::List
    }
}

/// Repository filtering options
#[derive(Debug, Clone, Default)]
pub struct RepoFilterOptions {
    pub search_term: String,
}

/// Form data for adding a new repository
#[derive(Debug)]
pub struct AddRepoForm {
    pub name: String,
    pub url: String,
    pub branch: String,
    pub local_path: String,
    pub use_local: bool,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub adding: bool,
    pub result_receiver: Option<std::sync::mpsc::Receiver<Result<String, anyhow::Error>>>,
}

impl Clone for AddRepoForm {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            url: self.url.clone(),
            branch: self.branch.clone(),
            local_path: self.local_path.clone(),
            use_local: self.use_local,
            error_message: self.error_message.clone(),
            status_message: self.status_message.clone(),
            adding: self.adding,
            result_receiver: None, // Cannot clone receiver
        }
    }
}

impl Default for AddRepoForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            url: String::new(),
            branch: String::new(),
            local_path: String::new(),
            use_local: false,
            error_message: None,
            status_message: None,
            adding: false,
            result_receiver: None,
        }
    }
}

/// Query options for repository search
#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    pub repo_name: String,
    pub query_text: String,
    pub element_type: Option<String>,
    pub language: Option<String>,
    pub limit: usize,
}

impl QueryOptions {
    pub fn new(repo_name: String) -> Self {
        Self {
            repo_name,
            limit: 10,
            ..Default::default()
        }
    }
}

/// Query result struct to store the results
#[derive(Debug)]
pub struct QueryResult {
    pub is_loading: bool,
    pub success: bool,
    pub error_message: Option<String>,
    pub results: Vec<QueryResultItem>,
    pub channel: Option<crate::gui::repository::query::QueryChannel>,
}

impl Clone for QueryResult {
    fn clone(&self) -> Self {
        Self {
            is_loading: self.is_loading,
            success: self.success,
            error_message: self.error_message.clone(),
            results: self.results.clone(),
            channel: None, // Channel cannot be cloned, so we set it to None
        }
    }
}

impl Default for QueryResult {
    fn default() -> Self {
        Self {
            is_loading: false,
            success: false,
            error_message: None,
            results: Vec::new(),
            channel: None,
        }
    }
}

/// Single item in query results
#[derive(Debug, Clone)]
pub struct QueryResultItem {
    pub score: f32,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub content: String,
}

/// File search options
#[derive(Debug, Clone, Default)]
pub struct FileSearchOptions {
    pub repo_name: String,
    pub pattern: String,
    pub case_sensitive: bool,
}

impl FileSearchOptions {
    pub fn new(repo_name: String) -> Self {
        Self {
            repo_name,
            ..Default::default()
        }
    }
}

/// File view options
#[derive(Debug, Clone, Default)]
pub struct FileViewOptions {
    pub repo_name: String,
    pub file_path: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

impl FileViewOptions {
    pub fn new(repo_name: String) -> Self {
        Self {
            repo_name,
            ..Default::default()
        }
    }
}

/// Repository information with additional UI state
#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub name: String,
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub local_path: Option<PathBuf>,
    pub is_syncing: bool, 
}

impl From<RepositoryConfig> for RepoInfo {
    fn from(config: RepositoryConfig) -> Self {
        Self {
            name: config.name.clone(),
            remote: Some(config.url.clone()),
            branch: config.active_branch.clone().or_else(|| Some(config.default_branch.clone())),
            local_path: Some(config.local_path.clone()),
            is_syncing: false,
        }
    }
}

/// Simplified sync status for displaying indicatif output
#[derive(Debug, Clone, Default)]
pub struct SimpleSyncStatus {
    pub is_running: bool,
    pub is_complete: bool,
    pub is_success: bool,
    pub output_lines: Vec<String>,
    pub final_message: String,
    pub started_at: Option<std::time::Instant>,
}

/// State for the repository panel
#[derive(Debug, Default)]
pub struct RepoPanelState {
    pub active_tab: RepoPanelTab,
    pub repositories: Vec<RepoInfo>,
    pub repository_filter: RepoFilterOptions,
    pub add_repo_form: AddRepoForm,
    pub selected_repo: Option<String>,
    pub selected_repos: Vec<String>,
    pub branch_overrides: std::collections::HashMap<String, String>,
    pub query_options: QueryOptions,
    pub query_result: QueryResult,
    pub file_search_options: FileSearchOptions,
    pub file_view_options: FileViewOptions,
    pub is_loading_repos: bool,
    pub file_search_result: FileSearchResult,
    pub file_view_result: FileViewResult,
    pub sync_status_map: Option<std::collections::HashMap<String, super::manager::SyncStatus>>,
    pub simple_sync_status_map: Option<std::collections::HashMap<String, SimpleSyncStatus>>,
    pub branch_management: BranchManagementState,
}

/// State for branch management operations
#[derive(Debug, Default)]
pub struct BranchManagementState {
    pub selected_repo_for_branches: Option<String>,
    pub available_branches: Vec<String>,
    pub available_tags: Vec<String>,
    pub current_branch: Option<String>,
    pub is_loading_branches: bool,
    pub is_loading_tags: bool,
    pub is_switching_branch: bool,
    pub switch_error: Option<String>,
    pub switch_success: Option<String>,
    pub new_branch_name: String,
    pub is_creating_branch: bool,
    pub create_error: Option<String>,
    pub create_success: Option<String>,
    pub branch_to_delete: Option<String>,
    pub is_deleting_branch: bool,
    pub delete_error: Option<String>,
    pub delete_success: Option<String>,
    pub show_delete_confirmation: bool,
    pub last_sync_result: Option<BranchSyncResult>,
    
    // New fields for target_ref support
    pub manual_ref_input: String,
    pub ref_type_tab: RefTypeTab,
    
    // Channels for async operation results
    pub branch_result_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<BranchOperationResult>>,
    pub tag_result_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<TagOperationResult>>,
    pub switch_result_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<SwitchOperationResult>>,
    pub create_result_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<CreateBranchResult>>,
    pub delete_result_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<DeleteBranchResult>>,
}

/// Different types of Git references that can be displayed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefTypeTab {
    Branches,
    Tags,
    Manual,
}

impl Default for RefTypeTab {
    fn default() -> Self {
        Self::Branches
    }
}

/// Channel for file search results
#[derive(Debug)]
pub struct FileSearchChannel {
    pub sender: mpsc::Sender<FileSearchResult>,
    pub receiver: mpsc::Receiver<FileSearchResult>,
}

/// File search result struct to store search results
#[derive(Debug)]
pub struct FileSearchResult {
    pub is_loading: bool,
    pub error_message: Option<String>,
    pub files: Vec<String>,
    pub channel: Option<FileSearchChannel>,
}

impl Clone for FileSearchResult {
    fn clone(&self) -> Self {
        Self {
            is_loading: self.is_loading,
            error_message: self.error_message.clone(),
            files: self.files.clone(),
            channel: None, // Channel cannot be cloned
        }
    }
}

impl Default for FileSearchResult {
    fn default() -> Self {
        // Create a channel for receiving search results
        let (sender, receiver) = mpsc::channel(10);
        
        Self {
            is_loading: false,
            error_message: None,
            files: Vec::new(),
            channel: Some(FileSearchChannel { sender, receiver }),
        }
    }
}

/// Channel for file view results
#[derive(Debug)]
pub struct FileViewChannel {
    pub sender: mpsc::Sender<FileViewResult>,
    pub receiver: mpsc::Receiver<FileViewResult>,
}

/// File view result struct to store file content
#[derive(Debug)]
pub struct FileViewResult {
    pub is_loading: bool,
    pub error_message: Option<String>,
    pub content: String,
    pub channel: Option<FileViewChannel>,
}

impl Clone for FileViewResult {
    fn clone(&self) -> Self {
        Self {
            is_loading: self.is_loading,
            error_message: self.error_message.clone(),
            content: self.content.clone(),
            channel: None, // Channel cannot be cloned
        }
    }
}

impl Default for FileViewResult {
    fn default() -> Self {
        // Create a channel for receiving file content
        let (sender, receiver) = mpsc::channel(10);
        
        Self {
            is_loading: false,
            error_message: None,
            content: String::new(),
            channel: Some(FileViewChannel { sender, receiver }),
        }
    }
}

/// NEW struct to represent displayable stage information
#[derive(Debug, Clone, Default)]
pub struct GuiSyncStageDisplay {
    pub name: String, // e.g., "Git Fetch", "Indexing"
    pub current_file: Option<String>,
    pub current_progress: Option<(u32, u32)>, // (current, total) items, like (received_objects, total_objects) or (current_file_num, total_files)
    pub files_per_second: Option<f64>,
    pub overall_message: String, // General message for the stage or error
}

/// RENAMED and MODIFIED version of the original SyncProgress struct
#[derive(Debug, Clone, Default)]
pub struct DisplayableSyncProgress { // <<< RENAMED from SyncProgress
    // pub stage: String, // <<< REMOVED, replaced by stage_detail
    pub stage_detail: GuiSyncStageDisplay, // <<< NEW field
    pub current_overall: u64,           // Current progress value (e.g., current_file_num if applicable)
    pub total_overall: u64,             // Total progress value (e.g., total_files if applicable)
    pub percentage_overall: f32,        // Calculated overall percentage (0.0 to 1.0)
    pub message: String, // General message, can be derived from stage_detail.overall_message or specific stage message
    pub elapsed_seconds: f64,   // Time elapsed since sync started (this might be managed outside)
}

impl DisplayableSyncProgress {
    // Conversion from the core SyncProgress type
    pub fn from_core_progress(core_progress: &CoreSyncProgress, elapsed_seconds: f64) -> Self {
        let mut displayable = DisplayableSyncProgress {
            elapsed_seconds, // This might be better set by the caller/manager
            ..Default::default()
        };

        let mut current_overall = 0u64;
        let mut total_overall = 0u64;

        match &core_progress.stage {
            CoreSyncStage::Idle => {
                displayable.stage_detail.name = "Idle".to_string();
                displayable.stage_detail.overall_message = "Waiting for sync to start.".to_string();
            }
            CoreSyncStage::GitFetch { message, progress } => {
                displayable.stage_detail.name = "Git Fetch".to_string();
                displayable.stage_detail.overall_message = message.clone();
                if let Some((received, total)) = progress {
                    displayable.stage_detail.current_progress = Some((*received, *total));
                    current_overall = *received as u64;
                    total_overall = *total as u64;
                }
            }
            CoreSyncStage::DiffCalculation { message } => {
                displayable.stage_detail.name = "Diff Calculation".to_string();
                displayable.stage_detail.overall_message = message.clone();
            }
            CoreSyncStage::IndexFile { current_file, total_files, current_file_num, files_per_second, .. } => {
                displayable.stage_detail.name = "Indexing Files".to_string();
                displayable.stage_detail.current_file = current_file.as_ref().map(|p| p.to_string_lossy().into_owned());
                displayable.stage_detail.current_progress = Some((*current_file_num as u32, *total_files as u32));
                displayable.stage_detail.files_per_second = *files_per_second;
                current_overall = *current_file_num as u64;
                total_overall = *total_files as u64;
                displayable.stage_detail.overall_message = format!("Indexing file {} of {}", current_file_num, total_files);
            }
            CoreSyncStage::DeleteFile { current_file, total_files, current_file_num, files_per_second, .. } => {
                displayable.stage_detail.name = "Deleting Files".to_string();
                displayable.stage_detail.current_file = current_file.as_ref().map(|p| p.to_string_lossy().into_owned());
                displayable.stage_detail.current_progress = Some((*current_file_num as u32, *total_files as u32));
                displayable.stage_detail.files_per_second = *files_per_second;
                current_overall = *current_file_num as u64;
                total_overall = *total_files as u64;
                displayable.stage_detail.overall_message = format!("Deleting file {} of {}", current_file_num, total_files);
            }
            CoreSyncStage::CollectFiles { total_files, message } => {
                displayable.stage_detail.name = "Collecting Files".to_string();
                displayable.stage_detail.overall_message = message.clone();
                // total_overall = *total_files as u64; // No current_overall here, it's a preparatory step
            }
            CoreSyncStage::QueryLanguages { message } => {
                displayable.stage_detail.name = "Querying Languages".to_string();
                displayable.stage_detail.overall_message = message.clone();
            }
            CoreSyncStage::VerifyingCollection { message } => {
                displayable.stage_detail.name = "Verifying Collection".to_string();
                displayable.stage_detail.overall_message = message.clone();
            }
            CoreSyncStage::Completed { message } => {
                displayable.stage_detail.name = "Completed".to_string();
                displayable.stage_detail.overall_message = message.clone();
                total_overall = 1; // Ensure percentage can be 100%
                current_overall = 1;
            }
            CoreSyncStage::Error { message } => {
                displayable.stage_detail.name = "Error".to_string();
                displayable.stage_detail.overall_message = message.clone();
            }
        }

        displayable.message = displayable.stage_detail.overall_message.clone();
        displayable.current_overall = current_overall;
        displayable.total_overall = total_overall;
        if total_overall > 0 {
            displayable.percentage_overall = (current_overall as f32 / total_overall as f32).min(1.0);
        } else if matches!(core_progress.stage, CoreSyncStage::Completed {..}) {
            displayable.percentage_overall = 1.0;
        } else {
            displayable.percentage_overall = 0.0; // Avoid division by zero, or if total is not yet known
        }

        displayable
    }
}

/// Result of a branch switching operation with sync details
#[derive(Debug, Clone)]
pub struct BranchSyncResult {
    pub success: bool,
    pub previous_branch: String,
    pub new_branch: String,
    pub sync_type: String,
    pub files_processed: usize,
    pub error_message: Option<String>,
}

/// Result of branch listing operation
#[derive(Debug, Clone)]
pub struct BranchOperationResult {
    pub repo_name: String,
    pub success: bool,
    pub branches: Vec<String>,
    pub current_branch: Option<String>,
    pub error_message: Option<String>,
}

/// Result of tag listing operation
#[derive(Debug, Clone)]
pub struct TagOperationResult {
    pub repo_name: String,
    pub success: bool,
    pub tags: Vec<String>,
    pub error_message: Option<String>,
}

/// Result of switch operation
#[derive(Debug, Clone)]
pub struct SwitchOperationResult {
    pub repo_name: String,
    pub target_ref: String,
    pub success: bool,
    pub sync_result: Option<BranchSyncResult>,
    pub error_message: Option<String>,
}

/// Result of create branch operation
#[derive(Debug, Clone)]
pub struct CreateBranchResult {
    pub repo_name: String,
    pub branch_name: String,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Result of delete branch operation
#[derive(Debug, Clone)]
pub struct DeleteBranchResult {
    pub repo_name: String,
    pub branch_name: String,
    pub success: bool,
    pub error_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_panel_tab_enum() {
        // Test all tab variants exist
        let tabs = [
            RepoPanelTab::List,
            RepoPanelTab::Query,
            RepoPanelTab::SearchFile,
            RepoPanelTab::ViewFile,
        ];
        
        for tab in &tabs {
            // Should be debug printable
            format!("{:?}", tab);
        }
        
        // Test equality
        assert_eq!(RepoPanelTab::List, RepoPanelTab::List);
        assert_ne!(RepoPanelTab::List, RepoPanelTab::Query);
    }

    #[test]
    fn test_repository_info_creation() {
        let repo = RepoInfo {
            name: "test-repo".to_string(),
            remote: Some("https://github.com/test/repo.git".to_string()),
            branch: Some("main".to_string()),
            local_path: Some("/path/to/repo".into()),
            is_syncing: false,
        };
        
        assert_eq!(repo.name, "test-repo");
        assert_eq!(repo.remote, Some("https://github.com/test/repo.git".to_string()));
        assert_eq!(repo.branch, Some("main".to_string()));
        assert!(!repo.is_syncing);
    }

    #[test]
    fn test_repository_info_without_branch() {
        let repo = RepoInfo {
            name: "no-branch-repo".to_string(),
            remote: None,
            branch: None,
            local_path: Some("/path/to/repo".into()),
            is_syncing: false,
        };
        
        assert_eq!(repo.name, "no-branch-repo");
        assert!(repo.branch.is_none());
        assert!(repo.remote.is_none());
        assert!(!repo.is_syncing);
    }

    #[test]
    fn test_query_options_default() {
        let options = QueryOptions {
            repo_name: String::new(),
            query_text: String::new(),
            element_type: None,
            language: None,
            limit: 10,
        };
        
        assert!(options.repo_name.is_empty());
        assert!(options.query_text.is_empty());
        assert!(options.element_type.is_none());
        assert!(options.language.is_none());
        assert_eq!(options.limit, 10);
    }

    #[test]
    fn test_query_options_with_filters() {
        let options = QueryOptions {
            repo_name: "my-repo".to_string(),
            query_text: "function".to_string(),
            element_type: Some("function".to_string()),
            language: Some("rust".to_string()),
            limit: 50,
        };
        
        assert_eq!(options.repo_name, "my-repo");
        assert_eq!(options.query_text, "function");
        assert_eq!(options.element_type, Some("function".to_string()));
        assert_eq!(options.language, Some("rust".to_string()));
        assert_eq!(options.limit, 50);
    }

    #[test]
    fn test_query_result_item() {
        let item = QueryResultItem {
            score: 0.95,
            path: "src/main.rs".to_string(),
            start_line: 10,
            end_line: 25,
            content: "fn main() {\n    println!(\"Hello\");\n}".to_string(),
        };
        
        assert_eq!(item.score, 0.95);
        assert_eq!(item.path, "src/main.rs");
        assert_eq!(item.start_line, 10);
        assert_eq!(item.end_line, 25);
        assert!(item.content.contains("fn main()"));
    }

    #[test]
    fn test_query_result_initial_state() {
        let result = QueryResult {
            is_loading: false,
            success: false,
            error_message: None,
            results: Vec::new(),
            channel: None,
        };
        
        assert!(!result.is_loading);
        assert!(!result.success);
        assert!(result.error_message.is_none());
        assert!(result.results.is_empty());
        assert!(result.channel.is_none());
    }

    #[test]
    fn test_query_result_with_error() {
        let result = QueryResult {
            is_loading: false,
            success: false,
            error_message: Some("Connection failed".to_string()),
            results: Vec::new(),
            channel: None,
        };
        
        assert!(!result.is_loading);
        assert!(!result.success);
        assert_eq!(result.error_message, Some("Connection failed".to_string()));
        assert!(result.results.is_empty());
    }

    #[test]
    fn test_query_result_with_results() {
        let items = vec![
            QueryResultItem {
                score: 0.95,
                path: "file1.rs".to_string(),
                start_line: 1,
                end_line: 10,
                content: "content1".to_string(),
            },
            QueryResultItem {
                score: 0.85,
                path: "file2.rs".to_string(),
                start_line: 20,
                end_line: 30,
                content: "content2".to_string(),
            },
        ];
        
        let result = QueryResult {
            is_loading: false,
            success: true,
            error_message: None,
            results: items,
            channel: None,
        };
        
        assert!(!result.is_loading);
        assert!(result.success);
        assert!(result.error_message.is_none());
        assert_eq!(result.results.len(), 2);
        assert_eq!(result.results[0].score, 0.95);
        assert_eq!(result.results[1].path, "file2.rs");
    }

    #[test]
    fn test_file_search_options() {
        let options = FileSearchOptions {
            repo_name: "test-repo".to_string(),
            pattern: "*.rs".to_string(),
            case_sensitive: true,
        };
        
        assert_eq!(options.repo_name, "test-repo");
        assert_eq!(options.pattern, "*.rs");
        assert!(options.case_sensitive);
    }

    #[test]
    fn test_file_search_result_empty() {
        let result = FileSearchResult {
            is_loading: false,
            error_message: None,
            files: Vec::new(),
            channel: None,
        };
        
        assert!(!result.is_loading);
        assert!(result.error_message.is_none());
        assert!(result.files.is_empty());
    }

    #[test]
    fn test_file_search_result_with_files() {
        let files = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "tests/test.rs".to_string(),
        ];
        
        let result = FileSearchResult {
            is_loading: false,
            error_message: None,
            files,
            channel: None,
        };
        
        assert!(!result.is_loading);
        assert!(result.error_message.is_none());
        assert_eq!(result.files.len(), 3);
        assert!(result.files.contains(&"src/main.rs".to_string()));
    }

    #[test]
    fn test_file_view_options() {
        let options = FileViewOptions {
            repo_name: "my-repo".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: Some(10),
            end_line: Some(20),
        };
        
        assert_eq!(options.repo_name, "my-repo");
        assert_eq!(options.file_path, "src/main.rs");
        assert_eq!(options.start_line, Some(10));
        assert_eq!(options.end_line, Some(20));
    }

    #[test]
    fn test_file_view_options_full_file() {
        let options = FileViewOptions {
            repo_name: "my-repo".to_string(),
            file_path: "src/main.rs".to_string(),
            start_line: None,
            end_line: None,
        };
        
        assert_eq!(options.repo_name, "my-repo");
        assert_eq!(options.file_path, "src/main.rs");
        assert!(options.start_line.is_none());
        assert!(options.end_line.is_none());
    }

    #[test]
    fn test_file_view_result() {
        let result = FileViewResult {
            is_loading: false,
            error_message: None,
            content: "fn main() {\n    println!(\"Hello\");\n}".to_string(),
            channel: None,
        };
        
        assert!(!result.is_loading);
        assert!(result.error_message.is_none());
        assert!(result.content.contains("fn main()"));
    }

    #[test]
    fn test_repo_panel_state_initial() {
        let state = RepoPanelState::default();
        
        assert_eq!(state.active_tab, RepoPanelTab::List);
        assert!(state.repositories.is_empty());
        assert!(state.selected_repo.is_none());
        assert!(state.query_options.repo_name.is_empty());
        assert!(!state.is_loading_repos);
    }

    #[test]
    fn test_repository_selection_consistency() {
        let mut state = RepoPanelState {
            selected_repo: Some("repo1".to_string()),
            query_options: QueryOptions {
                repo_name: "repo1".to_string(),
                ..Default::default()
            },
            file_search_options: FileSearchOptions {
                repo_name: "repo1".to_string(),
                ..Default::default()
            },
            file_view_options: FileViewOptions {
                repo_name: "repo1".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        
        // Test that all repo names are consistent
        assert_eq!(state.selected_repo, Some("repo1".to_string()));
        assert_eq!(state.query_options.repo_name, "repo1");
        assert_eq!(state.file_search_options.repo_name, "repo1");
        assert_eq!(state.file_view_options.repo_name, "repo1");
        
        // Test updating selected repo should sync all others
        state.selected_repo = Some("repo2".to_string());
        // In the actual UI code, this would trigger syncing:
        if let Some(selected_repo) = &state.selected_repo {
            if state.query_options.repo_name != *selected_repo {
                state.query_options.repo_name = selected_repo.clone();
            }
            if state.file_search_options.repo_name != *selected_repo {
                state.file_search_options.repo_name = selected_repo.clone();
            }
            if state.file_view_options.repo_name != *selected_repo {
                state.file_view_options.repo_name = selected_repo.clone();
            }
        }
        
        assert_eq!(state.query_options.repo_name, "repo2");
        assert_eq!(state.file_search_options.repo_name, "repo2");
        assert_eq!(state.file_view_options.repo_name, "repo2");
    }

    #[test]
    fn test_query_limit_bounds() {
        let mut options = QueryOptions {
            repo_name: String::new(),
            query_text: String::new(),
            element_type: None,
            language: None,
            limit: 50,
        };
        
        // Test valid range
        assert!(options.limit >= 1);
        assert!(options.limit <= 100);
        
        // Test boundary values
        options.limit = 1;
        assert_eq!(options.limit, 1);
        
        options.limit = 100;
        assert_eq!(options.limit, 100);
    }

    #[test]
    fn test_score_range() {
        let item = QueryResultItem {
            score: 0.85,
            path: "test.rs".to_string(),
            start_line: 1,
            end_line: 10,
            content: "test".to_string(),
        };
        
        // Scores should typically be between 0.0 and 1.0
        assert!(item.score >= 0.0);
        assert!(item.score <= 1.0);
    }

    #[test]
    fn test_line_numbers_validity() {
        let item = QueryResultItem {
            score: 0.95,
            path: "test.rs".to_string(),
            start_line: 10,
            end_line: 25,
            content: "test content".to_string(),
        };
        
        // End line should be >= start line
        assert!(item.end_line >= item.start_line);
        
        // Line numbers should be positive
        assert!(item.start_line > 0);
        assert!(item.end_line > 0);
    }
} 