use anyhow::Result;
use chrono::{DateTime, Utc};
use egui::{Ui, Vec2, TextEdit, Button, ScrollArea, Color32};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::conversation::types::ConversationSummary;
use crate::gui::app::AppState;
use crate::gui::theme::AppTheme;


/// Smart conversation starter with intent detection and context pre-loading
#[derive(Debug, Clone)]
pub struct ConversationStarter {
    /// Current input text
    input_text: String,
    
    /// Detected intent from input
    detected_intent: Option<ConversationIntent>,
    
    /// Available conversation templates
    templates: Vec<ConversationTemplate>,
    
    /// Selected template
    selected_template: Option<usize>,
    
    /// Suggested context items
    suggested_context: Vec<ContextSuggestion>,
    
    /// Selected context items
    selected_context: Vec<Uuid>,
    
    /// Configuration
    config: StarterConfig,
    
    /// Whether the starter is visible
    visible: bool,
    
    /// Last analysis timestamp
    last_analysis: Option<DateTime<Utc>>,
}

/// Configuration for the conversation starter
#[derive(Debug, Clone)]
pub struct StarterConfig {
    /// Enable intent detection
    pub enable_intent_detection: bool,
    
    /// Enable context suggestions
    pub enable_context_suggestions: bool,
    
    /// Enable template suggestions
    pub enable_template_suggestions: bool,
    
    /// Minimum input length for analysis
    pub min_input_length: usize,
    
    /// Analysis delay in milliseconds
    pub analysis_delay_ms: u64,
    
    /// Maximum context suggestions
    pub max_context_suggestions: usize,
}

impl Default for StarterConfig {
    fn default() -> Self {
        Self {
            enable_intent_detection: true,
            enable_context_suggestions: true,
            enable_template_suggestions: true,
            min_input_length: 3,
            analysis_delay_ms: 500,
            max_context_suggestions: 10,
        }
    }
}

/// Detected conversation intent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConversationIntent {
    /// Debugging or troubleshooting
    Debug,
    
    /// Code review or analysis
    CodeReview,
    
    /// Feature development
    FeatureDevelopment,
    
    /// Bug fixing
    BugFix,
    
    /// Documentation
    Documentation,
    
    /// Refactoring
    Refactoring,
    
    /// Testing
    Testing,
    
    /// Research or exploration
    Research,
    
    /// Planning or architecture
    Planning,
    
    /// General question
    Question,
    
    /// Unknown intent
    Unknown,
}

impl ConversationIntent {
    /// Get display name for the intent
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Debug => "Debug & Troubleshoot",
            Self::CodeReview => "Code Review",
            Self::FeatureDevelopment => "Feature Development",
            Self::BugFix => "Bug Fix",
            Self::Documentation => "Documentation",
            Self::Refactoring => "Refactoring",
            Self::Testing => "Testing",
            Self::Research => "Research",
            Self::Planning => "Planning",
            Self::Question => "Question",
            Self::Unknown => "General",
        }
    }
    
    /// Get icon for the intent
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Debug => "🐛",
            Self::CodeReview => "👀",
            Self::FeatureDevelopment => "✨",
            Self::BugFix => "🔧",
            Self::Documentation => "📝",
            Self::Refactoring => "♻️",
            Self::Testing => "🧪",
            Self::Research => "🔍",
            Self::Planning => "📋",
            Self::Question => "❓",
            Self::Unknown => "💬",
        }
    }
    
    /// Get color for the intent
    pub fn color(&self) -> Color32 {
        match self {
            Self::Debug => Color32::from_rgb(255, 100, 100),
            Self::CodeReview => Color32::from_rgb(100, 150, 255),
            Self::FeatureDevelopment => Color32::from_rgb(100, 255, 150),
            Self::BugFix => Color32::from_rgb(255, 150, 100),
            Self::Documentation => Color32::from_rgb(150, 150, 255),
            Self::Refactoring => Color32::from_rgb(255, 255, 100),
            Self::Testing => Color32::from_rgb(150, 255, 150),
            Self::Research => Color32::from_rgb(255, 150, 255),
            Self::Planning => Color32::from_rgb(200, 200, 200),
            Self::Question => Color32::from_rgb(150, 200, 255),
            Self::Unknown => Color32::from_rgb(180, 180, 180),
        }
    }
}

/// Conversation template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTemplate {
    /// Template ID
    pub id: Uuid,
    
    /// Template name
    pub name: String,
    
    /// Template description
    pub description: String,
    
    /// Associated intent
    pub intent: Option<ConversationIntent>,
    
    /// Template content
    pub content: String,
    
    /// Suggested tags
    pub suggested_tags: Vec<String>,
    
    /// Required context types
    pub required_context: Vec<ContextType>,
    
    /// Template category
    pub category: TemplateCategory,
    
    /// Usage count
    pub usage_count: usize,
    
    /// Last used timestamp
    pub last_used: Option<DateTime<Utc>>,
}

/// Template category
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateCategory {
    Development,
    Debugging,
    Review,
    Documentation,
    Planning,
    Custom,
}

impl TemplateCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Development => "Development",
            Self::Debugging => "Debugging",
            Self::Review => "Review",
            Self::Documentation => "Documentation",
            Self::Planning => "Planning",
            Self::Custom => "Custom",
        }
    }
}

/// Context suggestion
#[derive(Debug, Clone)]
pub struct ContextSuggestion {
    /// Suggestion ID
    pub id: Uuid,
    
    /// Context type
    pub context_type: ContextType,
    
    /// Display name
    pub name: String,
    
    /// Description
    pub description: String,
    
    /// Relevance score (0.0 to 1.0)
    pub relevance: f32,
    
    /// Context data
    pub data: ContextData,
}

/// Type of context
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextType {
    /// File or directory
    File,
    
    /// Git repository
    Repository,
    
    /// Previous conversation
    Conversation,
    
    /// Code symbol (function, class, etc.)
    Symbol,
    
    /// Documentation
    Documentation,
    
    /// Issue or task
    Issue,
}

impl ContextType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Repository => "Repository",
            Self::Conversation => "Conversation",
            Self::Symbol => "Symbol",
            Self::Documentation => "Documentation",
            Self::Issue => "Issue",
        }
    }
    
    pub fn icon(&self) -> &'static str {
        match self {
            Self::File => "📄",
            Self::Repository => "📁",
            Self::Conversation => "💬",
            Self::Symbol => "🔤",
            Self::Documentation => "📖",
            Self::Issue => "🎯",
        }
    }
}

/// Context data
#[derive(Debug, Clone)]
pub enum ContextData {
    File { path: String, content_preview: Option<String> },
    Repository { name: String, branch: String },
    Conversation { summary: ConversationSummary },
    Project { name: String, path: String },
    Symbol { name: String, location: String },
    Documentation { title: String, url: Option<String> },
    Issue { title: String, id: String },
}

impl ConversationStarter {
    /// Create a new conversation starter
    pub fn new() -> Self {
        Self {
            input_text: String::new(),
            detected_intent: None,
            templates: Self::create_default_templates(),
            selected_template: None,
            suggested_context: Vec::new(),
            selected_context: Vec::new(),
            config: StarterConfig::default(),
            visible: false,
            last_analysis: None,
        }
    }
    
    /// Create default conversation templates
    fn create_default_templates() -> Vec<ConversationTemplate> {
        vec![
            ConversationTemplate {
                id: Uuid::new_v4(),
                name: "Debug Issue".to_string(),
                description: "Investigate and fix a bug or issue".to_string(),
                intent: Some(ConversationIntent::Debug),
                content: "I'm experiencing an issue with [describe the problem]. Here's what I've observed:\n\n- [Symptom 1]\n- [Symptom 2]\n\nSteps to reproduce:\n1. [Step 1]\n2. [Step 2]\n\nExpected behavior: [What should happen]\nActual behavior: [What actually happens]\n\nCan you help me debug this?".to_string(),
                suggested_tags: vec!["debug".to_string(), "issue".to_string()],
                required_context: vec![ContextType::File, ContextType::Repository],
                category: TemplateCategory::Debugging,
                usage_count: 0,
                last_used: None,
            },
            ConversationTemplate {
                id: Uuid::new_v4(),
                name: "Code Review".to_string(),
                description: "Review code for quality, security, and best practices".to_string(),
                intent: Some(ConversationIntent::CodeReview),
                content: "Please review the following code for:\n\n- Code quality and readability\n- Performance considerations\n- Security vulnerabilities\n- Best practices adherence\n- Potential bugs\n\n[Paste or reference the code to review]".to_string(),
                suggested_tags: vec!["review".to_string(), "quality".to_string()],
                required_context: vec![ContextType::File],
                category: TemplateCategory::Review,
                usage_count: 0,
                last_used: None,
            },
            ConversationTemplate {
                id: Uuid::new_v4(),
                name: "Feature Development".to_string(),
                description: "Plan and implement a new feature".to_string(),
                intent: Some(ConversationIntent::FeatureDevelopment),
                content: "I want to implement a new feature: [Feature name]\n\nRequirements:\n- [Requirement 1]\n- [Requirement 2]\n\nAcceptance criteria:\n- [Criteria 1]\n- [Criteria 2]\n\nCan you help me plan the implementation approach?".to_string(),
                suggested_tags: vec!["feature".to_string(), "development".to_string()],
                required_context: vec![ContextType::Repository],
                category: TemplateCategory::Development,
                usage_count: 0,
                last_used: None,
            },
            ConversationTemplate {
                id: Uuid::new_v4(),
                name: "Refactoring".to_string(),
                description: "Improve code structure and maintainability".to_string(),
                intent: Some(ConversationIntent::Refactoring),
                content: "I want to refactor [code/module/function] to improve:\n\n- Code readability\n- Performance\n- Maintainability\n- Testability\n\nCurrent issues:\n- [Issue 1]\n- [Issue 2]\n\nCan you suggest refactoring approaches?".to_string(),
                suggested_tags: vec!["refactor".to_string(), "improvement".to_string()],
                required_context: vec![ContextType::File, ContextType::Symbol],
                category: TemplateCategory::Development,
                usage_count: 0,
                last_used: None,
            },
            ConversationTemplate {
                id: Uuid::new_v4(),
                name: "Documentation".to_string(),
                description: "Create or improve documentation".to_string(),
                intent: Some(ConversationIntent::Documentation),
                content: "I need help with documentation for [component/feature/API].\n\nTarget audience: [developers/users/maintainers]\n\nTopics to cover:\n- [Topic 1]\n- [Topic 2]\n\nCan you help me create comprehensive documentation?".to_string(),
                suggested_tags: vec!["documentation".to_string(), "writing".to_string()],
                required_context: vec![ContextType::File, ContextType::Symbol],
                category: TemplateCategory::Documentation,
                usage_count: 0,
                last_used: None,
            },
        ]
    }
    
    /// Show the conversation starter
    pub fn show(&mut self) {
        self.visible = true;
    }
    
    /// Hide the conversation starter
    pub fn hide(&mut self) {
        self.visible = false;
        self.reset();
    }
    
    /// Reset the starter state
    pub fn reset(&mut self) {
        self.input_text.clear();
        self.detected_intent = None;
        self.selected_template = None;
        self.suggested_context.clear();
        self.selected_context.clear();
        self.last_analysis = None;
    }
    
    /// Analyze input text for intent detection
    pub fn analyze_input(&mut self, input: &str) -> Result<()> {
        if input.len() < self.config.min_input_length {
            self.detected_intent = None;
            return Ok(());
        }
        
        // Simple keyword-based intent detection
        let input_lower = input.to_lowercase();
        
        self.detected_intent = Some(if input_lower.contains("debug") || input_lower.contains("bug") || input_lower.contains("error") || input_lower.contains("issue") {
            ConversationIntent::Debug
        } else if input_lower.contains("review") || input_lower.contains("check") || input_lower.contains("analyze") {
            ConversationIntent::CodeReview
        } else if input_lower.contains("feature") || input_lower.contains("implement") || input_lower.contains("add") || input_lower.contains("create") {
            ConversationIntent::FeatureDevelopment
        } else if input_lower.contains("fix") || input_lower.contains("repair") || input_lower.contains("solve") {
            ConversationIntent::BugFix
        } else if input_lower.contains("document") || input_lower.contains("explain") || input_lower.contains("describe") {
            ConversationIntent::Documentation
        } else if input_lower.contains("refactor") || input_lower.contains("improve") || input_lower.contains("optimize") {
            ConversationIntent::Refactoring
        } else if input_lower.contains("test") || input_lower.contains("verify") || input_lower.contains("validate") {
            ConversationIntent::Testing
        } else if input_lower.contains("research") || input_lower.contains("explore") || input_lower.contains("investigate") {
            ConversationIntent::Research
        } else if input_lower.contains("plan") || input_lower.contains("design") || input_lower.contains("architecture") {
            ConversationIntent::Planning
        } else if input_lower.contains("?") || input_lower.contains("how") || input_lower.contains("what") || input_lower.contains("why") {
            ConversationIntent::Question
        } else {
            ConversationIntent::Unknown
        });
        
        self.last_analysis = Some(Utc::now());
        Ok(())
    }
    
    /// Generate context suggestions based on current application state
    pub fn generate_context_suggestions(&mut self, app_state: &AppState) -> Result<()> {
        self.suggested_context.clear();
        
        // Add recent conversations as context
        for conv in app_state.conversation_list.iter().take(5) {
            self.suggested_context.push(ContextSuggestion {
                id: Uuid::new_v4(),
                context_type: ContextType::Conversation,
                name: conv.title.clone(),
                description: format!("{} messages, last active {}", conv.message_count, conv.last_active.format("%Y-%m-%d")),
                relevance: 0.7,
                data: ContextData::Conversation { summary: conv.clone() },
            });
        }
        
        
        // Sort by relevance
        self.suggested_context.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
        
        // Limit suggestions
        self.suggested_context.truncate(self.config.max_context_suggestions);
        
        Ok(())
    }
    
    /// Get templates matching the detected intent
    pub fn get_matching_templates(&self) -> Vec<&ConversationTemplate> {
        if let Some(ref intent) = self.detected_intent {
            self.templates.iter()
                .filter(|t| t.intent.as_ref() == Some(intent))
                .collect()
        } else {
            self.templates.iter().collect()
        }
    }
    
    /// Render the conversation starter UI
    pub fn render(&mut self, ui: &mut Ui, app_state: &mut AppState, theme: &AppTheme) -> Result<Option<StarterAction>> {
        if !self.visible {
            return Ok(None);
        }
        
        let mut action = None;
        
        ui.vertical(|ui| {
            // Header
            ui.horizontal(|ui| {
                ui.heading("🚀 Start New Conversation");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✕").clicked() {
                        action = Some(StarterAction::Cancel);
                    }
                });
            });
            
            ui.separator();
            
            // Input area
            ui.vertical(|ui| {
                ui.label("What would you like to work on?");
                
                let response = ui.add(
                    TextEdit::multiline(&mut self.input_text)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text("Describe what you want to accomplish...")
                );
                
                if response.changed() {
                    let input_text = self.input_text.clone();
                    let _ = self.analyze_input(&input_text);
                    let _ = self.generate_context_suggestions(app_state);
                }
            });
            
            ui.separator();
            
            // Intent detection
            if let Some(ref intent) = self.detected_intent {
                ui.horizontal(|ui| {
                    ui.label("Detected intent:");
                    ui.colored_label(intent.color(), format!("{} {}", intent.icon(), intent.display_name()));
                });
                ui.separator();
            }
            
            // Templates section
            if self.config.enable_template_suggestions {
                ui.collapsing("📋 Templates", |ui| {
                    let matching_templates: Vec<_> = self.get_matching_templates().into_iter().cloned().collect();
                    
                    if matching_templates.is_empty() {
                        ui.label("No templates available");
                    } else {
                        for (i, template) in matching_templates.iter().enumerate() {
                            let selected = self.selected_template == Some(i);
                            
                            if ui.selectable_label(selected, &template.name).clicked() {
                                self.selected_template = if selected { None } else { Some(i) };
                                if !selected {
                                    self.input_text = template.content.clone();
                                }
                            }
                            
                            if selected {
                                ui.indent("template_details", |ui| {
                                    ui.label(&template.description);
                                    if !template.suggested_tags.is_empty() {
                                        ui.horizontal(|ui| {
                                            ui.label("Tags:");
                                            for tag in &template.suggested_tags {
                                                ui.small(format!("#{}", tag));
                                            }
                                        });
                                    }
                                });
                            }
                        }
                    }
                });
                ui.separator();
            }
            
            // Context suggestions
            if self.config.enable_context_suggestions && !self.suggested_context.is_empty() {
                ui.collapsing("🔗 Suggested Context", |ui| {
                    ScrollArea::vertical()
                        .max_height(150.0)
                        .show(ui, |ui| {
                            for suggestion in &self.suggested_context {
                                let selected = self.selected_context.contains(&suggestion.id);
                                
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut selected.clone(), "").changed() {
                                        if selected {
                                            self.selected_context.retain(|id| *id != suggestion.id);
                                        } else {
                                            self.selected_context.push(suggestion.id);
                                        }
                                    }
                                    
                                    ui.label(format!("{} {}", suggestion.context_type.icon(), suggestion.name));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.small(format!("{:.0}%", suggestion.relevance * 100.0));
                                    });
                                });
                                
                                if !suggestion.description.is_empty() {
                                    ui.indent("context_desc", |ui| {
                                        ui.small(&suggestion.description);
                                    });
                                }
                            }
                        });
                });
                ui.separator();
            }
            
            // Action buttons
            ui.horizontal(|ui| {
                if ui.add(Button::new("🚀 Start Conversation").min_size(Vec2::new(120.0, 30.0))).clicked() 
                    && !self.input_text.trim().is_empty() {
                    action = Some(StarterAction::StartConversation {
                        title: self.extract_title(),
                        content: self.input_text.clone(),
                        intent: self.detected_intent.clone(),
                        template_id: self.selected_template.and_then(|i| self.get_matching_templates().get(i).map(|t| t.id)),
                        context_ids: self.selected_context.clone(),
                    });
                }
                
                if ui.button("📋 Save as Template").clicked() 
                    && !self.input_text.trim().is_empty() {
                    action = Some(StarterAction::SaveTemplate {
                        name: self.extract_title(),
                        content: self.input_text.clone(),
                        intent: self.detected_intent.clone(),
                    });
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        action = Some(StarterAction::Cancel);
                    }
                });
            });
        });
        
        Ok(action)
    }
    
    /// Extract a title from the input text
    fn extract_title(&self) -> String {
        let lines: Vec<&str> = self.input_text.lines().collect();
        if let Some(first_line) = lines.first() {
            let title = first_line.trim();
            if title.len() > 50 {
                format!("{}...", &title[..47])
            } else if title.is_empty() && lines.len() > 1 {
                let second_line = lines[1].trim();
                if second_line.len() > 50 {
                    format!("{}...", &second_line[..47])
                } else {
                    second_line.to_string()
                }
            } else {
                title.to_string()
            }
        } else {
            "New Conversation".to_string()
        }
    }
}

impl Default for ConversationStarter {
    fn default() -> Self {
        Self::new()
    }
}

/// Actions that can be triggered by the conversation starter
#[derive(Debug, Clone)]
pub enum StarterAction {
    /// Start a new conversation
    StartConversation {
        title: String,
        content: String,
        intent: Option<ConversationIntent>,
        template_id: Option<Uuid>,
        context_ids: Vec<Uuid>,
    },
    
    /// Save current input as a template
    SaveTemplate {
        name: String,
        content: String,
        intent: Option<ConversationIntent>,
    },
    
    /// Cancel the starter
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_starter_creation() {
        let starter = ConversationStarter::new();
        assert!(!starter.visible);
        assert!(starter.input_text.is_empty());
        assert!(starter.detected_intent.is_none());
        assert!(!starter.templates.is_empty());
    }
    
    #[test]
    fn test_intent_detection() {
        let mut starter = ConversationStarter::new();
        
        // Test debug intent
        starter.analyze_input("I have a bug in my code").unwrap();
        assert_eq!(starter.detected_intent, Some(ConversationIntent::Debug));
        
        // Test feature intent
        starter.analyze_input("I want to implement a new feature").unwrap();
        assert_eq!(starter.detected_intent, Some(ConversationIntent::FeatureDevelopment));
        
        // Test review intent
        starter.analyze_input("Please review this code").unwrap();
        assert_eq!(starter.detected_intent, Some(ConversationIntent::CodeReview));
        
        // Test question intent
        starter.analyze_input("How do I do this?").unwrap();
        assert_eq!(starter.detected_intent, Some(ConversationIntent::Question));
    }
    
    #[test]
    fn test_template_matching() {
        let starter = ConversationStarter::new();
        
        // Test with no intent
        let all_templates = starter.get_matching_templates();
        assert_eq!(all_templates.len(), starter.templates.len());
    }
    
    #[test]
    fn test_title_extraction() {
        let mut starter = ConversationStarter::new();
        
        starter.input_text = "Debug the authentication issue\nI'm having trouble with login".to_string();
        assert_eq!(starter.extract_title(), "Debug the authentication issue");
        
        starter.input_text = "This is a very long title that should be truncated because it exceeds the maximum length".to_string();
        assert!(starter.extract_title().ends_with("..."));
        assert!(starter.extract_title().len() <= 50);
        
        starter.input_text = "\nSecond line title".to_string();
        assert_eq!(starter.extract_title(), "Second line title");
    }
    
    #[test]
    fn test_intent_display_properties() {
        let intent = ConversationIntent::Debug;
        assert_eq!(intent.display_name(), "Debug & Troubleshoot");
        assert_eq!(intent.icon(), "🐛");
        assert_ne!(intent.color(), Color32::TRANSPARENT);
    }
    
    #[test]
    fn test_template_categories() {
        let category = TemplateCategory::Development;
        assert_eq!(category.display_name(), "Development");
    }
    
    #[test]
    fn test_context_types() {
        let context_type = ContextType::File;
        assert_eq!(context_type.display_name(), "File");
        assert_eq!(context_type.icon(), "📄");
    }
} 