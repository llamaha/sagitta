use std::path::PathBuf;
use anyhow::Result;
use egui::{Context, Ui};
use git2::{Repository, Oid};
use chrono::{Utc, TimeZone};

use crate::gui::theme::AppTheme;
use super::types::{CommitInfo, GitHistoryState};
use super::graph::render_commit_graph;

pub struct GitHistoryModal {
    pub visible: bool,
    repository_path: Option<PathBuf>,
    state: GitHistoryState,
    show_revert_confirmation: bool,
    revert_target: Option<String>,
    search_query: String,
    error_message: Option<String>,
    is_loading: bool,
}

impl GitHistoryModal {
    pub fn new() -> Self {
        Self {
            visible: false,
            repository_path: None,
            state: GitHistoryState {
                max_commits: 100,
                ..Default::default()
            },
            show_revert_confirmation: false,
            revert_target: None,
            search_query: String::new(),
            error_message: None,
            is_loading: false,
        }
    }

    pub fn set_repository(&mut self, path: PathBuf) {
        if self.repository_path.as_ref() != Some(&path) {
            self.repository_path = Some(path);
            self.refresh_commits();
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible && self.repository_path.is_some() {
            self.refresh_commits();
        }
    }

    pub fn render(&mut self, ctx: &Context, theme: AppTheme) {
        if !self.visible {
            return;
        }

        egui::Window::new("📜 Git History")
            .default_size([800.0, 600.0])
            .resizable(true)
            .collapsible(false)
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                self.render_content(ui, theme);
            });

        // Render revert confirmation dialog
        if self.show_revert_confirmation {
            self.render_revert_confirmation(ctx, theme);
        }
    }

    fn render_content(&mut self, ui: &mut Ui, theme: AppTheme) {
        // Header with controls
        ui.horizontal(|ui| {
            ui.heading("Repository History");
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✕").clicked() {
                    self.visible = false;
                }
                
                ui.separator();
                
                if ui.button("🔄 Refresh").clicked() {
                    self.refresh_commits();
                }
                
                // Search box
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .desired_width(200.0)
                        .hint_text("Search commits...")
                );
            });
        });

        ui.separator();

        // Error message if any
        if let Some(error) = &self.error_message {
            ui.colored_label(theme.error_color(), format!("Error: {}", error));
            ui.separator();
        }

        // Loading indicator
        if self.is_loading {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.label("Loading commit history...");
            });
            return;
        }

        // Main content area
        if self.repository_path.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("No repository selected");
            });
            return;
        }

        if self.state.commits.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No commits found");
            });
            return;
        }

        // Split view: graph on left, details on right
        ui.columns(2, |columns| {
            // Left column: Commit graph
            columns[0].group(|ui| {
                ui.label(egui::RichText::new("Commit Graph").strong());
                ui.separator();
                
                egui::ScrollArea::both()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        render_commit_graph(ui, &mut self.state, theme);
                    });
            });

            // Right column: Commit details
            columns[1].group(|ui| {
                ui.label(egui::RichText::new("Commit Details").strong());
                ui.separator();
                
                if let Some(selected_id) = &self.state.selected_commit {
                    if let Some(commit) = self.state.commits.iter().find(|c| c.id == *selected_id) {
                        let commit_clone = commit.clone();
                        self.render_commit_details(ui, &commit_clone, theme);
                    } else {
                        ui.label("Select a commit to view details");
                    }
                } else {
                    ui.label("Select a commit to view details");
                }
            });
        });
    }

    fn render_commit_details(&mut self, ui: &mut Ui, commit: &CommitInfo, theme: AppTheme) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Commit ID
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Commit:").strong());
                    ui.monospace(&commit.short_id);
                    if ui.button("📋").on_hover_text("Copy full commit ID").clicked() {
                        ui.output_mut(|o| o.copied_text = commit.id.clone());
                    }
                });

                // Author
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Author:").strong());
                    ui.label(format!("{} <{}>", commit.author, commit.email));
                });

                // Date
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Date:").strong());
                    ui.label(commit.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string());
                });

                // Branches
                if !commit.branch_refs.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Branches:").strong());
                        for branch in &commit.branch_refs {
                            ui.label(egui::RichText::new(branch).color(theme.accent_color()));
                        }
                    });
                }

                ui.separator();

                // Message
                ui.label(egui::RichText::new("Message:").strong());
                ui.add_space(4.0);
                ui.label(&commit.message);

                ui.add_space(16.0);

                // Actions
                ui.horizontal(|ui| {
                    if ui.button("⏪ Revert to this commit")
                        .on_hover_text("Revert repository to this commit")
                        .clicked() 
                    {
                        self.revert_target = Some(commit.id.clone());
                        self.show_revert_confirmation = true;
                    }
                });
            });
    }

    fn render_revert_confirmation(&mut self, ctx: &Context, theme: AppTheme) {
        egui::Window::new("Confirm Revert")
            .collapsible(false)
            .resizable(false)
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                ui.label("⚠️ Warning: This will reset your repository to the selected commit.");
                ui.label("All changes after this commit will be lost!");
                ui.add_space(8.0);
                
                if let Some(commit_id) = &self.revert_target {
                    if let Some(commit) = self.find_commit(commit_id) {
                        ui.group(|ui| {
                            ui.label(format!("Reverting to: {}", commit.short_id));
                            ui.label(format!("Message: {}", commit.message));
                        });
                    }
                }
                
                ui.add_space(8.0);
                
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_revert_confirmation = false;
                        self.revert_target = None;
                    }
                    
                    if ui.button(egui::RichText::new("Revert").color(theme.error_color()))
                        .clicked() 
                    {
                        if let Some(commit_id) = self.revert_target.take() {
                            self.perform_revert(&commit_id);
                        }
                        self.show_revert_confirmation = false;
                    }
                });
            });
    }

    fn refresh_commits(&mut self) {
        self.is_loading = true;
        self.error_message = None;
        
        if let Some(repo_path) = &self.repository_path.clone() {
            match self.fetch_commits(&repo_path) {
                Ok(()) => {
                    self.error_message = None;
                }
                Err(e) => {
                    self.error_message = Some(e.to_string());
                    self.state.commits.clear();
                }
            }
        }
        
        self.is_loading = false;
    }

    fn fetch_commits(&mut self, repo_path: &PathBuf) -> Result<()> {
        let repo = Repository::open(repo_path)?;
        let mut revwalk = repo.revwalk()?;
        
        // Start from HEAD
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;
        
        let mut commits = Vec::new();
        let mut count = 0;
        
        for oid_result in revwalk {
            if count >= self.state.max_commits {
                break;
            }
            
            let oid = oid_result?;
            let commit = repo.find_commit(oid)?;
            
            let commit_info = self.create_commit_info(&commit)?;
            commits.push(commit_info);
            count += 1;
        }
        
        self.state.commits = commits;
        self.state.commit_map = self.state.commits
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.clone(), i))
            .collect();
        
        Ok(())
    }

    fn create_commit_info(&self, commit: &git2::Commit) -> Result<CommitInfo> {
        let id = commit.id().to_string();
        let short_id = id.chars().take(7).collect();
        let message = commit.message().unwrap_or("<no message>").to_string();
        let author = commit.author();
        let author_name = author.name().unwrap_or("<unknown>").to_string();
        let email = author.email().unwrap_or("<unknown>").to_string();
        
        let timestamp = Utc.timestamp_opt(commit.time().seconds(), 0)
            .single()
            .unwrap_or_else(Utc::now);
        
        let parents = commit.parent_ids()
            .map(|oid| oid.to_string())
            .collect();
        
        Ok(CommitInfo {
            id,
            short_id,
            message,
            author: author_name,
            email,
            timestamp,
            parents,
            branch_refs: Vec::new(), // TODO: Fetch branch refs
        })
    }

    fn find_commit(&self, id: &str) -> Option<&CommitInfo> {
        self.state.commits.iter().find(|c| c.id == id)
    }

    fn perform_revert(&mut self, commit_id: &str) {
        if let Some(repo_path) = &self.repository_path {
            match self.revert_to_commit(repo_path, commit_id) {
                Ok(()) => {
                    self.error_message = None;
                    self.refresh_commits();
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to revert: {}", e));
                }
            }
        }
    }

    fn revert_to_commit(&self, repo_path: &PathBuf, commit_id: &str) -> Result<()> {
        let repo = Repository::open(repo_path)?;
        let oid = Oid::from_str(commit_id)?;
        let commit = repo.find_commit(oid)?;
        
        // Reset to the commit
        let mut checkout_builder = git2::build::CheckoutBuilder::new();
        checkout_builder.force();
        
        repo.reset(
            commit.as_object(),
            git2::ResetType::Hard,
            Some(&mut checkout_builder),
        )?;
        
        Ok(())
    }
}