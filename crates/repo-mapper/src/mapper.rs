use crate::error::RepoMapperError;
use crate::scanners;
use crate::types::{MethodInfo, RepoMapOptions, RepoMapResult, RepoMapSummary};
use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use walkdir::WalkDir;

/// Main repository mapper that orchestrates the scanning process
pub struct RepoMapper {
    options: RepoMapOptions,
}

impl RepoMapper {
    /// Create a new repository mapper with the given options
    pub fn new(options: RepoMapOptions) -> Self {
        Self { options }
    }

    /// Scan a repository and generate a map
    pub fn scan_repository(&mut self, repo_path: &Path) -> Result<RepoMapResult, RepoMapperError> {
        if !repo_path.exists() {
            return Err(RepoMapperError::PathNotFound {
                path: repo_path.to_string_lossy().to_string(),
            });
        }

        // Determine which paths to scan
        let scan_paths = if let Some(ref paths) = self.options.paths {
            paths
                .iter()
                .map(|p| repo_path.join(p))
                .collect::<Vec<_>>()
        } else {
            vec![repo_path.to_path_buf()]
        };

        // Collect files to process
        let mut all_files = Vec::new();
        for scan_path in scan_paths {
            let files = self.collect_files(&scan_path)?;
            all_files.extend(files);
        }

        if all_files.is_empty() {
            return Err(RepoMapperError::NoFilesFound);
        }

        // Process files in parallel
        let results = Mutex::new(HashMap::new());
        all_files.par_iter().for_each(|file_path| {
            if let Ok(methods) = self.scan_file(file_path) {
                if !methods.is_empty() {
                    let relative_path = file_path
                        .strip_prefix(repo_path)
                        .unwrap_or(file_path)
                        .to_string_lossy()
                        .to_string();
                    let mut results_guard = results.lock().unwrap();
                    results_guard.insert(relative_path, methods);
                }
            }
        });

        let methods_by_file = results.into_inner().unwrap();

        // Generate formatted output and summary
        let map_content = self.format_results(&methods_by_file);
        let summary = self.generate_summary(&methods_by_file);

        Ok(RepoMapResult {
            map_content,
            summary,
            methods_by_file,
        })
    }

    /// Collect files to process based on options
    fn collect_files(&self, path: &Path) -> Result<Vec<std::path::PathBuf>, RepoMapperError> {
        let files = if let Some(ref pattern) = self.options.content_pattern {
            // Pattern-based file collection
            self.collect_files_by_pattern(path, pattern)?
        } else {
            // Standard file collection
            self.collect_all_files(path)?
        };

        // Filter by extension if specified
        let filtered_files = if let Some(ref ext) = self.options.file_extension {
            files
                .into_iter()
                .filter(|f| {
                    f.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e == ext)
                        .unwrap_or(false)
                })
                .collect()
        } else {
            files
        };

        Ok(filtered_files)
    }

    /// Collect files matching a content pattern
    fn collect_files_by_pattern(
        &self,
        path: &Path,
        pattern: &str,
    ) -> Result<Vec<std::path::PathBuf>, RepoMapperError> {
        let matcher = RegexMatcher::new(&regex::escape(pattern))
            .map_err(|e| RepoMapperError::PatternSearchError {
                message: e.to_string(),
            })?;

        let mut matching_files = Vec::new();

        for result in WalkBuilder::new(path)
            .hidden(false)
            .ignore(true)
            .build()
        {
            if let Ok(entry) = result {
                if entry.file_type().map_or(false, |ft| ft.is_file()) {
                    let file_path = entry.path();
                    if !self.is_ignored(file_path) && self.is_supported_file(file_path) {
                        if let Ok(content) = fs::read_to_string(file_path) {
                            if matcher
                                .find(content.as_bytes())
                                .unwrap_or(None)
                                .is_some()
                            {
                                matching_files.push(file_path.to_path_buf());
                            }
                        }
                    }
                }
            }
        }

        Ok(matching_files)
    }

    /// Collect all supported files
    fn collect_all_files(&self, path: &Path) -> Result<Vec<std::path::PathBuf>, RepoMapperError> {
        let files = WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| !self.is_ignored(e.path()))
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .filter(|p| self.is_supported_file(p))
            .collect();

        Ok(files)
    }

    /// Check if a path should be ignored
    fn is_ignored(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        [
            "node_modules",
            "vendor",
            "target",
            ".git",
            "build",
            "dist",
            ".next",
            "__pycache__",
            ".pytest_cache",
            "coverage",
        ]
        .iter()
        .any(|&ignored| path_str.contains(ignored))
    }

    /// Check if a file is supported for scanning
    fn is_supported_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            matches!(
                ext,
                "rb" | "js" | "ts" | "jsx" | "tsx" | "vue" | "go" | "rs" | "py" | "yaml" | "yml" | "md"
            )
        } else {
            false
        }
    }

    /// Scan a single file for methods
    fn scan_file(&self, path: &Path) -> Result<Vec<MethodInfo>, RepoMapperError> {
        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();
        let mut methods = Vec::new();
        let mut current_docstring = None;

        for (i, line) in lines.iter().enumerate() {
            let context = if self.options.include_context {
                self.get_context(&lines, i)
            } else {
                String::new()
            };

            // Track docstrings/comments for all languages
            if self.options.include_docstrings {
                if line.trim().starts_with("///")
                    || line.trim().starts_with("//")
                    || line.trim().starts_with('#')
                    || line.trim().starts_with("/*")
                    || line.trim().starts_with('*')
                    || line.trim().starts_with("\"\"\"")
                {
                    let doc = line
                        .trim_start_matches("///")
                        .trim_start_matches("//")
                        .trim_start_matches('#')
                        .trim_start_matches("/*")
                        .trim_start_matches('*')
                        .trim_start_matches("\"\"\"")
                        .trim();
                    current_docstring = Some(doc.to_string());
                }
            }

            // Determine file type and scan accordingly
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                match ext {
                    "ts" | "tsx" => {
                        scanners::typescript::scan_line(
                            line,
                            &context,
                            current_docstring.clone(),
                            &mut methods,
                            i + 1,
                            self.options.max_calls_per_method,
                        );
                    }
                    "rb" => {
                        scanners::ruby::scan_line(
                            line,
                            &context,
                            current_docstring.clone(),
                            &mut methods,
                            i + 1,
                            self.options.max_calls_per_method,
                        );
                    }
                    "js" | "jsx" => {
                        scanners::javascript::scan_line(
                            line,
                            &context,
                            current_docstring.clone(),
                            &mut methods,
                            i + 1,
                            self.options.max_calls_per_method,
                        );
                    }
                    "vue" => {
                        scanners::vue::scan_line(
                            line,
                            &context,
                            current_docstring.clone(),
                            &mut methods,
                            i + 1,
                            self.options.max_calls_per_method,
                        );
                    }
                    "go" => {
                        scanners::go::scan_line(
                            line,
                            &context,
                            current_docstring.clone(),
                            &mut methods,
                            i + 1,
                            self.options.max_calls_per_method,
                        );
                    }
                    "rs" => {
                        scanners::rust::scan_line(
                            line,
                            &context,
                            current_docstring.clone(),
                            &mut methods,
                            i + 1,
                            self.options.max_calls_per_method,
                        );
                    }
                    "py" => {
                        scanners::python::scan_line(
                            line,
                            &context,
                            current_docstring.clone(),
                            &mut methods,
                            i + 1,
                            self.options.max_calls_per_method,
                        );
                    }
                    "yaml" | "yml" => {
                        scanners::yaml::scan_line(
                            line,
                            &context,
                            current_docstring.clone(),
                            &mut methods,
                            i + 1,
                            self.options.max_calls_per_method,
                        );
                    }
                    "md" => {
                        scanners::markdown::scan_line(
                            line,
                            &context,
                            current_docstring.clone(),
                            &mut methods,
                            i + 1,
                            self.options.max_calls_per_method,
                        );
                    }
                    _ => {}
                }
            }
        }

        Ok(methods)
    }

    /// Get context lines around a specific line
    fn get_context(&self, lines: &[&str], current_line: usize) -> String {
        let start = current_line.saturating_sub(2);
        let end = (current_line + 3).min(lines.len());
        lines[start..end].join("\n")
    }

    /// Format the results into a readable map
    fn format_results(&self, methods_by_file: &HashMap<String, Vec<MethodInfo>>) -> String {
        let mut output = String::new();
        output.push_str("=== Repository Method Map ===\n");

        for (file, methods) in methods_by_file {
            if methods.is_empty() {
                continue;
            }

            output.push_str(&format!("\n📁 {}\n", file));

            for method in methods {
                let icon = method.method_type.icon();

                // Basic info (always shown)
                if self.options.verbosity == 0 {
                    // Minimal: just name
                    output.push_str(&format!("  {} {}\n", icon, method.name));
                } else {
                    // Normal/detailed: include signature
                    let params_display = if !method.params.is_empty() {
                        format!("({})", method.params)
                    } else {
                        String::new()
                    };
                    output.push_str(&format!("  {} {}{}\n", icon, method.name, params_display));

                    // Level 1 verbosity: Show docstrings and method calls
                    if self.options.verbosity >= 1 {
                        if let Some(doc) = &method.docstring {
                            output.push_str(&format!("    📝 {}\n", doc));
                        }
                        if !method.calls.is_empty() {
                            output.push_str(&format!("    📞 Calls: {}\n", method.calls.join(", ")));
                        }
                    }

                    // Level 2 verbosity: Show context
                    if self.options.verbosity >= 2 {
                        output.push_str("    📄 Context:\n");
                        for line in method.context.lines() {
                            output.push_str(&format!("       {}\n", line));
                        }
                    }
                }
            }
        }

        output
    }

    /// Generate summary statistics
    fn generate_summary(&self, methods_by_file: &HashMap<String, Vec<MethodInfo>>) -> RepoMapSummary {
        let mut file_type_counts = HashMap::new();
        let mut method_type_counts = HashMap::new();
        let mut languages_found = Vec::new();
        let mut total_methods = 0;

        for (file, methods) in methods_by_file {
            // Count file types
            let ext = if file.ends_with(".ts") || file.ends_with(".tsx") {
                "TypeScript"
            } else if file.ends_with(".rb") {
                "Ruby"
            } else if file.ends_with(".js") || file.ends_with(".jsx") {
                "JavaScript"
            } else if file.ends_with(".vue") {
                "Vue"
            } else if file.ends_with(".go") {
                "Go"
            } else if file.ends_with(".rs") {
                "Rust"
            } else if file.ends_with(".py") {
                "Python"
            } else if file.ends_with(".yaml") || file.ends_with(".yml") {
                "YAML"
            } else if file.ends_with(".md") {
                "Markdown"
            } else {
                "Other"
            };

            *file_type_counts.entry(ext.to_string()).or_insert(0) += 1;

            if !languages_found.contains(&ext.to_string()) {
                languages_found.push(ext.to_string());
            }

            // Count method types
            for method in methods {
                let method_type_name = method.method_type.display_name();
                *method_type_counts
                    .entry(method_type_name.to_string())
                    .or_insert(0) += 1;
                total_methods += 1;
            }
        }

        languages_found.sort();

        RepoMapSummary {
            files_scanned: methods_by_file.len(),
            total_methods,
            file_type_counts,
            method_type_counts,
            languages_found,
        }
    }
} 