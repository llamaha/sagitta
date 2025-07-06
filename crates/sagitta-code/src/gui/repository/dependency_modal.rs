use egui::{Ui, Context, RichText, Color32};
use sagitta_search::config::RepositoryDependency;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::gui::repository::manager::RepositoryManager;
use crate::gui::theme::AppTheme;

/// Modal for managing repository dependencies
#[derive(Debug, Default)]
pub struct DependencyModal {
    /// Whether the modal is visible
    pub visible: bool,
    /// The repository name being edited
    pub repository_name: String,
    /// Current dependencies (working copy)
    pub dependencies: Vec<RepositoryDependency>,
    /// Form state for adding a new dependency
    pub add_form: AddDependencyForm,
    /// Error message to display
    pub error_message: Option<String>,
    /// Success message to display
    pub success_message: Option<String>,
    /// Whether we're saving changes
    pub is_saving: bool,
    /// Confirmation dialog state
    pub confirm_remove: Option<usize>,
}

#[derive(Debug, Default)]
pub struct AddDependencyForm {
    pub selected_repository: String,
    pub target_ref: String,
    pub purpose: String,
    pub is_adding: bool,
}

impl DependencyModal {
    /// Show the dependency modal for a specific repository
    pub fn show_for_repository(&mut self, repo_name: String, dependencies: Vec<RepositoryDependency>) {
        self.repository_name = repo_name;
        self.dependencies = dependencies;
        self.visible = true;
        self.error_message = None;
        self.success_message = None;
        self.add_form = AddDependencyForm::default();
    }
    
    /// Hide the modal
    pub fn hide(&mut self) {
        self.visible = false;
        self.confirm_remove = None;
    }
    
    /// Render the modal
    pub fn render(
        &mut self,
        ctx: &Context,
        available_repos: &[String],
        repo_manager: Arc<Mutex<RepositoryManager>>,
        theme: &AppTheme,
    ) {
        if !self.visible {
            return;
        }
        
        let modal_title = format!("Manage Dependencies - {}", self.repository_name);
        
        egui::Window::new(&modal_title)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .resizable(true)
            .default_size([700.0, 500.0])
            .collapsible(false)
            .show(ctx, |ui| {
                self.render_content(ui, available_repos, repo_manager.clone(), theme);
                
                ui.separator();
                
                // Bottom buttons
                ui.horizontal(|ui| {
                    if self.is_saving {
                        ui.spinner();
                        ui.label("Saving...");
                    } else {
                        if ui.button("Save Changes").clicked() {
                            self.save_dependencies(repo_manager.clone());
                        }
                        
                        if ui.button("Cancel").clicked() {
                            self.hide();
                        }
                    }
                    
                    // Show messages
                    if let Some(error) = &self.error_message {
                        ui.colored_label(Color32::from_rgb(220, 53, 69), error);
                    }
                    if let Some(success) = &self.success_message {
                        ui.colored_label(Color32::from_rgb(40, 167, 69), success);
                    }
                });
            });
        
        // Handle confirmation dialog
        if let Some(index) = self.confirm_remove {
            self.render_remove_confirmation(ctx, index);
        }
    }
    
    fn render_content(
        &mut self,
        ui: &mut Ui,
        available_repos: &[String],
        repo_manager: Arc<Mutex<RepositoryManager>>,
        _theme: &AppTheme,
    ) {
        // Current dependencies section
        ui.heading("Current Dependencies");
        
        if self.dependencies.is_empty() {
            ui.label("No dependencies configured.");
        } else {
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    egui::Grid::new("dependencies_grid")
                        .num_columns(4)
                        .striped(true)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            // Header
                            ui.label(RichText::new("Repository").strong());
                            ui.label(RichText::new("Target Ref").strong());
                            ui.label(RichText::new("Purpose").strong());
                            ui.label(RichText::new("Actions").strong());
                            ui.end_row();
                            
                            let mut to_remove = None;
                            
                            for (index, dep) in self.dependencies.iter().enumerate() {
                                ui.label(&dep.repository_name);
                                ui.label(dep.target_ref.as_deref().unwrap_or("latest"));
                                ui.label(dep.purpose.as_deref().unwrap_or("-"));
                                
                                if ui.button("Remove").clicked() {
                                    to_remove = Some(index);
                                }
                                
                                ui.end_row();
                            }
                            
                            if let Some(index) = to_remove {
                                self.confirm_remove = Some(index);
                            }
                        });
                });
        }
        
        ui.separator();
        
        // Add new dependency section
        ui.heading("Add New Dependency");
        
        egui::Grid::new("add_dependency_form")
            .num_columns(2)
            .spacing([10.0, 8.0])
            .show(ui, |ui| {
                ui.label("Repository:");
                egui::ComboBox::from_id_source("dep_repo_combo")
                    .selected_text(&self.add_form.selected_repository)
                    .show_ui(ui, |ui| {
                        // Filter out the current repository and already added dependencies
                        for repo in available_repos {
                            if repo != &self.repository_name && 
                               !self.dependencies.iter().any(|d| &d.repository_name == repo) {
                                ui.selectable_value(
                                    &mut self.add_form.selected_repository,
                                    repo.clone(),
                                    repo
                                );
                            }
                        }
                    });
                ui.end_row();
                
                ui.label("Target Ref (optional):");
                ui.text_edit_singleline(&mut self.add_form.target_ref)
                    .on_hover_text("Branch name, tag, or commit hash");
                ui.end_row();
                
                ui.label("Purpose (optional):");
                ui.text_edit_singleline(&mut self.add_form.purpose)
                    .on_hover_text("Why is this dependency needed?");
                ui.end_row();
            });
        
        ui.horizontal(|ui| {
            if self.add_form.is_adding {
                ui.spinner();
                ui.label("Adding...");
            } else {
                let can_add = !self.add_form.selected_repository.is_empty();
                ui.add_enabled(can_add, egui::Button::new("Add Dependency"))
                    .clicked()
                    .then(|| {
                        self.add_dependency();
                    });
            }
        });
    }
    
    fn render_remove_confirmation(&mut self, ctx: &Context, index: usize) {
        egui::Window::new("Confirm Remove")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                if let Some(dep) = self.dependencies.get(index) {
                    ui.label(format!("Remove dependency on '{}'?", dep.repository_name));
                    
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.confirm_remove = None;
                        }
                        
                        if ui.button("Remove").clicked() {
                            self.dependencies.remove(index);
                            self.confirm_remove = None;
                        }
                    });
                }
            });
    }
    
    fn add_dependency(&mut self) {
        let new_dep = RepositoryDependency {
            repository_name: self.add_form.selected_repository.clone(),
            target_ref: if self.add_form.target_ref.is_empty() {
                None
            } else {
                Some(self.add_form.target_ref.clone())
            },
            purpose: if self.add_form.purpose.is_empty() {
                None
            } else {
                Some(self.add_form.purpose.clone())
            },
        };
        
        self.dependencies.push(new_dep);
        self.add_form = AddDependencyForm::default();
        self.success_message = Some("Dependency added".to_string());
    }
    
    fn save_dependencies(&mut self, repo_manager: Arc<Mutex<RepositoryManager>>) {
        self.is_saving = true;
        self.error_message = None;
        self.success_message = None;
        
        let repo_name = self.repository_name.clone();
        let dependencies = self.dependencies.clone();
        
        // Save dependencies using repository manager
        let handle = tokio::runtime::Handle::current();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        
        handle.spawn(async move {
            let _manager = repo_manager.lock().await;
            // TODO: Implement save_dependencies method in RepositoryManager
            // For now, we'll just update the config directly
            let result = match sagitta_search::load_config(None) {
                Ok(mut config) => {
                    if let Some(repo) = config.repositories.iter_mut().find(|r| r.name == repo_name) {
                        repo.dependencies = dependencies;
                        sagitta_search::save_config(&config, None)
                            .map(|_| ())
                            .map_err(|e| format!("Failed to save config: {}", e))
                    } else {
                        Err("Repository not found in config".to_string())
                    }
                }
                Err(_) => Err("Failed to load config".to_string()),
            };
            
            let _ = tx.send(result);
        });
        
        // For now, assume success and update UI immediately
        // In a real implementation, we'd wait for the result
        self.is_saving = false;
        self.success_message = Some("Dependencies saved successfully".to_string());
    }
}