use anyhow::Result;
use egui::Color32;
use std::time::{Duration, Instant};
use std::sync::Arc;

use crate::agent::conversation::types::ProjectType;
use crate::agent::state::types::ConversationStatus;
use crate::config::{SagittaCodeConfig, SidebarPersistentConfig};
use super::types::{ConversationSidebar, OrganizationMode, SidebarFilters};

impl ConversationSidebar {
    /// Get display name for organization mode
    pub fn organization_mode_display_name(&self) -> &str {
        match self.organization_mode {
            OrganizationMode::Recency => "📅 Recency",
            OrganizationMode::Project => "📁 Project", 
            OrganizationMode::Status => "📊 Status",
            OrganizationMode::Clusters => "🔗 Clusters",
            OrganizationMode::Tags => "🏷️ Tags",
            OrganizationMode::Success => "✅ Success",
            OrganizationMode::Custom(ref name) => name,
        }
    }
    
    /// Load persistent state from configuration
    pub fn load_persistent_state(&mut self, config: &SidebarPersistentConfig) {
        // Load organization mode
        self.organization_mode = match config.last_organization_mode.as_str() {
            "Recency" => OrganizationMode::Recency,
            "Project" => OrganizationMode::Project,
            "Status" => OrganizationMode::Status,
            "Clusters" => OrganizationMode::Clusters,
            "Tags" => OrganizationMode::Tags,
            "Success" => OrganizationMode::Success,
            custom => OrganizationMode::Custom(custom.to_string()),
        };
        
        // Load expanded groups
        self.expanded_groups = config.expanded_groups.iter().cloned().collect();
        
        // Load search query and initialize input buffer
        self.search_query = config.last_search_query.clone();
        self.search_input = self.search_query.clone().unwrap_or_default();
        
        // Load filter settings
        self.filters = SidebarFilters {
            project_types: config.filters.project_types.iter()
                .filter_map(|pt_str| match pt_str.as_str() {
                    "Unknown" => Some(ProjectType::Unknown),
                    "Rust" => Some(ProjectType::Rust),
                    "Python" => Some(ProjectType::Python),
                    "JavaScript" => Some(ProjectType::JavaScript),
                    "TypeScript" => Some(ProjectType::TypeScript),
                    "Go" => Some(ProjectType::Go),
                    "Ruby" => Some(ProjectType::Ruby),
                    "Markdown" => Some(ProjectType::Markdown),
                    "Yaml" => Some(ProjectType::Yaml),
                    "Html" => Some(ProjectType::Html),
                    _ => None,
                })
                .collect(),
            statuses: config.filters.statuses.iter()
                .filter_map(|s| match s.as_str() {
                    "Active" => Some(ConversationStatus::Active),
                    "Paused" => Some(ConversationStatus::Paused),
                    "Completed" => Some(ConversationStatus::Completed),
                    "Archived" => Some(ConversationStatus::Archived),
                    "Summarizing" => Some(ConversationStatus::Summarizing),
                    _ => None,
                })
                .collect(),
            tags: config.filters.tags.clone(),
            date_range: None, // Date ranges are not persisted for now
            min_messages: config.filters.min_messages,
            min_success_rate: config.filters.min_success_rate,
            favorites_only: config.filters.favorites_only,
            branches_only: config.filters.branches_only,
            checkpoints_only: config.filters.checkpoints_only,
        };
        
        // Load UI state
        self.show_filters = config.show_filters;
        self.show_branch_suggestions = config.show_branch_suggestions;
        self.show_checkpoint_suggestions = config.show_checkpoint_suggestions;
        
        // Load accessibility settings
        self.accessibility_enabled = config.enable_accessibility;
        self.color_blind_friendly = config.color_blind_friendly;
    }
    
    /// Save persistent state to configuration
    pub fn save_persistent_state(&mut self, app_config: &mut SagittaCodeConfig) -> Result<()> {
        let config = &mut app_config.conversation.sidebar;
        
        // Save organization mode
        config.last_organization_mode = match &self.organization_mode {
            OrganizationMode::Recency => "Recency".to_string(),
            OrganizationMode::Project => "Project".to_string(),
            OrganizationMode::Status => "Status".to_string(),
            OrganizationMode::Clusters => "Clusters".to_string(),
            OrganizationMode::Tags => "Tags".to_string(),
            OrganizationMode::Success => "Success".to_string(),
            OrganizationMode::Custom(name) => name.clone(),
        };
        
        // Save expanded groups
        config.expanded_groups = self.expanded_groups.iter().cloned().collect();
        
        // Save search query
        config.last_search_query = self.search_query.clone();
        
        // Save filter settings
        config.filters.project_types = self.filters.project_types.iter()
            .map(|pt| match pt {
                ProjectType::Unknown => "Unknown".to_string(),
                ProjectType::Rust => "Rust".to_string(),
                ProjectType::Python => "Python".to_string(),
                ProjectType::JavaScript => "JavaScript".to_string(),
                ProjectType::TypeScript => "TypeScript".to_string(),
                ProjectType::Go => "Go".to_string(),
                ProjectType::Ruby => "Ruby".to_string(),
                ProjectType::Markdown => "Markdown".to_string(),
                ProjectType::Yaml => "Yaml".to_string(),
                ProjectType::Html => "Html".to_string(),
            })
            .collect();
            
        config.filters.statuses = self.filters.statuses.iter()
            .map(|status| match status {
                ConversationStatus::Active => "Active".to_string(),
                ConversationStatus::Paused => "Paused".to_string(),
                ConversationStatus::Completed => "Completed".to_string(),
                ConversationStatus::Archived => "Archived".to_string(),
                ConversationStatus::Summarizing => "Summarizing".to_string(),
            })
            .collect();
            
        config.filters.tags = self.filters.tags.clone();
        config.filters.min_messages = self.filters.min_messages;
        config.filters.min_success_rate = self.filters.min_success_rate;
        config.filters.favorites_only = self.filters.favorites_only;
        config.filters.branches_only = self.filters.branches_only;
        config.filters.checkpoints_only = self.filters.checkpoints_only;
        
        // Save UI state
        config.show_filters = self.show_filters;
        config.show_branch_suggestions = self.show_branch_suggestions;
        config.show_checkpoint_suggestions = self.show_checkpoint_suggestions;
        
        // Save accessibility settings
        config.enable_accessibility = self.accessibility_enabled;
        config.color_blind_friendly = self.color_blind_friendly;
        
        // Save configuration to disk - respect test isolation
        crate::config::save_config(app_config)?;
        self.last_state_save = Some(Instant::now());
        
        Ok(())
    }
    
    /// Auto-save state if enough time has passed
    pub fn auto_save_state(&mut self, config: Arc<tokio::sync::Mutex<SagittaCodeConfig>>) {
        let should_save = match self.last_state_save {
            Some(last_save) => last_save.elapsed() > Duration::from_secs(30), // Auto-save every 30 seconds
            None => true, // First save
        };

        if should_save && self.config.persist_state {
            match config.try_lock() {
                Ok(mut config_guard) => {
                    if let Err(e) = self.save_persistent_state(&mut config_guard) {
                        log::error!("Failed to auto-save sidebar state: {}", e);
                    } else {
                        self.last_state_save = Some(Instant::now());
                    }
                },
                Err(_) => {
                    log::warn!("Failed to acquire config lock for auto-save");
                }
            }
        }
    }
    
    /// Get color-blind friendly color palette
    pub fn get_accessible_color(&self, base_color: Color32, color_type: &str) -> Color32 {
        if !self.color_blind_friendly {
            return base_color;
        }
        
        // Color-blind friendly palette (Viridis-inspired)
        match color_type {
            "success" => Color32::from_rgb(68, 1, 84),      // Dark purple
            "warning" => Color32::from_rgb(253, 231, 37),   // Bright yellow
            "error" => Color32::from_rgb(94, 201, 98),      // Green (counter-intuitive but accessible)
            "info" => Color32::from_rgb(33, 145, 140),      // Teal
            "primary" => Color32::from_rgb(59, 82, 139),    // Blue
            "secondary" => Color32::from_rgb(180, 180, 180), // Gray
            _ => base_color,
        }
    }
    
    /// Add screen reader announcement
    pub fn announce_to_screen_reader(&mut self, message: String) {
        if !self.accessibility_enabled {
            return;
        }
        
        // Limit announcements to prevent spam
        let now = Instant::now();
        if let Some(last_time) = self.last_announcement_time {
            if now.duration_since(last_time) < Duration::from_millis(500) {
                return;
            }
        }
        
        self.screen_reader_announcements.push(message);
        self.last_announcement_time = Some(now);
        
        // Keep only the last 5 announcements
        if self.screen_reader_announcements.len() > 5 {
            self.screen_reader_announcements.remove(0);
        }
    }
    
    /// Check if search should be debounced
    pub fn should_debounce_search(&mut self, query: &str, debounce_ms: u64) -> bool {
        let now = Instant::now();
        
        // If query changed, reset timer
        if self.last_search_query.as_ref() != Some(&query.to_string()) {
            self.search_debounce_timer = Some(now);
            self.last_search_query = Some(query.to_string());
            return true; // Debounce new query
        }
        
        // Check if enough time has passed
        if let Some(timer) = self.search_debounce_timer {
            if now.duration_since(timer) >= Duration::from_millis(debounce_ms) {
                self.search_debounce_timer = None;
                return false; // Don't debounce, execute search
            }
        }
        
        true // Still debouncing
    }
    
    /// Get virtual scrolling range for performance
    pub fn get_virtual_scroll_range(&self, total_items: usize, max_rendered: usize) -> (usize, usize) {
        let start = self.virtual_scroll_offset;
        let end = (start + max_rendered).min(total_items);
        (start, end)
    }
    
    /// Update virtual scroll offset
    pub fn update_virtual_scroll_offset(&mut self, new_offset: usize, total_items: usize, max_rendered: usize) {
        let max_offset = total_items.saturating_sub(max_rendered);
        self.virtual_scroll_offset = new_offset.min(max_offset);
    }
} 