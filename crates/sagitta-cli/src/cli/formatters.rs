// Formatters for displaying search results, stats, etc. 
use anyhow::Result;
use colored::*;
use qdrant_client::qdrant::ScoredPoint;
use serde_json;
use serde::Serialize;

use crate::cli::{
    commands::{ // Import constants directly from commands where they are re-exported
        FIELD_CHUNK_CONTENT, FIELD_ELEMENT_TYPE, FIELD_FILE_PATH, FIELD_LANGUAGE, FIELD_START_LINE, FIELD_END_LINE,
        // New repo fields might be useful here later
        // FIELD_BRANCH, FIELD_COMMIT_HASH
    }
};

// Create a serializable struct to represent search results
#[derive(Serialize)]
struct SearchResult {
    score: f32,
    file_path: String,
    start_line: usize,
    end_line: usize,
    language: String,
    element_type: String,
    content: String,
    preview: Option<String>,
}

/// Formats search results for display, handling both JSON and human-readable output.
pub fn print_search_results(results: &[ScoredPoint], query_text: &str, json_output: bool) -> Result<()> {
    if results.is_empty() {
        if json_output {
            println!("[]"); // Output empty JSON array
        } else {
            println!("No results found for query: \"{query_text}\"");
        }
        return Ok(());
    }

    if json_output {
        // Convert ScoredPoint array to our serializable SearchResult array
        let serializable_results: Vec<SearchResult> = results
            .iter()
            .map(|point| {
                let payload_map = &point.payload;
                let file_path = payload_map
                    .get(FIELD_FILE_PATH)
                    .and_then(|v| v.as_str())
                    .map_or("<unknown_file>", |v| v)
                    .to_string();
                let start_line = payload_map
                    .get(FIELD_START_LINE)
                    .and_then(|v| v.as_integer())
                    .map(|l| l as usize)
                    .unwrap_or(0);
                let end_line = payload_map
                    .get(FIELD_END_LINE)
                    .and_then(|v| v.as_integer())
                    .map(|l| l as usize)
                    .unwrap_or(start_line);
                let language = payload_map.get(FIELD_LANGUAGE).and_then(|v| v.as_str()).map_or("<unknown>", |v| v).to_string();
                let element_type = payload_map.get(FIELD_ELEMENT_TYPE).and_then(|v| v.as_str()).map_or("<unknown>", |v| v).to_string();
                let content = payload_map.get(FIELD_CHUNK_CONTENT).and_then(|v| v.as_str()).map(|s| s.to_string());
                
                // Extract preview from content if available
                let preview = content.as_ref().and_then(|c| c.lines().next()).map(|line| {
                    if line.len() > 120 {
                        format!("{}...", &line[..117])
                    } else {
                        line.to_string()
                    }
                });

                SearchResult {
                    score: point.score,
                    file_path,
                    start_line,
                    end_line,
                    language,
                    element_type, 
                    content: content.unwrap_or_else(|| "[Content not included]".to_string()),
                    preview,
                }
            })
            .collect();

        // Serialize the results to JSON, wrapped in a "results" field
        let output_json = serde_json::json!({ "results": serializable_results });
        let json_string = serde_json::to_string_pretty(&output_json)?;
        println!("{json_string}");
    } else {
        // Original human-readable output
        println!("Search results for: \"{}\"", query_text.cyan());
        println!("{}", "=".repeat(40)); // Separator

        for (idx, point) in results.iter().enumerate() {
            let payload_map = &point.payload; 

            let file_path = payload_map
                .get(FIELD_FILE_PATH)
                .and_then(|v| v.as_str())
                .map_or("<unknown_file>", |v| v)
                .to_string();
            let start_line = payload_map
                .get(FIELD_START_LINE)
                .and_then(|v| v.as_integer())
                .map(|l| l as usize)
                .unwrap_or(0);
            let language = payload_map.get(FIELD_LANGUAGE).and_then(|v| v.as_str()).map_or("<unknown>", |v| v).to_string();
            let element_type = payload_map.get(FIELD_ELEMENT_TYPE).and_then(|v| v.as_str()).map_or("<unknown>", |v| v).to_string();
            let snippet = payload_map.get(FIELD_CHUNK_CONTENT).and_then(|v| v.as_str()).map(|s| s.to_string());
            
            // Get end line if available
            let end_line = payload_map
                .get(FIELD_END_LINE)
                .and_then(|v| v.as_integer())
                .map(|l| l as usize)
                .unwrap_or(start_line);

            // Format line range
            let line_range = if end_line > start_line {
                format!("{start_line}-{end_line}")
            } else {
                start_line.to_string()
            };

            println!(
                "{}. Score: {:.4} | File: {} | Lines: {} | Lang: {} | Type: {}",
                (idx + 1).to_string().bold(),
                point.score,
                file_path.green(),
                line_range.yellow(),
                language,
                element_type
            );

            // Print the content if available, otherwise show preview
            if let Some(content) = snippet {
                // Print the full chunk content as the snippet, indented
                println!("{}", "-".repeat(4));
                for line in content.lines() {
                    println!("  {line}");
                }
            } else {
                // No full content available - show preview if we have it
                println!("{}", "-".repeat(4));
                
                // Try to get preview from payload
                let preview = payload_map.get(FIELD_CHUNK_CONTENT).and_then(|v| v.as_str())
                    .and_then(|c| c.lines().next())
                    .map(|line| {
                        if line.len() > 100 {
                            format!("{}...", &line[..97])
                        } else {
                            line.to_string()
                        }
                    });
                
                if let Some(preview_line) = preview {
                    println!("  {preview_line}");
                    println!("  {} Use 'sagitta-cli repo view-file -n <repo> {} -s {} -e {}' for full content",
                            "[...]".dimmed(),
                            file_path,
                            start_line,
                            end_line
                    );
                } else {
                    println!("  {} Use 'sagitta-cli repo view-file -n <repo> {} -s {} -e {}' to see content",
                            "[No preview available]".dimmed(),
                            file_path,
                            start_line,
                            end_line
                    );
                }
            }
            println!("{}", "=".repeat(40)); // Separator between results
        }
    }
    Ok(())
} 