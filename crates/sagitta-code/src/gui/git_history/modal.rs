use std::path::PathBuf;
use anyhow::Result;
use egui::{Context, Ui};
use git2::{Repository, Oid};
use chrono::{Utc, TimeZone};

use crate::gui::theme::AppTheme;
use super::types::{CommitInfo, GitHistoryState};

pub struct GitHistoryModal {
    pub visible: bool,
    repository_path: Option<PathBuf>,
    state: GitHistoryState,
    show_revert_confirmation: bool,
    revert_target: Option<String>,
    search_query: String,
    error_message: Option<String>,
    is_loading: bool,
    commits_per_page: usize,
    current_page: usize,
    selected_commits: Vec<String>,
    show_squash_dialog: bool,
    squash_message: String,
}

impl Default for GitHistoryModal {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHistoryModal {
    pub fn new() -> Self {
        Self {
            visible: false,
            repository_path: None,
            state: GitHistoryState {
                max_commits: 500,
                ..Default::default()
            },
            show_revert_confirmation: false,
            revert_target: None,
            search_query: String::new(),
            error_message: None,
            is_loading: false,
            commits_per_page: 25,
            current_page: 0,
            selected_commits: Vec::new(),
            show_squash_dialog: false,
            squash_message: String::new(),
        }
    }

    pub fn set_repository(&mut self, path: PathBuf) {
        if self.repository_path.as_ref() != Some(&path) {
            self.repository_path = Some(path);
            // Always refresh when repository changes, even if modal is visible
            self.refresh_commits();
            
            // Clear any previous state since it's from a different repo
            self.revert_target = None;
            self.show_revert_confirmation = false;
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

        egui::SidePanel::right("git_history_panel")
            .default_width(800.0)
            .min_width(600.0)
            .resizable(true)
            .frame(egui::Frame::NONE
                .fill(theme.panel_background())
                .inner_margin(egui::Margin::same(10)))
            .show(ctx, |ui| {
                // Apply theme to the UI
                ui.visuals_mut().override_text_color = Some(theme.text_color());
                ui.visuals_mut().widgets.noninteractive.bg_fill = theme.panel_background();
                ui.visuals_mut().widgets.inactive.bg_fill = theme.input_background();
                ui.visuals_mut().widgets.active.bg_fill = theme.button_background();
                ui.visuals_mut().widgets.hovered.bg_fill = theme.button_background().gamma_multiply(1.2);
                
                self.render_content(ui, theme);
            });

        // Render revert confirmation dialog
        if self.show_revert_confirmation {
            self.render_revert_confirmation(ctx, theme);
        }
        
        // Render squash dialog
        if self.show_squash_dialog {
            self.render_squash_dialog(ctx, theme);
        }
    }

    fn render_content(&mut self, ui: &mut Ui, theme: AppTheme) {
        // Header with controls
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("📜 Git History").heading().color(theme.text_color()));
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("✕").fill(theme.button_background())).clicked() {
                    self.visible = false;
                }
                
                ui.separator();
                
                if ui.add(egui::Button::new("🔄 Refresh").fill(theme.button_background())).clicked() {
                    self.refresh_commits();
                }
                
                // Search box with theme colors
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
            ui.colored_label(theme.error_color(), format!("Error: {error}"));
            ui.separator();
        }

        // Loading indicator
        if self.is_loading {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.colored_label(theme.text_color(), "Loading commit history...");
            });
            return;
        }

        // Main content area
        if self.repository_path.is_none() {
            ui.centered_and_justified(|ui| {
                ui.colored_label(theme.hint_text_color(), "No repository selected");
            });
            return;
        }

        if self.state.commits.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.colored_label(theme.hint_text_color(), "No commits found");
            });
            return;
        }
        
        // Action buttons
        ui.horizontal(|ui| {
            if !self.selected_commits.is_empty() {
                if ui.add(egui::Button::new(format!("🔀 Squash {} commit{}",
                    self.selected_commits.len(),
                    if self.selected_commits.len() == 1 { "" } else { "s" }
                )).fill(theme.accent_color())).clicked() {
                    self.show_squash_dialog = true;
                }
                
                if ui.add(egui::Button::new("Clear Selection").fill(theme.button_background())).clicked() {
                    self.selected_commits.clear();
                }
                
                ui.separator();
                ui.colored_label(theme.text_color(), format!("{} selected", self.selected_commits.len()));
            }
        });

        ui.separator();

        // Render commit log
        self.render_commit_log(ui, theme);
        
        // Pagination controls
        ui.separator();
        self.render_pagination_controls(ui, theme);
    }


    fn render_revert_confirmation(&mut self, ctx: &Context, theme: AppTheme) {
        egui::Window::new("Confirm Revert")
            .collapsible(false)
            .resizable(false)
            .frame(egui::Frame::window(&ctx.style()).fill(theme.panel_background()))
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(theme.text_color());
                ui.visuals_mut().widgets.noninteractive.bg_fill = theme.panel_background();
                ui.visuals_mut().widgets.inactive.bg_fill = theme.input_background();
                ui.visuals_mut().widgets.active.bg_fill = theme.button_background();
                
                ui.colored_label(theme.warning_color(), "⚠️ Warning: This will reset your repository to the selected commit.");
                ui.colored_label(theme.text_color(), "All changes after this commit will be lost!");
                ui.add_space(8.0);
                
                if let Some(commit_id) = &self.revert_target {
                    if let Some(commit) = self.find_commit(commit_id) {
                        ui.group(|ui| {
                            ui.visuals_mut().widgets.noninteractive.bg_fill = theme.input_background();
                            ui.colored_label(theme.text_color(), format!("Reverting to: {}", commit.short_id));
                            ui.colored_label(theme.text_color(), format!("Message: {}", commit.message));
                        });
                    }
                }
                
                ui.add_space(8.0);
                
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Cancel").fill(theme.button_background())).clicked() {
                        self.show_revert_confirmation = false;
                        self.revert_target = None;
                    }
                    
                    if ui.add(egui::Button::new(egui::RichText::new("Revert").color(egui::Color32::WHITE))
                        .fill(theme.error_color())).clicked() 
                    {
                        if let Some(commit_id) = self.revert_target.take() {
                            self.perform_revert(&commit_id);
                        }
                        self.show_revert_confirmation = false;
                    }
                });
            });
    }

    fn render_squash_dialog(&mut self, ctx: &Context, theme: AppTheme) {
        egui::Window::new("Squash Commits")
            .collapsible(false)
            .resizable(false)
            .frame(egui::Frame::window(&ctx.style()).fill(theme.panel_background()))
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(theme.text_color());
                ui.visuals_mut().widgets.noninteractive.bg_fill = theme.panel_background();
                ui.visuals_mut().widgets.inactive.bg_fill = theme.input_background();
                ui.visuals_mut().widgets.active.bg_fill = theme.button_background();
                
                ui.colored_label(theme.text_color(), format!("Squashing {} commits into one", self.selected_commits.len()));
                ui.separator();
                
                // Show selected commits
                ui.group(|ui| {
                    ui.visuals_mut().widgets.noninteractive.bg_fill = theme.input_background();
                    ui.colored_label(theme.text_color(), "Selected commits:");
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for commit_id in &self.selected_commits {
                                if let Some(commit) = self.state.commits.iter().find(|c| c.id == *commit_id) {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(theme.accent_color(), &commit.short_id);
                                        ui.colored_label(theme.hint_text_color(), "-");
                                        let msg = if commit.message.len() > 50 {
                                            format!("{}...", &commit.message[..47])
                                        } else {
                                            commit.message.clone()
                                        };
                                        ui.colored_label(theme.text_color(), msg);
                                    });
                                }
                            }
                        });
                });
                
                ui.add_space(8.0);
                ui.colored_label(theme.text_color(), "New commit message:");
                ui.text_edit_multiline(&mut self.squash_message);
                
                ui.add_space(8.0);
                
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Cancel").fill(theme.button_background())).clicked() {
                        self.show_squash_dialog = false;
                        self.squash_message.clear();
                    }
                    
                    if ui.add(egui::Button::new(egui::RichText::new("Squash").color(egui::Color32::WHITE))
                        .fill(theme.accent_color()))
                        .on_hover_text("This will create a new commit with all selected changes")
                        .clicked() 
                    {
                        if !self.squash_message.trim().is_empty() {
                            self.perform_squash();
                            self.show_squash_dialog = false;
                        }
                    }
                });
            });
    }

    fn refresh_commits(&mut self) {
        self.is_loading = true;
        self.error_message = None;
        
        if let Some(repo_path) = &self.repository_path.clone() {
            match self.fetch_commits(repo_path) {
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
                    self.error_message = Some(format!("Failed to revert: {e}"));
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

    fn perform_squash(&mut self) {
        if let Some(repo_path) = &self.repository_path {
            // Sort commits by their index in the commit list (oldest first)
            let mut sorted_commits: Vec<_> = self.selected_commits.iter()
                .filter_map(|id| {
                    self.state.commits.iter()
                        .position(|c| c.id == *id)
                        .map(|idx| (idx, id.clone()))
                })
                .collect();
            sorted_commits.sort_by_key(|(idx, _)| *idx);
            
            if sorted_commits.len() < 2 {
                self.error_message = Some("Need at least 2 commits to squash".to_string());
                return;
            }
            
            // Get the oldest commit (base)
            let base_commit = &sorted_commits.last().unwrap().1;
            
            match self.squash_commits(repo_path, base_commit, &sorted_commits, &self.squash_message) {
                Ok(()) => {
                    self.error_message = None;
                    self.selected_commits.clear();
                    self.squash_message.clear();
                    self.refresh_commits();
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to squash commits: {e}"));
                }
            }
        }
    }

    fn squash_commits(&self, repo_path: &PathBuf, base_commit: &str, _commits: &[(usize, String)], _message: &str) -> Result<()> {
        let _repo = Repository::open(repo_path)?;
        
        // This is a simplified implementation - in production you'd want to:
        // 1. Create a new branch
        // 2. Reset to the parent of the oldest commit
        // 3. Cherry-pick all changes
        // 4. Create a single commit with the new message
        
        // For now, we'll just show an error that this needs to be implemented
        Err(anyhow::anyhow!("Squash functionality not yet implemented. Use git rebase -i {} manually", base_commit))
    }

    fn render_commit_log(&mut self, ui: &mut Ui, theme: AppTheme) {
        // Filter commits based on search query
        let filtered_commits: Vec<CommitInfo> = if self.search_query.is_empty() {
            self.state.commits.clone()
        } else {
            let query = self.search_query.to_lowercase();
            self.state.commits.iter()
                .filter(|c| 
                    c.message.to_lowercase().contains(&query) ||
                    c.author.to_lowercase().contains(&query) ||
                    c.id.contains(&query) ||
                    c.short_id.contains(&query)
                )
                .cloned()
                .collect()
        };

        // Calculate pagination
        let total_commits = filtered_commits.len();
        let total_pages = (total_commits + self.commits_per_page - 1) / self.commits_per_page;
        self.current_page = self.current_page.min(total_pages.saturating_sub(1));
        
        let start_idx = self.current_page * self.commits_per_page;
        let end_idx = (start_idx + self.commits_per_page).min(total_commits);
        
        // Render commits as a log
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                
                for (idx, commit) in filtered_commits[start_idx..end_idx].iter().enumerate() {
                    let global_idx = start_idx + idx;
                    self.render_commit_row(ui, commit, global_idx, theme);
                }
            });
    }

    fn render_commit_row(&mut self, ui: &mut Ui, commit: &CommitInfo, _idx: usize, theme: AppTheme) {
        ui.group(|ui| {
            // Apply theme to group
            ui.visuals_mut().widgets.noninteractive.bg_fill = theme.input_background();
            ui.spacing_mut().item_spacing.y = 4.0;
            
            // First row: checkbox, commit hash, branches, date, actions
            ui.horizontal(|ui| {
                // Checkbox for squash selection
                let mut is_selected = self.selected_commits.contains(&commit.id);
                if ui.checkbox(&mut is_selected, "").changed() {
                    if is_selected {
                        self.selected_commits.push(commit.id.clone());
                    } else {
                        self.selected_commits.retain(|id| id != &commit.id);
                    }
                }
                
                // Commit hash (non-clickable now)
                ui.label(
                    egui::RichText::new(&commit.short_id)
                        .monospace()
                        .color(theme.accent_color())
                );
                
                // Branch refs
                for branch in &commit.branch_refs {
                    ui.label(
                        egui::RichText::new(format!("[{}]", branch))
                            .small()
                            .color(theme.info_color())
                    );
                }
                
                // Actions and date on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Revert button
                    if ui.add(egui::Button::new("⏪ Revert").fill(theme.warning_color())).clicked() {
                        self.revert_target = Some(commit.id.clone());
                        self.show_revert_confirmation = true;
                    }
                    
                    // Copy ID button
                    if ui.add(egui::Button::new("📋").fill(theme.button_background()))
                        .on_hover_text("Copy commit ID").clicked() {
                        ui.ctx().copy_text(commit.id.clone());
                    }
                    
                    ui.separator();
                    
                    // Date
                    ui.label(
                        egui::RichText::new(commit.timestamp.format("%Y-%m-%d %H:%M").to_string())
                            .small()
                            .color(theme.hint_text_color())
                    );
                });
            });
            
            // Second row: commit message
            ui.horizontal(|ui| {
                ui.add_space(20.0); // Indent
                ui.colored_label(theme.text_color(), &commit.message);
            });
            
            // Third row: author
            ui.horizontal(|ui| {
                ui.add_space(20.0); // Indent
                ui.label(
                    egui::RichText::new(format!("by {}", commit.author))
                        .small()
                        .color(theme.hint_text_color())
                );
            });
        });
    }
    
    fn render_pagination_controls(&mut self, ui: &mut Ui, theme: AppTheme) {
        let filtered_count = if self.search_query.is_empty() {
            self.state.commits.len()
        } else {
            let query = self.search_query.to_lowercase();
            self.state.commits.iter()
                .filter(|c| 
                    c.message.to_lowercase().contains(&query) ||
                    c.author.to_lowercase().contains(&query) ||
                    c.id.contains(&query) ||
                    c.short_id.contains(&query)
                )
                .count()
        };
        
        let total_pages = (filtered_count + self.commits_per_page - 1) / self.commits_per_page;
        
        ui.horizontal(|ui| {
            ui.colored_label(theme.text_color(), format!(
                "Showing {} commits (page {} of {})",
                filtered_count,
                self.current_page + 1,
                total_pages.max(1)
            ));
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Last page
                if ui.add_enabled(self.current_page < total_pages.saturating_sub(1), 
                    egui::Button::new("⏭").fill(theme.button_background())).clicked() {
                    self.current_page = total_pages.saturating_sub(1);
                }
                
                // Next page
                if ui.add_enabled(self.current_page < total_pages.saturating_sub(1), 
                    egui::Button::new("▶").fill(theme.button_background())).clicked() {
                    self.current_page += 1;
                }
                
                // Page number
                ui.colored_label(theme.text_color(), format!("{} / {}", self.current_page + 1, total_pages.max(1)));
                
                // Previous page
                if ui.add_enabled(self.current_page > 0, 
                    egui::Button::new("◀").fill(theme.button_background())).clicked() {
                    self.current_page = self.current_page.saturating_sub(1);
                }
                
                // First page
                if ui.add_enabled(self.current_page > 0, 
                    egui::Button::new("⏮").fill(theme.button_background())).clicked() {
                    self.current_page = 0;
                }
                
                ui.separator();
                
                // Items per page selector
                ui.colored_label(theme.text_color(), "Per page:");
                egui::ComboBox::from_id_salt("commits_per_page")
                    .selected_text(self.commits_per_page.to_string())
                    .show_ui(ui, |ui| {
                        for &count in &[10, 25, 50, 100] {
                            if ui.selectable_value(&mut self.commits_per_page, count, count.to_string()).clicked() {
                                self.current_page = 0; // Reset to first page
                            }
                        }
                    });
            });
        });
    }
}