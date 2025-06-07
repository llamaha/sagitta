// Tools panel UI will go here

use std::sync::Arc;
use std::path::PathBuf;
use egui::{Ui, RichText, Color32, Grid, TextEdit, Button, Checkbox, Layout, Align, Spinner, ComboBox, Frame, Stroke};
use tokio::sync::Mutex;
use rfd::FileDialog;

use crate::gui::repository::manager::RepositoryManager;
use crate::gui::theme::AppTheme;
use crate::agent::Agent;
use crate::config::types::SagittaCodeConfig;

/// Project creation form state
#[derive(Debug, Clone)]
pub struct ProjectCreationForm {
    pub name: String,
    pub language: String,
    pub framework: Option<String>,
    pub path: String,
    pub description: String,
    pub additional_requirements: String,
    pub initialize_git: bool,
    pub run_setup: bool,
    pub use_llm_scaffolding: bool,
    pub creating: bool,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
}

impl Default for ProjectCreationForm {
    fn default() -> Self {
        Self {
            name: String::new(),
            language: "rust".to_string(),
            framework: None,
            path: String::new(),
            description: String::new(),
            additional_requirements: String::new(),
            initialize_git: true,
            run_setup: true,
            use_llm_scaffolding: true,
            creating: false,
            status_message: None,
            error_message: None,
        }
    }
}

/// Project creation panel state
#[derive(Debug)]
pub struct ProjectCreationPanelState {
    pub form: ProjectCreationForm,
    pub available_languages: Vec<String>,
    pub framework_suggestions: std::collections::HashMap<String, Vec<String>>,
}

impl Default for ProjectCreationPanelState {
    fn default() -> Self {
        let mut framework_suggestions = std::collections::HashMap::new();
        framework_suggestions.insert("rust".to_string(), vec![
            "CLI".to_string(),
            "Web API (Axum)".to_string(),
            "Web API (Actix)".to_string(),
            "Desktop GUI (Tauri)".to_string(),
            "Library".to_string(),
        ]);
        framework_suggestions.insert("python".to_string(), vec![
            "CLI (Click)".to_string(),
            "Web API (FastAPI)".to_string(),
            "Web API (Flask)".to_string(),
            "Data Science".to_string(),
            "Django Web App".to_string(),
            "Library".to_string(),
        ]);
        framework_suggestions.insert("typescript".to_string(), vec![
            "Node.js CLI".to_string(),
            "Express API".to_string(),
            "React App".to_string(),
            "Next.js App".to_string(),
            "Library (NPM)".to_string(),
        ]);
        framework_suggestions.insert("javascript".to_string(), vec![
            "Node.js CLI".to_string(),
            "Express API".to_string(),
            "React App".to_string(),
            "Vue.js App".to_string(),
            "Library (NPM)".to_string(),
        ]);
        framework_suggestions.insert("go".to_string(), vec![
            "CLI".to_string(),
            "Web API (Gin)".to_string(),
            "Microservice".to_string(),
            "Library".to_string(),
        ]);
        framework_suggestions.insert("java".to_string(), vec![
            "Spring Boot API".to_string(),
            "CLI Application".to_string(),
            "Library".to_string(),
        ]);

        Self {
            form: ProjectCreationForm::default(),
            available_languages: vec![
                "rust".to_string(),
                "python".to_string(),
                "typescript".to_string(),
                "javascript".to_string(),
                "go".to_string(),
                "java".to_string(),
                "cpp".to_string(),
                "c".to_string(),
                "php".to_string(),
                "ruby".to_string(),
                "swift".to_string(),
                "kotlin".to_string(),
            ],
            framework_suggestions,
        }
    }
}

/// Render the project creation panel
pub fn render_project_creation_panel(
    ui: &mut Ui,
    state: &mut ProjectCreationPanelState,
    config: &SagittaCodeConfig,
    agent: Option<&Arc<Agent>>,
    theme: AppTheme,
) {
    // Initialize default path from repositories base path if empty
    if state.form.path.is_empty() {
        if let Some(base_path) = &config.sagitta.repositories_base_path {
            state.form.path = base_path.to_string_lossy().to_string();
        } else {
            state.form.path = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("projects")
                .to_string_lossy()
                .to_string();
        }
    }

    ui.heading("🆕 Create New Project");
    ui.add_space(8.0);

    // Intelligent suggestions banner
    if !state.form.name.is_empty() && agent.is_some() {
        Frame::none()
            .fill(theme.info_background())
            .stroke(Stroke::new(1.0, theme.info_color()))
            .rounding(4.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("💡").size(14.0));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Smart Suggestions:").strong().color(theme.info_color()));
                        ui.label(format!("• Project will be created at: {}/{}", state.form.path, state.form.name));
                        if let Some(frameworks) = state.framework_suggestions.get(&state.form.language) {
                            if !frameworks.is_empty() {
                                ui.label(format!("• Popular {} frameworks: {}", state.form.language, frameworks.join(", ")));
                            }
                        }
                        if state.form.use_llm_scaffolding {
                            ui.label("• AI will generate custom project structure based on your requirements");
                        } else {
                            ui.label("• Using minimal template for fast, reliable setup");
                        }
                    });
                });
            });
        ui.add_space(8.0);
    }

    // Main form in a nice frame
    Frame::none()
        .fill(theme.panel_background())
        .stroke(Stroke::new(1.0, theme.border_color()))
        .rounding(6.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            Grid::new("project_creation_form")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    // Project Name
                    ui.label(RichText::new("Project Name:").strong());
                    ui.add_enabled(!state.form.creating, TextEdit::singleline(&mut state.form.name)
                        .hint_text("my-awesome-project"));
                    ui.end_row();

                    // Language Selection
                    ui.label(RichText::new("Language:").strong());
                    ui.horizontal(|ui| {
                        ComboBox::from_id_source("language_combo")
                            .selected_text(&state.form.language)
                            .show_ui(ui, |ui| {
                                for lang in &state.available_languages {
                                    ui.selectable_value(&mut state.form.language, lang.clone(), lang);
                                }
                            });
                        
                        // Language-specific icon
                        let icon = match state.form.language.as_str() {
                            "rust" => "🦀",
                            "python" => "🐍",
                            "typescript" | "javascript" => "📜",
                            "go" => "🐹",
                            "java" => "☕",
                            _ => "💻",
                        };
                        ui.label(RichText::new(icon).size(16.0));
                    });
                    ui.end_row();

                    // Framework Selection (context-aware)
                    ui.label(RichText::new("Framework:").strong());
                    ui.horizontal(|ui| {
                        let framework_display = state.form.framework.as_ref().unwrap_or(&"None".to_string()).clone();
                        ComboBox::from_id_source("framework_combo")
                            .selected_text(&framework_display)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut state.form.framework, None, "None");
                                if let Some(frameworks) = state.framework_suggestions.get(&state.form.language) {
                                    for framework in frameworks {
                                        ui.selectable_value(&mut state.form.framework, Some(framework.clone()), framework);
                                    }
                                }
                            });
                    });
                    ui.end_row();

                    // Project Location with intelligent default
                    ui.label(RichText::new("Location:").strong());
                    ui.horizontal(|ui| {
                        ui.add_enabled(!state.form.creating, TextEdit::singleline(&mut state.form.path)
                            .hint_text("Path where project will be created"));
                        
                        if ui.add_enabled(!state.form.creating, Button::new("📁 Browse")).clicked() {
                            if let Some(path) = FileDialog::new()
                                .set_title("Select Project Directory")
                                .pick_folder() {
                                state.form.path = path.to_string_lossy().to_string();
                            }
                        }
                        
                        // Reset to default button
                        if ui.add_enabled(!state.form.creating, Button::new("🏠 Default").small())
                            .on_hover_text("Reset to repositories base path")
                            .clicked() {
                            if let Some(base_path) = &config.sagitta.repositories_base_path {
                                state.form.path = base_path.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.end_row();

                    // Description
                    ui.label(RichText::new("Description:").strong());
                    ui.add_enabled(!state.form.creating, TextEdit::multiline(&mut state.form.description)
                        .desired_rows(2)
                        .hint_text("Brief description of what this project does..."));
                    ui.end_row();

                    // Additional Requirements (for AI)
                    if state.form.use_llm_scaffolding {
                        ui.label(RichText::new("AI Requirements:").strong());
                        ui.add_enabled(!state.form.creating, TextEdit::multiline(&mut state.form.additional_requirements)
                            .desired_rows(2)
                            .hint_text("Specific libraries, patterns, or features you want..."));
                        ui.end_row();
                    }
                });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Options section
            ui.label(RichText::new("⚙️ Options:").strong());
            ui.add_space(4.0);
            
            Grid::new("project_options")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.add_enabled(!state.form.creating, Checkbox::new(&mut state.form.initialize_git, ""));
                    ui.label("Initialize Git repository");
                    ui.end_row();

                    ui.add_enabled(!state.form.creating, Checkbox::new(&mut state.form.run_setup, ""));
                    ui.label("Run initial setup commands");
                    ui.end_row();

                    ui.add_enabled(!state.form.creating, Checkbox::new(&mut state.form.use_llm_scaffolding, ""));
                    ui.horizontal(|ui| {
                        ui.label("Use AI scaffolding");
                        ui.label(RichText::new("(recommended)").small().color(theme.hint_text_color()));
                    });
                    ui.end_row();
                });
        });

    ui.add_space(12.0);

    // Status messages
    if let Some(status) = &state.form.status_message {
        ui.horizontal(|ui| {
            ui.label(RichText::new("✅").color(theme.success_color()));
            ui.label(RichText::new(status).color(theme.success_color()));
        });
    }

    if let Some(error) = &state.form.error_message {
        ui.horizontal(|ui| {
            ui.label(RichText::new("❌").color(theme.error_color()));
            ui.label(RichText::new(error).color(theme.error_color()));
        });
    }

    // Action buttons
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            // Show spinner when creating
            if state.form.creating {
                ui.add(Spinner::new());
                ui.label("Creating project...");
            }
            
            let button_text = if state.form.creating {
                "Creating..."
            } else {
                "🚀 Create Project"
            };
            
            let create_enabled = !state.form.creating 
                && !state.form.name.trim().is_empty() 
                && !state.form.path.trim().is_empty();
            
            if ui.add_enabled(create_enabled, Button::new(RichText::new(button_text).size(14.0)))
                .on_hover_text("Create the project with the specified settings")
                .clicked() {
                trigger_project_creation(state, agent);
            }
            
            // Clear/Reset button
            if ui.add_enabled(!state.form.creating, Button::new("🗑️ Clear"))
                .on_hover_text("Clear the form")
                .clicked() {
                clear_form(state, config);
            }
        });
    });
}

/// Trigger the actual project creation
fn trigger_project_creation(state: &mut ProjectCreationPanelState, agent: Option<&Arc<Agent>>) {
    // Clear previous messages
    state.form.error_message = None;
    state.form.status_message = None;
    state.form.creating = true;

    // Validate form
    if state.form.name.trim().is_empty() {
        state.form.error_message = Some("Project name is required".to_string());
        state.form.creating = false;
        return;
    }

    if state.form.path.trim().is_empty() {
        state.form.error_message = Some("Project location is required".to_string());
        state.form.creating = false;
        return;
    }

    // If we have an agent, send a natural language message to create the project
    if let Some(agent) = agent {
        let mut message = format!(
            "Create a new {} project named '{}' in the directory '{}/{}'. Description: {}.",
            state.form.language,
            state.form.name,
            state.form.path,
            state.form.name,
            if state.form.description.is_empty() { 
                "A new project" 
            } else { 
                &state.form.description 
            }
        );

        if let Some(framework) = &state.form.framework {
            message.push_str(&format!(" Use the {} framework.", framework));
        }

        if !state.form.additional_requirements.is_empty() {
            message.push_str(&format!(" Additional requirements: {}", state.form.additional_requirements));
        }

        if !state.form.initialize_git {
            message.push_str(" Do not initialize a git repository.");
        }

        if !state.form.run_setup {
            message.push_str(" Do not run setup commands.");
        }

        if !state.form.use_llm_scaffolding {
            message.push_str(" Use a minimal template approach instead of AI scaffolding.");
        }

        // Send the message to the agent
        let agent_clone = agent.clone();
        let message_clone = message.clone();
        
        tokio::spawn(async move {
            if let Err(e) = agent_clone.process_message_stream(message_clone).await {
                log::error!("Failed to process project creation message: {}", e);
            }
        });

        state.form.status_message = Some("Project creation request sent to agent. Check the chat for progress.".to_string());
        state.form.creating = false;
    } else {
        state.form.error_message = Some("Agent not available. Please try again later.".to_string());
        state.form.creating = false;
    }
}

/// Clear the form and reset to defaults
fn clear_form(state: &mut ProjectCreationPanelState, config: &SagittaCodeConfig) {
    state.form = ProjectCreationForm::default();
    
    // Re-initialize the path with the configured base path
    if let Some(base_path) = &config.sagitta.repositories_base_path {
        state.form.path = base_path.to_string_lossy().to_string();
    } else {
        state.form.path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("projects")
            .to_string_lossy()
            .to_string();
    }
}

