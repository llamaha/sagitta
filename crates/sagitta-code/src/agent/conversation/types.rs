// ... file removed, all types now imported from sagitta-code-engine ... 

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde_json::Value;
use crate::agent::state::types::ConversationStatus; // Direct import is good
use crate::agent::message::types::AgentMessage; // Corrected path
use std::path::PathBuf; // For PathBuf

// --- Placeholder Types for Conversation System ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Conversation {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub messages: Vec<AgentMessage>, // Used corrected AgentMessage path
    pub status: ConversationStatus, 
    pub workspace_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub branches: Vec<ConversationBranch>,
    pub checkpoints: Vec<ConversationCheckpoint>,
    pub project_context: Option<ProjectContext>,
}

impl Conversation {
    pub fn new(title: String, workspace_id: Option<Uuid>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title,
            created_at: now,
            last_active: now,
            messages: Vec::new(),
            status: ConversationStatus::default(),
            workspace_id,
            tags: Vec::new(),
            branches: Vec::new(),
            checkpoints: Vec::new(),
            project_context: None,
        }
    }
    pub fn to_summary(&self) -> ConversationSummary {
        ConversationSummary {
            id: self.id,
            title: self.title.clone(),
            created_at: self.created_at, 
            last_active: self.last_active,
            message_count: self.messages.len(),
            status: self.status.clone(),
            tags: self.tags.clone(),
            workspace_id: self.workspace_id,
            has_branches: !self.branches.is_empty(),
            has_checkpoints: !self.checkpoints.is_empty(),
            project_name: self.project_context.as_ref().map(|pc| pc.name.clone()),
        }
    }
    pub fn add_message(&mut self, message: AgentMessage) { // Used corrected AgentMessage path
        self.messages.push(message);
        self.last_active = Utc::now();
    }
    pub fn get_active_messages(&self) -> Vec<AgentMessage> {
        self.messages.clone()
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>, 
    pub last_active: DateTime<Utc>,
    pub message_count: usize,
    pub status: ConversationStatus, 
    pub tags: Vec<String>,
    pub workspace_id: Option<Uuid>,
    pub has_branches: bool,
    pub has_checkpoints: bool,
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectContext {
    pub name: String,
    pub project_type: ProjectType,
    pub root_path: Option<PathBuf>, 
    pub description: Option<String>, 
    pub repositories: Vec<String>, 
    pub settings: HashMap<String, Value>,
}

/// Project type enumeration for different programming languages and contexts
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Copy)]
pub enum ProjectType {
    /// Rust programming language projects
    Rust,
    /// Python programming language projects  
    Python,
    /// JavaScript programming language projects
    JavaScript,
    /// TypeScript programming language projects
    TypeScript,
    /// Go programming language projects
    Go,
    /// Ruby programming language projects
    Ruby,
    /// Markdown documentation projects
    Markdown,
    /// YAML configuration projects
    Yaml,
    /// HTML web projects
    Html,
    /// Mixed or unknown project type
    Unknown,
}

impl Default for ProjectType {
    fn default() -> Self {
        ProjectType::Unknown
    }
}

impl ProjectType {
    /// Detect project type from file extension
    pub fn from_extension(extension: &str) -> Self {
        match extension.to_lowercase().as_str() {
            "rs" => ProjectType::Rust,
            "py" => ProjectType::Python,
            "js" | "jsx" => ProjectType::JavaScript,
            "ts" | "tsx" => ProjectType::TypeScript,
            "go" => ProjectType::Go,
            "rb" => ProjectType::Ruby,
            "md" => ProjectType::Markdown,
            "yaml" | "yml" => ProjectType::Yaml,
            "html" => ProjectType::Html,
            _ => ProjectType::Unknown,
        }
    }
    
    /// Detect project type from project name or path
    pub fn from_project_name(name: &str) -> Self {
        let name_lower = name.to_lowercase();
        
        if name_lower.contains("rust") || name_lower.contains("cargo") || name_lower.ends_with(".rs") {
            ProjectType::Rust
        } else if name_lower.contains("python") || name_lower.contains("py") || name_lower.ends_with(".py") {
            ProjectType::Python
        } else if name_lower.contains("javascript") || name_lower.contains("js") || name_lower.contains("node") || name_lower.ends_with(".js") || name_lower.ends_with(".jsx") {
            ProjectType::JavaScript
        } else if name_lower.contains("typescript") || name_lower.contains("ts") || name_lower.ends_with(".ts") || name_lower.ends_with(".tsx") {
            ProjectType::TypeScript
        } else if name_lower.contains("go") || name_lower.ends_with(".go") {
            ProjectType::Go
        } else if name_lower.contains("ruby") || name_lower.contains("rb") || name_lower.ends_with(".rb") {
            ProjectType::Ruby
        } else if name_lower.contains("markdown") || name_lower.contains("md") || name_lower.ends_with(".md") {
            ProjectType::Markdown
        } else if name_lower.contains("yaml") || name_lower.contains("yml") || name_lower.ends_with(".yaml") || name_lower.ends_with(".yml") {
            ProjectType::Yaml
        } else if name_lower.contains("html") || name_lower.ends_with(".html") {
            ProjectType::Html
        } else {
            ProjectType::Unknown
        }
    }
    
    /// Detect project type from directory contents by checking for project files
    pub fn from_directory(path: &std::path::Path) -> Self {
        // Check for project-specific files in order of specificity
        if path.join("Cargo.toml").exists() {
            ProjectType::Rust
        } else if path.join("package.json").exists() {
            // Check if it's TypeScript by looking for tsconfig.json or .ts files
            if path.join("tsconfig.json").exists() {
                ProjectType::TypeScript
            } else {
                // Check for .ts files in common directories
                let ts_dirs = ["src", "lib", "app", "."];
                for dir in &ts_dirs {
                    let dir_path = path.join(dir);
                    if dir_path.exists() {
                        if let Ok(entries) = std::fs::read_dir(&dir_path) {
                            for entry in entries.flatten() {
                                if let Some(name) = entry.file_name().to_str() {
                                    if name.ends_with(".ts") || name.ends_with(".tsx") {
                                        return ProjectType::TypeScript;
                                    }
                                }
                            }
                        }
                    }
                }
                ProjectType::JavaScript
            }
        } else if path.join("requirements.txt").exists() || 
                  path.join("pyproject.toml").exists() || 
                  path.join("setup.py").exists() ||
                  path.join("Pipfile").exists() {
            ProjectType::Python
        } else if path.join("go.mod").exists() {
            ProjectType::Go
        } else if path.join("Gemfile").exists() {
            ProjectType::Ruby
        } else {
            // Check for dominant file types in the directory
            let mut file_counts = std::collections::HashMap::new();
            
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if let Some(ext) = name.split('.').last() {
                            let project_type = Self::from_extension(ext);
                            if project_type != ProjectType::Unknown {
                                *file_counts.entry(project_type).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
            
            // Return the most common project type, or Unknown if none found
            file_counts.into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(project_type, _)| project_type)
                .unwrap_or(ProjectType::Unknown)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BranchStatus {
    #[default]
    Active,
    Merged,
    Archived,
    Successful, 
    Failed,     
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationBranch {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>, 
    pub parent_message_id: Option<Uuid>,
    pub messages: Vec<AgentMessage>, // Used corrected AgentMessage path
    pub created_at: DateTime<Utc>,
    pub status: BranchStatus,
    pub merged: bool,
    pub success_score: Option<f32>,
}

impl ConversationBranch {
    pub fn new(title: String, parent_message_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            description: None,
            parent_message_id,
            messages: Vec::new(),
            created_at: Utc::now(),
            status: BranchStatus::Active,
            merged: false,
            success_score: None,
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationCheckpoint {
    pub id: Uuid,
    pub message_id: Uuid,
    pub title: String,
    pub description: Option<String>, 
    pub created_at: DateTime<Utc>,
    pub context_snapshot: Option<ContextSnapshot>,
    pub auto_generated: bool, 
}

impl ConversationCheckpoint {
    pub fn new(message_id: Uuid, title: String, description: Option<String>, context_snapshot: Option<ContextSnapshot>, auto_generated: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            message_id,
            title,
            description,
            created_at: Utc::now(),
            context_snapshot,
            auto_generated,
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextSnapshot {
    pub file_states: HashMap<PathBuf, String>, // Key is PathBuf
    pub repository_states: HashMap<String, String>, 
    pub environment: HashMap<String, String>,
    pub working_directory: PathBuf,
    pub agent_state: String, 
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationQuery {
    pub text: Option<String>,
    pub status: Option<ConversationStatus>,
    pub project_type: Option<ProjectType>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub date_range: Option<(DateTime<Utc>, DateTime<Utc>)>, 
    pub workspace_id: Option<Uuid>, 
    pub tags: Option<Vec<String>>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationSearchResult {
    pub id: Uuid, 
    pub title: String,
    pub relevance_score: f32,
    pub summary_snippet: Option<String>,
    pub last_active: DateTime<Utc>,
    pub conversation: Option<Box<Conversation>>, // Changed to Box<Conversation> to avoid recursion issues if Conversation contains Vec<ConversationSummary>
    pub matching_snippets: Vec<String>, 
    pub matching_messages: Vec<AgentMessage>, // Used corrected AgentMessage path
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationMetadata {
    pub word_count: u32,
    pub user_feedback_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSettings {
    pub default_project_type: Option<ProjectType>,
    pub max_conversations: Option<usize>,
    pub auto_cleanup_days: Option<u32>,
    pub auto_context_loading: bool,
    pub auto_checkpoints: bool,
    pub auto_branching: bool,
    pub default_tags: Vec<String>,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            default_project_type: None,
            max_conversations: Some(100),
            auto_cleanup_days: Some(30),
            auto_context_loading: true,
            auto_checkpoints: true,
            auto_branching: false,
            default_tags: Vec::new(),
        }
    }
}

// Re-export ConversationStatus from agent::state::types to avoid circular dependency if it were defined here
// pub use crate::agent::state::types::ConversationStatus; 