use std::sync::Arc;
use egui::{Ui, RichText, Grid, TextEdit, Button, Checkbox, ComboBox, Frame, Stroke, CornerRadius};
use tokio::sync::Mutex;

use crate::gui::theme::AppTheme;
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
                create_command: |name| format!("cargo new {name}"),
                tool_name: "Cargo",
                install_instructions: "Install Rust from https://rustup.rs/",
            }),
            "python" => Some(LanguageProjectInfo {
                command_check: "python",
                create_command: |name| {
                    format!(
                        "mkdir {name} && cd {name} && python -m venv venv && echo '# {name}' > README.md"
                    )
                },
                tool_name: "Python",
                install_instructions: "Install Python from https://python.org/",
            }),
            "javascript" => Some(LanguageProjectInfo {
                command_check: "npm",
                create_command: |name| format!("mkdir {name} && cd {name} && npm init -y"),
                tool_name: "Node.js/npm",
                install_instructions: "Install Node.js from https://nodejs.org/",
            }),
            "typescript" => Some(LanguageProjectInfo {
                command_check: "npm",
                create_command: |name| format!("mkdir {name} && cd {name} && npm init -y --typescript"),
                tool_name: "Node.js/npm",
                install_instructions: "Install Node.js from https://nodejs.org/",
            }),
            "go" => Some(LanguageProjectInfo {
                command_check: "go",
                create_command: |name| format!("mkdir {name} && cd {name} && go mod init {name}"),
                tool_name: "Go",
                install_instructions: "Install Go from https://golang.org/",
            }),
            "ruby" => Some(LanguageProjectInfo {
                command_check: "bundle",
                create_command: |name| {
                    format!("mkdir {name} && cd {name} && bundle init")
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
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: AppTheme,
) {
    // Check if a project was just created and switch to List tab if so
    if let Some(newly_created) = state.newly_created_repository.take() {
        log::info!("Project '{newly_created}' was created, switching to repository list");
        state.active_tab = super::types::RepoPanelTab::List;
        state.is_loading_repos = true; // Trigger a refresh
        return;
    }
    // Use repositories_base_path from config, with fallback
    let base_path = config.repositories_base_path();
    state.project_form.path = base_path.to_string_lossy().to_string();

    ui.heading("Create New Project");
    ui.add_space(8.0);

    // Always show project info frame to avoid layout changes
    Frame::NONE
        .fill(theme.info_background())
        .stroke(Stroke::new(1.0, theme.info_color()))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("💡").size(14.0));
                ui.vertical(|ui| {
                    ui.label(RichText::new("Project Info:").strong().color(theme.info_color()));
                    let path = &state.project_form.path;
                    let name = if state.project_form.name.is_empty() {
                        "<project-name>"
                    } else {
                        &state.project_form.name
                    };
                    ui.label(format!("• Location: {path}/{name}"));
                    
                    // Check if the language tool is available
                    if let Some(info) = LanguageProjectInfo::get_language_info(&state.project_form.language) {
                        let tool = info.tool_name;
                        ui.label(format!("• Will use {tool} to create project"));
                    }
                });
            });
        });
    ui.add_space(8.0);

    // Main form
    Frame::NONE
        .fill(theme.panel_background())
        .stroke(Stroke::new(1.0, theme.border_color()))
        .corner_radius(CornerRadius::same(6))
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

                        ComboBox::from_id_salt("language_combo")
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

                    // Project Location (read-only, always uses repositories_base_path)
                    ui.label(RichText::new("Base Location:").strong());
                    ui.label(RichText::new(&state.project_form.path).color(theme.hint_text_color()));
                    ui.end_row();

                    // Description
                    ui.label(RichText::new("Description:").strong());
                    ui.add_enabled(!state.project_form.creating, 
                        TextEdit::multiline(&mut state.project_form.description)
                            .desired_rows(2)
                            .hint_text("Brief description of the project..."));
                    ui.end_row();

                });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Options
            ui.label(RichText::new("⚙️ Options:").strong());
            ui.add_space(4.0);
            
            ui.horizontal(|ui| {
                ui.add_enabled(!state.project_form.creating, 
                    Checkbox::new(&mut state.project_form.initialize_git, ""));
                ui.label("Initialize Git repository");
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
                create_project(state, config, repo_manager.clone());
            }
            
            // Clear button
            if ui.add_enabled(!state.project_form.creating, Button::new("🗑️ Clear"))
                .clicked() {
                state.project_form = Default::default();
                // Use repositories_base_path with fallback
                let base_path = config.repositories_base_path();
                state.project_form.path = base_path.to_string_lossy().to_string();
            }
        });
    });
}

/// Create the project directly without AI involvement
fn create_project(
    state: &mut RepoPanelState,
    _config: &SagittaCodeConfig,
    repo_manager: Arc<Mutex<RepositoryManager>>,
) {
    let project_name = state.project_form.name.clone();
    let project_path = state.project_form.path.clone();
    let language = state.project_form.language.clone();
    let initialize_git = state.project_form.initialize_git;
    let description = state.project_form.description.clone();
    
    if let Some(info) = LanguageProjectInfo::get_language_info(&language) {
        let repo_manager_clone = repo_manager.clone();
        let full_path = format!("{project_path}/{project_name}");
        let project_name_for_state = project_name.clone();
        
        tokio::spawn(async move {
            // First check if the tool is available
            let check_command = if cfg!(windows) {
                let cmd = info.command_check;
                format!("where {cmd}")
            } else {
                let cmd = info.command_check;
                format!("which {cmd}")
            };

            match tokio::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" })
                .args(if cfg!(windows) { ["/C", &check_command] } else { ["-c", &check_command] })
                .output()
                .await
            {
                Ok(output) if output.status.success() => {
                    // Tool is available, create the base directory if it doesn't exist
                    if let Err(e) = tokio::fs::create_dir_all(&project_path).await {
                        log::error!("Failed to create base directory {project_path}: {e}");
                        return;
                    }
                    
                    // Create the project
                    let create_cmd = (info.create_command)(&project_name);
                    
                    let result = if cfg!(windows) {
                        tokio::process::Command::new("cmd")
                            .args(["/C", &format!("cd /d \"{project_path}\" && {create_cmd}")])
                            .output()
                            .await
                    } else {
                        tokio::process::Command::new("sh")
                            .args(["-c", &format!("cd '{project_path}' && {create_cmd}")])
                            .output()
                            .await
                    };
                    
                    match result {
                        Ok(output) if output.status.success() => {
                            log::info!("Project created successfully at {full_path}");
                            
                            // Initialize git if requested
                            if initialize_git {
                                let git_init_result = if cfg!(windows) {
                                    tokio::process::Command::new("cmd")
                                        .args(["/C", &format!("cd /d \"{full_path}\" && git init -b main")])
                                        .output()
                                        .await
                                } else {
                                    tokio::process::Command::new("sh")
                                        .args(["-c", &format!("cd '{full_path}' && git init -b main")])
                                        .output()
                                        .await
                                };
                                
                                match git_init_result {
                                    Ok(git_output) if git_output.status.success() => {
                                        log::info!("Git repository initialized at {full_path}");
                                        
                                        // Create an initial commit to avoid "unborn branch" state
                                        let readme_content = format!("# {project_name}\n\n{}", 
                                            if !description.is_empty() {
                                                &description
                                            } else {
                                                "A new project created with Sagitta Code."
                                            }
                                        );
                                        
                                        // Create README.md
                                        if let Err(e) = tokio::fs::write(format!("{full_path}/README.md"), &readme_content).await {
                                            log::warn!("Failed to create README.md: {e}");
                                        }
                                        
                                        // Create .gitignore with common patterns
                                        let gitignore_content = match language.as_str() {
                                            "rust" => "target/\nCargo.lock\n*.rs.bk\n",
                                            "python" => "venv/\n__pycache__/\n*.py[cod]\n.env\n",
                                            "javascript" | "typescript" => "node_modules/\nnpm-debug.log*\n.env\ndist/\n",
                                            "go" => "*.exe\n*.dll\n*.so\n*.dylib\n",
                                            _ => "*.log\n*.tmp\n.DS_Store\n",
                                        };
                                        
                                        if let Err(e) = tokio::fs::write(format!("{full_path}/.gitignore"), gitignore_content).await {
                                            log::warn!("Failed to create .gitignore: {e}");
                                        }
                                        
                                        // Create initial commit
                                        let commit_commands = format!(
                                            "cd '{full_path}' && git add . && git commit -m \"Initial commit\n\nProject created with Sagitta Code\""
                                        );
                                        
                                        let commit_result = if cfg!(windows) {
                                            tokio::process::Command::new("cmd")
                                                .args(["/C", &commit_commands.replace('\'', "\"")])
                                                .output()
                                                .await
                                        } else {
                                            tokio::process::Command::new("sh")
                                                .args(["-c", &commit_commands])
                                                .output()
                                                .await
                                        };
                                        
                                        match commit_result {
                                            Ok(output) if output.status.success() => {
                                                log::info!("Initial commit created for {full_path}");
                                            }
                                            Ok(output) => {
                                                let error = String::from_utf8_lossy(&output.stderr);
                                                log::warn!("Failed to create initial commit: {error}");
                                            }
                                            Err(e) => {
                                                log::warn!("Failed to execute commit command: {e}");
                                            }
                                        }
                                    }
                                    Ok(git_output) => {
                                        let error = String::from_utf8_lossy(&git_output.stderr);
                                        log::warn!("Git init warning for {full_path}: {error}");
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to initialize git for {full_path}: {e}");
                                    }
                                }
                            }
                            
                            // Add to repository manager as a local repository
                            let manager = repo_manager_clone.lock().await;
                            // For local projects, we need to add them with local_path instead of URL
                            if let Err(e) = manager.add_local_repository(&project_name, &full_path).await {
                                log::error!("Failed to add repository after creation: {e}");
                            } else {
                                log::info!("Successfully added repository '{project_name}' to Sagitta as local repository");
                                // Signal that the project was created successfully by storing the name
                                // This will trigger a tab switch and refresh in the next render cycle
                            }
                        }
                        Ok(output) => {
                            let error = String::from_utf8_lossy(&output.stderr);
                            log::error!("Failed to create project: {error}");
                        }
                        Err(e) => {
                            log::error!("Failed to execute create command: {e}");
                        }
                    }
                }
                _ => {
                    let tool = info.tool_name;
                    let instructions = info.install_instructions;
                    log::error!("{tool} is not installed. {instructions}");
                }
            }
        });

        state.project_form.status_message = Some("Creating project... Check the logs for progress.".to_string());
    } else {
        state.project_form.error_message = Some(format!("Project creation for {language} is not yet supported"));
    }
    
    state.project_form.creating = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::repository::types::ProjectCreationForm;

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
        assert_eq!(form.path, "");
        assert_eq!(form.description, "");
        assert!(form.initialize_git);
        assert!(!form.creating);
        assert!(form.status_message.is_none());
        assert!(form.error_message.is_none());
    }
}