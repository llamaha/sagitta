// Formatters for displaying search results, stats, etc. 
use anyhow::Result;
use colored::*;
use qdrant_client::qdrant::ScoredPoint;
use serde_json;
use serde::Serialize;

use super::commands::{ // Use super::commands path
    FIELD_CHUNK_CONTENT, FIELD_ELEMENT_TYPE, FIELD_FILE_PATH, FIELD_LANGUAGE, FIELD_START_LINE,
    // New repo fields might be useful here later
    // FIELD_BRANCH, FIELD_COMMIT_HASH
};

// Create a serializable struct to represent search results
#[derive(Serialize)]
struct SearchResult {
    score: f32,
    file_path: String,
    start_line: usize,
    language: String,
    element_type: String,
    content: String,
}

pub fn print_search_results(results: &[ScoredPoint], query_text: &str, json_output: bool) -> Result<()> {
    if results.is_empty() {
        if json_output {
            println!("[]"); // Output empty JSON array
        } else {
            println!("No results found for query: \"{}\"", query_text);
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
                let language = payload_map.get(FIELD_LANGUAGE).and_then(|v| v.as_str()).map_or("<unknown>", |v| v).to_string();
                let element_type = payload_map.get(FIELD_ELEMENT_TYPE).and_then(|v| v.as_str()).map_or("<unknown>", |v| v).to_string();
                let content = payload_map.get(FIELD_CHUNK_CONTENT).and_then(|v| v.as_str()).map_or("[Error: Snippet content missing]", |v| v).to_string();

                SearchResult {
                    score: point.score,
                    file_path,
                    start_line,
                    language,
                    element_type, 
                    content,
                }
            })
            .collect();

        // Serialize the results to JSON
        let json_string = serde_json::to_string_pretty(&serializable_results)?;
        println!("{}", json_string);
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
            let snippet = payload_map.get(FIELD_CHUNK_CONTENT).and_then(|v| v.as_str()).map_or("[Error: Snippet content missing]", |v| v).to_string();

            println!(
                "{}. Score: {:.4} | File: {} | Line: {} | Lang: {} | Type: {}",
                (idx + 1).to_string().bold(),
                point.score,
                file_path.green(),
                start_line.to_string().yellow(),
                language,
                element_type
            );

            // Print the full chunk content as the snippet, indented
            println!("{}", "-".repeat(4));
            for line in snippet.lines() {
                println!("  {}", line);
            }
            println!("{}", "=".repeat(40)); // Separator between results
        }
    }
    Ok(())
} 