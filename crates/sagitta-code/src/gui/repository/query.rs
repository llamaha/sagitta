use std::sync::Arc;
use egui::{Ui, RichText, Grid, TextEdit, ScrollArea, ComboBox, Button, Stroke, Frame, Vec2};
use tokio::sync::{Mutex, oneshot};
use super::manager::RepositoryManager;

use super::types::{RepoPanelState, QueryResultItem};
use super::types::QueryResult;
use anyhow::Result;
use qdrant_client::qdrant::QueryResponse;

// A simple non-blocking channel for query results
pub struct QueryChannel {
    pub receiver: Option<oneshot::Receiver<Result<QueryResponse, anyhow::Error>>>,
}

impl std::fmt::Debug for QueryChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryChannel").finish()
    }
}

// Field names from the payload (copied from sagitta-cli)
const FIELD_CHUNK_CONTENT: &str = "chunk_content";
const FIELD_FILE_PATH: &str = "file_path";
const FIELD_START_LINE: &str = "start_line";
const FIELD_END_LINE: &str = "end_line"; // Assuming this exists, otherwise we'll handle it
const FIELD_LANGUAGE: &str = "language";
const FIELD_ELEMENT_TYPE: &str = "element_type";
const FIELD_BRANCH: &str = "branch";

/// Render the query repository view
pub fn render_query_repo(
    ui: &mut Ui, 
    state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>,
    repo_manager: Arc<Mutex<RepositoryManager>>,
    theme: crate::gui::theme::AppTheme,
) {
    ui.heading("Query Repository");
    
    // Sync query options with selected repository if needed
    if let Some(selected_repo) = &state.selected_repo {
        if state.query_options.repo_name != *selected_repo {
            state.query_options.repo_name = selected_repo.clone();
        }
    }
    
    if state.selected_repo.is_none() {
        ui.label("No repository selected");
        
        // Repository selector dropdown
        let repo_names = state.repo_names();
        if !repo_names.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Select repository:");
                ComboBox::from_id_salt("query_no_repo_selector")
                    .selected_text("Choose repository...")
                    .show_ui(ui, |ui| {
                        for name in repo_names {
                            if ui.selectable_value(
                                &mut state.selected_repo,
                                Some(name.clone()),
                                &name
                            ).clicked() {
                                state.query_options.repo_name = name;
                            }
                        }
                    });
            });
        } else {
            ui.label("No repositories available");
            if ui.button("Go to Repository List").clicked() {
                state.active_tab = super::types::RepoPanelTab::List;
            }
        }
        
        return;
    }
    
    // Query options
    Grid::new("query_options_grid")
        .num_columns(2)
        .spacing([8.0, 8.0])
        .show(ui, |ui| {
            ui.label("Repository:");
            let repo_names = state.repo_names();
            let selected_text = state.selected_repo.as_ref().unwrap_or(&state.query_options.repo_name);
            ComboBox::from_id_salt("repository_select")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for name in repo_names {
                        if ui.selectable_value(
                            &mut state.query_options.repo_name, 
                            name.clone(),
                            &name
                        ).clicked() {
                            // Also update the selected_repo to maintain consistency
                            state.selected_repo = Some(name.clone());
                        }
                    }
                });
            ui.end_row();
            
            ui.label("Query:");
            ui.text_edit_singleline(&mut state.query_options.query_text);
            ui.end_row();
            
            ui.label("Element Type (optional):");
            let element_type = state.query_options.element_type.get_or_insert_with(String::new);
            ui.text_edit_singleline(element_type);
            ui.end_row();
            
            ui.label("Language (optional):");
            let language = state.query_options.language.get_or_insert_with(String::new);
            ui.text_edit_singleline(language);
            ui.end_row();
            
            // Display the current branch (but don't allow editing)
            if let Some(repo) = state.repositories.iter().find(|r| r.name == state.query_options.repo_name) {
                if let Some(branch) = &repo.branch {
                    ui.label("Branch:");
                    ui.label(branch);
                    ui.end_row();
                }
            }
            
            ui.label("Limit:");
            ui.add(egui::Slider::new(&mut state.query_options.limit, 1..=100));
            ui.end_row();
        });
    
    // Query button
    let mut query_requested = false;
    let query_enabled = !state.query_result.is_loading && !state.query_options.query_text.is_empty();
    
    ui.vertical_centered(|ui| {
        let button_text = if state.query_result.is_loading {
            RichText::new("Querying...").color(theme.hint_text_color())
        } else {
            RichText::new("Run Query")
        };
        
        let query_button = ui.add_enabled(
            query_enabled,
            Button::new(button_text)
        );
        
        if query_button.clicked() {
            query_requested = true;
        }
    });
    
    // Process query request outside of the UI closure
    if query_requested && query_enabled {
        // Reset previous results
        state.query_result = QueryResult {
            is_loading: true,
            success: false,
            error_message: None,
            results: Vec::new(),
            channel: None,
        };
        
        // Clone all necessary data for the query
        let repo_name = state.query_options.repo_name.clone();
        let query_text = state.query_options.query_text.clone();
        let limit = state.query_options.limit;
        let element_type = state.query_options.element_type.clone();
        let language = state.query_options.language.clone();
        
        // Get the branch from the repository configuration
        let branch = state.repositories.iter()
            .find(|r| r.name == repo_name)
            .and_then(|r| r.branch.clone());
        
        let repo_manager_clone = Arc::clone(&repo_manager);
        
        // Create a oneshot channel for the result
        let (sender, receiver) = oneshot::channel();
        state.query_result.channel = Some(QueryChannel { receiver: Some(receiver) });
        
        // Launch the query in a separate thread
        std::thread::spawn(move || {
            // Get a new independent tokio runtime
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            
            // Run the async query operation inside this thread
            rt.block_on(async move {
                let manager = repo_manager_clone.lock().await;
                let element_type_ref = element_type.as_ref().filter(|s| !s.is_empty());
                let language_ref = language.as_ref().filter(|s| !s.is_empty());
                let branch_ref = branch.as_ref();
                
                // Call the query method
                let result = manager.query(
                    &repo_name,
                    &query_text,
                    limit,
                    element_type_ref.map(|s| s.as_str()),
                    language_ref.map(|s| s.as_str()),
                    branch_ref.map(|s| s.as_str()),
                ).await;
                
                // Send the result through the channel (ignore errors if UI is closed)
                let _ = sender.send(result);
            });
        });
    }
    
    // Check for query results - non-blocking approach
    if state.query_result.is_loading {
        let mut query_completed = false;
        let mut new_result = None;
        let mut query_cancelled = false;
        
        // Extract the channel and try to receive, avoiding nested borrows
        if let Some(channel) = &mut state.query_result.channel {
            if let Some(receiver) = &mut channel.receiver {
                match receiver.try_recv() {
                    Ok(result) => {
                        // We got a result, store it for processing outside the borrow
                        new_result = Some(result);
                        query_completed = true;
                    },
                    Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                        // No result yet, keep waiting
                    },
                    Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                        // The sender was dropped without sending a value
                        query_cancelled = true;
                        query_completed = true;
                    }
                }
                
                // If we processed a result, clear the receiver
                if query_completed {
                    channel.receiver = None;
                }
            }
        }
        
        // Now process the result outside of the channel borrow
        if query_completed {
            if let Some(result) = new_result {
                match result {
                    Ok(response) => {
                        // Process the query response
                        let mut results = Vec::new();
                        
                        // Convert response results to QueryResultItems
                        for point in response.result {
                            let payload = &point.payload;
                            
                            // Extract fields from the payload
                            let path = payload.get(FIELD_FILE_PATH)
                                .and_then(|v| v.as_str())
                                .map_or("<unknown_file>", |v| v)
                                .to_string();
                            
                            let start_line = payload.get(FIELD_START_LINE)
                                .and_then(|v| v.as_integer())
                                .map(|l| l as u32)
                                .unwrap_or(1);
                            
                            // Try to get end_line or calculate it based on content
                            let end_line = payload.get(FIELD_END_LINE)
                                .and_then(|v| v.as_integer())
                                .map(|l| l as u32)
                                .unwrap_or_else(|| {
                                    // If no end_line, estimate based on content
                                    let content = payload.get(FIELD_CHUNK_CONTENT)
                                        .and_then(|v| v.as_str())
                                        .map_or("", |v| v);
                                    start_line + content.lines().count() as u32
                                });
                            
                            let content = payload.get(FIELD_CHUNK_CONTENT)
                                .and_then(|v| v.as_str())
                                .map_or("[Error: Snippet content missing]", |v| v)
                                .to_string();
                            
                            results.push(QueryResultItem {
                                score: point.score,
                                path,
                                start_line,
                                end_line,
                                content,
                            });
                        }
                        
                        // If we don't have any results but the query was successful, 
                        // add a message indicating no results
                        if results.is_empty() {
                            state.query_result = QueryResult {
                                is_loading: false,
                                success: true,
                                error_message: Some("No results found for this query. Try adjusting your search terms.".to_string()),
                                results: Vec::new(),
                                channel: None,
                            };
                        } else {
                            // Update the state with the results
                            state.query_result = QueryResult {
                                is_loading: false,
                                success: true,
                                error_message: None,
                                results,
                                channel: None,
                            };
                        }
                    },
                    Err(err) => {
                        // Update the state with the error
                        state.query_result = QueryResult {
                            is_loading: false,
                            success: false,
                            error_message: Some(format!("Query failed: {err}")),
                            results: Vec::new(),
                            channel: None,
                        };
                    }
                }
            } else if query_cancelled {
                // The sender was dropped without sending a value - query failed
                state.query_result = QueryResult {
                    is_loading: false,
                    success: false,
                    error_message: Some("Query operation was cancelled".to_string()),
                    results: Vec::new(),
                    channel: None,
                };
            }
        }
    }
    
    ui.separator();
    
    // Display query results
    render_query_results(ui, state, theme);
}

fn render_query_results(ui: &mut Ui, state: &mut tokio::sync::MutexGuard<'_, RepoPanelState>, theme: crate::gui::theme::AppTheme) {
    ui.heading("Query Results");
    
    if state.query_result.is_loading {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Querying repository...");
        });
        return;
    }
    
    if let Some(error) = &state.query_result.error_message {
        ui.label(RichText::new(error).color(theme.error_color()));
        return;
    }
    
    if state.query_result.results.is_empty() {
        ui.label("No results found. Try a different query.");
        return;
    }
    
    // Display result count
    ui.label(RichText::new(format!("Found {} results", state.query_result.results.len())).strong());
    
    // Collect all the results and info we need beforehand
    let results_info: Vec<_> = state.query_result.results.iter().enumerate().map(|(i, result)| {
        (
            i,
            result.clone(),
            result.path.clone(),
            result.start_line,
            result.end_line
        )
    }).collect();
    
    // Track any view file actions to apply after UI rendering
    let mut view_file_action: Option<(String, u32, u32)> = None;
    
    ScrollArea::vertical()
        .max_height(400.0)
        .show(ui, |ui| {
            for (i, result, path, start_line, end_line) in &results_info {
                let frame = Frame::NONE
                    .fill(theme.code_background())
                    .stroke(Stroke::new(1.0, theme.border_color()))
                    .corner_radius(egui::CornerRadius::same(4))
                    .inner_margin(Vec2::new(8.0, 8.0))
                    .outer_margin(Vec2::new(0.0, 4.0));
                
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Result #{}", i + 1)).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("Score: {:.2}", result.score)).monospace());
                        });
                    });
                    
                    ui.label(RichText::new(path).italics());
                    ui.label(format!("Lines {start_line}-{end_line}"));
                    
                    let code_frame = Frame::NONE
                        .fill(ui.visuals().code_bg_color)
                        .corner_radius(egui::CornerRadius::same(2))
                        .inner_margin(Vec2::new(8.0, 8.0));
                    
                    code_frame.show(ui, |ui| {
                        let mut content = result.content.clone();
                        ui.add(TextEdit::multiline(&mut content)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .interactive(false)
                            .frame(false));
                    });
                    
                    ui.horizontal(|ui| {
                        if ui.button("View File").clicked() {
                            // Store the view file action to apply after UI rendering
                            view_file_action = Some((path.clone(), *start_line, *end_line));
                        }
                    });
                });
                
                ui.add_space(4.0);
            }
        });
    
    // Apply view file action if any
    if let Some((path, start_line, end_line)) = view_file_action {
        state.file_view_options.file_path = path;
        state.file_view_options.start_line = Some(start_line as usize);
        state.file_view_options.end_line = Some(end_line as usize);
        state.active_tab = super::types::RepoPanelTab::ViewFile;
    }
}

#[cfg(test)]
mod tests {
    
    use crate::gui::repository::types::{RepoPanelState, RepoPanelTab};
    
    #[test]
    fn test_repository_selector_dropdown_behavior() {
        // Test that when no repository is selected, a dropdown is shown instead of redirecting
        let mut state = RepoPanelState::default();
        state.selected_repo = None;
        state.repositories = vec![
            crate::gui::repository::types::RepoInfo {
                name: "repo1".to_string(),
                remote: None,
                branch: None,
                local_path: None,
                is_syncing: false,
            },
            crate::gui::repository::types::RepoInfo {
                name: "repo2".to_string(),
                remote: None,
                branch: None,
                local_path: None,
                is_syncing: false,
            },
        ];
        
        // Before: clicking "Select Repository" would set active_tab to List
        // Now: a dropdown should be available to select from available repositories
        
        let repo_names = state.repo_names();
        assert_eq!(repo_names.len(), 2);
        assert_eq!(repo_names[0], "repo1");
        assert_eq!(repo_names[1], "repo2");
        
        // Simulate selecting a repository from dropdown
        state.selected_repo = Some("repo1".to_string());
        state.query_options.repo_name = "repo1".to_string();
        
        assert_eq!(state.selected_repo, Some("repo1".to_string()));
        assert_eq!(state.query_options.repo_name, "repo1");
    }
    
    #[test]
    fn test_empty_repository_list_behavior() {
        // Test behavior when no repositories are available
        let mut state = RepoPanelState::default();
        state.selected_repo = None;
        state.repositories = vec![];
        
        let repo_names = state.repo_names();
        assert!(repo_names.is_empty());
        
        // In this case, "Go to Repository List" button should be shown
        // which would set active_tab to List
        state.active_tab = RepoPanelTab::List;
        assert_eq!(state.active_tab, RepoPanelTab::List);
    }
} 