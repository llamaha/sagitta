use anyhow::Result;
use glob::glob;
use std::fs;
use std::path::{Path, PathBuf};

pub fn find_files_matching_pattern(base_path: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let full_pattern = base_path.join(pattern);
    let pattern_str = full_pattern.to_string_lossy();
    
    let mut matches = Vec::new();
    for entry in glob(&pattern_str)? {
        if let Ok(path) = entry {
            if path.is_file() {
                matches.push(path);
            }
        }
    }
    
    Ok(matches)
}

pub fn read_file_range(path: &Path, start_line: usize, end_line: usize) -> Result<String> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    
    let start = start_line.saturating_sub(1);
    let end = end_line.min(lines.len());
    
    if start >= lines.len() {
        return Ok(String::new());
    }
    
    let selected_lines = &lines[start..end];
    Ok(selected_lines.join("\n"))
}