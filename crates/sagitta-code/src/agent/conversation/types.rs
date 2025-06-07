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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash, Copy)] // Added Copy
pub enum ProjectType {
    #[default]
    Unknown,
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    Cpp,
}

impl ProjectType {
    pub fn detect_from_path(path: &std::path::Path) -> Self {
        // Check for Rust project markers
        if path.join("Cargo.toml").exists() {
            return ProjectType::Rust;
        }
        
        // Check for Python project markers
        if path.join("pyproject.toml").exists() || 
           path.join("setup.py").exists() || 
           path.join("requirements.txt").exists() ||
           path.join("Pipfile").exists() {
            return ProjectType::Python;
        }
        
        // Check for JavaScript/Node.js project markers
        if path.join("package.json").exists() {
            return ProjectType::JavaScript;
        }
        
        // Check for TypeScript project markers
        if path.join("tsconfig.json").exists() {
            return ProjectType::TypeScript;
        }
        
        // Check for Go project markers
        if path.join("go.mod").exists() {
            return ProjectType::Go;
        }
        
        // Check for Java project markers
        if path.join("pom.xml").exists() {
            return ProjectType::Java;
        }
        
        // Check for C# project markers
        if path.join("project.json").exists() {
            return ProjectType::CSharp;
        }
        
        // Check for C++ project markers
        if path.join("CMakeLists.txt").exists() {
            return ProjectType::Cpp;
        }
        
        // Default to Unknown if no markers found
        ProjectType::Unknown
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