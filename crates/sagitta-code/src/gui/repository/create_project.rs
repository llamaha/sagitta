use std::sync::Arc;
use std::path::PathBuf;
use egui::{Ui, RichText, Grid, TextEdit, Button, Checkbox, ComboBox, Frame, Stroke};
use tokio::sync::Mutex;
use rfd::FileDialog;

use crate::gui::theme::AppTheme;
use crate::agent::Agent;
use crate::config::types::SagittaCodeConfig;
use super::types::RepoPanelState;
use super::manager::RepositoryManager;

/// Language-specific project creation commands and requirements
struct LanguageProjectInfo {
    command_check: &'static str,
    create_command: fn(&str) -> String,
    tool_name: &'static str,
    install_instructions: &'static str,
}

impl LanguageProjectInfo {
    fn get_language_info(language: &str) -> Option<Self> {
        match language {
            "rust" => Some(LanguageProjectInfo {
                command_check: "cargo",
                create_command: |name| format!("cargo new {}", name),
                tool_name: "Cargo",
                install_instructions: "Install Rust from https://rustup.rs/",
            }),
            "python" => Some(LanguageProjectInfo {
                command_check: "python",
                create_command: |name| {
                    format!(
                        "mkdir {} && cd {} && python -m venv venv && echo '# {}' > README.md",
                        name, name, name
                    )
                },
                tool_name: "Python",
                install_instructions: "Install Python from https://python.org/",
            }),
            "javascript" => Some(LanguageProjectInfo {
                command_check: "npm",
                create_command: |name| format!("mkdir {} && cd {} && npm init -y", name, name),
                tool_name: "Node.js/npm",
                install_instructions: "Install Node.js from https://nodejs.org/",
            }),
            "typescript" => Some(LanguageProjectInfo {
                command_check: "npm",
                create_command: |name| format!("mkdir {} && cd {} && npm init -y --typescript", name, name),
                tool_name: "Node.js/npm",
                install_instructions: "Install Node.js from https://nodejs.org/",
            }),
            "go" => Some(LanguageProjectInfo {
                command_check: "go",
                create_command: |name| format!("mkdir {} && cd {} && go mod init {}", name, name, name),
                tool_name: "Go",
                install_instructions: "Install Go from https://golang.org/",
            }),
            "ruby" => Some(LanguageProjectInfo {
                command_check: "bundle",
                create_command: |name| {
                    format!("mkdir {} && cd {} && bundle init", name, name)
                },
                tool_name: "Ruby/Bundler",
                install_instructions: "Install Ruby from https://ruby-lang.org/ and run 'gem install bundler'",
            }),
            _ => None,
        }
    }
}

/// Render the project creation tab
pub fn render_create_project(
    ui: &mut Ui,
    state: &mut RepoPanelState,
    config: &SagittaCodeConfig,
    agent: Option<&Arc<Agent>>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: AppTheme,
) {
    // Initialize form state if needed
    if state.project_form.path.is_empty() {
        if let Some(base_path) = &config.sagitta.repositories_base_path {
            state.project_form.path = base_path.to_string_lossy().to_string();
        } else {
            state.project_form.path = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("projects")
                .to_string_lossy()
                .to_string();
        }
    }

    ui.heading("🆕 Create New Project");
    ui.add_space(8.0);

    // Show intelligent suggestions if we have a project name
    if !state.project_form.name.is_empty() && agent.is_some() {
        Frame::none()
            .fill(theme.info_background())
            .stroke(Stroke::new(1.0, theme.info_color()))
            .rounding(4.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("💡").size(14.0));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Project Info:").strong().color(theme.info_color()));
                        ui.label(format!("• Location: {}/{}", state.project_form.path, state.project_form.name));
                        
                        // Check if the language tool is available
                        if let Some(info) = LanguageProjectInfo::get_language_info(&state.project_form.language) {
                            ui.label(format!("• Will use {} to create project", info.tool_name));
                        }
                    });
                });
            });
        ui.add_space(8.0);
    }

    // Main form
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
                    ui.add_enabled(!state.project_form.creating, 
                        TextEdit::singleline(&mut state.project_form.name)
                            .hint_text("my-awesome-project"));
                    ui.end_row();

                    // Language Selection
                    ui.label(RichText::new("Language:").strong());
                    ui.horizontal(|ui| {
                        let languages = vec![
                            ("rust", "🦀"),
                            ("python", "🐍"),
                            ("javascript", "📜"),
                            ("typescript", "📜"),
                            ("go", "🐹"),
                            ("ruby", "💎"),
                        ];

                        ComboBox::from_id_source("language_combo")
                            .selected_text(&state.project_form.language)
                            .show_ui(ui, |ui| {
                                for (lang, _icon) in &languages {
                                    ui.selectable_value(&mut state.project_form.language, lang.to_string(), *lang);
                                }
                            });
                        
                        // Show language icon
                        let icon = languages.iter()
                            .find(|(l, _)| l == &state.project_form.language.as_str())
                            .map(|(_, i)| *i)
                            .unwrap_or("💻");
                        ui.label(RichText::new(icon).size(16.0));
                    });
                    ui.end_row();

                    // Project Location
                    ui.label(RichText::new("Location:").strong());
                    ui.horizontal(|ui| {
                        ui.add_enabled(!state.project_form.creating, 
                            TextEdit::singleline(&mut state.project_form.path)
                                .hint_text("Path where project will be created"));
                        
                        if ui.add_enabled(!state.project_form.creating, Button::new("📁 Browse")).clicked() {
                            if let Some(path) = FileDialog::new()
                                .set_title("Select Project Directory")
                                .pick_folder() {
                                state.project_form.path = path.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.end_row();

                    // Description
                    ui.label(RichText::new("Description:").strong());
                    ui.add_enabled(!state.project_form.creating, 
                        TextEdit::multiline(&mut state.project_form.description)
                            .desired_rows(2)
                            .hint_text("Brief description of the project..."));
                    ui.end_row();

                    // Framework (if using AI)
                    if state.project_form.use_ai_scaffolding {
                        ui.label(RichText::new("Framework:").strong());
                        ui.add_enabled(!state.project_form.creating,
                            TextEdit::singleline(&mut state.project_form.framework)
                                .hint_text("e.g., Axum, FastAPI, Express..."));
                        ui.end_row();

                        ui.label(RichText::new("Requirements:").strong());
                        ui.add_enabled(!state.project_form.creating,
                            TextEdit::multiline(&mut state.project_form.additional_requirements)
                                .desired_rows(2)
                                .hint_text("Any specific libraries or features..."));
                        ui.end_row();
                    }
                });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Options
            ui.label(RichText::new("⚙️ Options:").strong());
            ui.add_space(4.0);
            
            Grid::new("project_options")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.add_enabled(!state.project_form.creating, 
                        Checkbox::new(&mut state.project_form.initialize_git, ""));
                    ui.label("Initialize Git repository");
                    ui.end_row();

                    ui.add_enabled(!state.project_form.creating && agent.is_some(), 
                        Checkbox::new(&mut state.project_form.use_ai_scaffolding, ""));
                    ui.horizontal(|ui| {
                        ui.label("Use AI scaffolding");
                        if agent.is_none() {
                            ui.label(RichText::new("(requires agent)").small().color(theme.hint_text_color()));
                        }
                    });
                    ui.end_row();
                });
        });

    ui.add_space(12.0);

    // Status messages
    if let Some(status) = &state.project_form.status_message {
        ui.horizontal(|ui| {
            ui.label(RichText::new("✅").color(theme.success_color()));
            ui.label(RichText::new(status).color(theme.success_color()));
        });
    }

    if let Some(error) = &state.project_form.error_message {
        ui.horizontal(|ui| {
            ui.label(RichText::new("❌").color(theme.error_color()));
            ui.label(RichText::new(error).color(theme.error_color()));
        });
    }

    // Action buttons
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            if state.project_form.creating {
                ui.spinner();
                ui.label("Creating project...");
            }
            
            let create_enabled = !state.project_form.creating 
                && !state.project_form.name.trim().is_empty() 
                && !state.project_form.path.trim().is_empty();
            
            if ui.add_enabled(create_enabled, Button::new(RichText::new("🚀 Create Project").size(14.0)))
                .clicked() {
                // Clear previous messages
                state.project_form.error_message = None;
                state.project_form.status_message = None;
                state.project_form.creating = true;

                // Create the project
                create_project(state, config, agent, repo_manager.clone());
            }
            
            // Clear button
            if ui.add_enabled(!state.project_form.creating, Button::new("🗑️ Clear"))
                .clicked() {
                state.project_form = Default::default();
                // Re-initialize path
                if let Some(base_path) = &config.sagitta.repositories_base_path {
                    state.project_form.path = base_path.to_string_lossy().to_string();
                }
            }
        });
    });
}

/// Create the project
fn create_project(
    state: &mut RepoPanelState,
    config: &SagittaCodeConfig,
    agent: Option<&Arc<Agent>>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
) {
    let project_name = state.project_form.name.clone();
    let project_path = state.project_form.path.clone();
    let language = state.project_form.language.clone();
    let use_ai = state.project_form.use_ai_scaffolding && agent.is_some();
    
    if use_ai {
        // Use AI to create the project
        if let Some(agent) = agent {
            let mut message = format!(
                "Create a new {} project named '{}' in the directory '{}/{}'. Description: {}.",
                language,
                project_name,
                project_path,
                project_name,
                if state.project_form.description.is_empty() { 
                    "A new project" 
                } else { 
                    &state.project_form.description 
                }
            );

            if !state.project_form.framework.is_empty() {
                message.push_str(&format!(" Use the {} framework.", state.project_form.framework));
            }

            if !state.project_form.additional_requirements.is_empty() {
                message.push_str(&format!(" Additional requirements: {}", state.project_form.additional_requirements));
            }

            if !state.project_form.initialize_git {
                message.push_str(" Do not initialize a git repository.");
            }

            // Send to agent
            let agent_clone = agent.clone();
            let message_clone = message.clone();
            let repo_manager_clone = repo_manager.clone();
            let project_full_path = format!("{}/{}", project_path, project_name);
            
            tokio::spawn(async move {
                if let Err(e) = agent_clone.process_message_stream(message_clone).await {
                    log::error!("Failed to process project creation message: {}", e);
                } else {
                    // After successful creation, add the repository
                    let mut manager = repo_manager_clone.lock().await;
                    if let Err(e) = manager.add_repository(&project_name, &project_full_path, None).await {
                        log::error!("Failed to add repository after creation: {}", e);
                    }
                }
            });

            state.project_form.status_message = Some("Project creation request sent to agent. Check the chat for progress.".to_string());
        }
    } else {
        // Use simple command-based creation
        if let Some(info) = LanguageProjectInfo::get_language_info(&language) {
            // Check if the tool is available
            let check_command = if cfg!(windows) {
                format!("where {}", info.command_check)
            } else {
                format!("which {}", info.command_check)
            };

            // Run the check command
            tokio::spawn(async move {
                match tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&check_command)
                    .output()
                    .await
                {
                    Ok(output) if output.status.success() => {
                        // Tool is available, create the project
                        let full_path = format!("{}/{}", project_path, project_name);
                        let create_cmd = (info.create_command)(&project_name);
                        
                        // Change to the project directory and run the command
                        let cd_and_create = format!("cd '{}' && {}", project_path, create_cmd);
                        
                        match tokio::process::Command::new("sh")
                            .arg("-c")
                            .arg(&cd_and_create)
                            .output()
                            .await
                        {
                            Ok(output) if output.status.success() => {
                                log::info!("Project created successfully at {}", full_path);
                                
                                // Add to repository manager
                                let mut manager = repo_manager.lock().await;
                                if let Err(e) = manager.add_repository(&project_name, &full_path, None).await {
                                    log::error!("Failed to add repository after creation: {}", e);
                                }
                            }
                            Ok(output) => {
                                let error = String::from_utf8_lossy(&output.stderr);
                                log::error!("Failed to create project: {}", error);
                            }
                            Err(e) => {
                                log::error!("Failed to execute create command: {}", e);
                            }
                        }
                    }
                    _ => {
                        log::error!("{} is not installed. {}", info.tool_name, info.install_instructions);
                    }
                }
            });

            state.project_form.status_message = Some("Creating project...".to_string());
        } else {
            state.project_form.error_message = Some(format!("Project creation for {} is not yet supported", language));
        }
    }
    
    state.project_form.creating = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::repository::types::{RepoPanelState, ProjectCreationForm};

    #[test]
    fn test_language_project_info() {
        // Test Rust project info
        let rust_info = LanguageProjectInfo::get_language_info("rust").unwrap();
        assert_eq!(rust_info.command_check, "cargo");
        assert_eq!(rust_info.tool_name, "Cargo");
        assert!(rust_info.install_instructions.contains("rustup.rs"));
        let create_cmd = (rust_info.create_command)("test-project");
        assert_eq!(create_cmd, "cargo new test-project");

        // Test Python project info
        let python_info = LanguageProjectInfo::get_language_info("python").unwrap();
        assert_eq!(python_info.command_check, "python");
        assert_eq!(python_info.tool_name, "Python");
        
        // Test JavaScript project info
        let js_info = LanguageProjectInfo::get_language_info("javascript").unwrap();
        assert_eq!(js_info.command_check, "npm");
        assert_eq!(js_info.tool_name, "Node.js/npm");
        let create_cmd = (js_info.create_command)("test-project");
        assert!(create_cmd.contains("npm init"));
        
        // Test TypeScript project info
        let ts_info = LanguageProjectInfo::get_language_info("typescript").unwrap();
        assert_eq!(ts_info.command_check, "npm");
        let create_cmd = (ts_info.create_command)("test-project");
        assert!(create_cmd.contains("npm init") && create_cmd.contains("--typescript"));
        
        // Test Go project info
        let go_info = LanguageProjectInfo::get_language_info("go").unwrap();
        assert_eq!(go_info.command_check, "go");
        assert_eq!(go_info.tool_name, "Go");
        
        // Test unsupported language
        assert!(LanguageProjectInfo::get_language_info("cobol").is_none());
    }

    #[test]
    fn test_project_creation_form_default() {
        let form = ProjectCreationForm::default();
        assert_eq!(form.name, "");
        assert_eq!(form.language, "rust");
        assert_eq!(form.framework, "");
        assert_eq!(form.path, "");
        assert_eq!(form.description, "");
        assert_eq!(form.additional_requirements, "");
        assert!(form.initialize_git);
        assert!(!form.use_ai_scaffolding);
        assert!(!form.creating);
        assert!(form.status_message.is_none());
        assert!(form.error_message.is_none());
    }
}