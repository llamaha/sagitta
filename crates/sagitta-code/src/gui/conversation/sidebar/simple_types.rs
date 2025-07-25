// Simplified sidebar types - just the essentials
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Simple sidebar action
#[derive(Debug, Clone)]
pub enum SimpleSidebarAction {
    SwitchToConversation(Uuid),
    CreateNewConversation,
    DeleteConversation(Uuid),
    RenameConversation(Uuid, String),
    RefreshList,
}

/// Simple conversation display item
#[derive(Debug, Clone)]
pub struct SimpleConversationItem {
    pub id: Uuid,
    pub title: String,
    pub last_active: DateTime<Utc>,
    pub is_selected: bool,
}

/// Simple conversation sidebar
#[derive(Clone)]
pub struct SimpleConversationSidebar {
    /// List of conversations
    pub conversations: Vec<SimpleConversationItem>,
    
    /// Selected conversation ID
    pub selected_conversation: Option<Uuid>,
    
    /// Search query
    pub search_query: String,
    
    /// Currently editing conversation (for rename)
    pub editing_conversation: Option<(Uuid, String)>,
    
    /// Pending action
    pub pending_action: Option<SimpleSidebarAction>,
}

impl SimpleConversationSidebar {
    pub fn new() -> Self {
        Self {
            conversations: Vec::new(),
            selected_conversation: None,
            search_query: String::new(),
            editing_conversation: None,
            pending_action: None,
        }
    }
    
    /// Update the conversation list
    pub fn update_conversations(&mut self, conversations: Vec<(Uuid, String, DateTime<Utc>)>) {
        self.conversations = conversations
            .into_iter()
            .map(|(id, title, last_active)| SimpleConversationItem {
                id,
                title,
                last_active,
                is_selected: self.selected_conversation == Some(id),
            })
            .collect();
    }
    
    /// Filter conversations by search query
    pub fn filtered_conversations(&self) -> Vec<&SimpleConversationItem> {
        if self.search_query.is_empty() {
            self.conversations.iter().collect()
        } else {
            let query = self.search_query.to_lowercase();
            self.conversations
                .iter()
                .filter(|conv| conv.title.to_lowercase().contains(&query))
                .collect()
        }
    }
    
    /// Start editing a conversation title
    pub fn start_editing(&mut self, id: Uuid, current_title: String) {
        self.editing_conversation = Some((id, current_title));
    }
    
    /// Stop editing
    pub fn stop_editing(&mut self) {
        self.editing_conversation = None;
    }
}