use crate::error::RepoMapperError;
use crate::scanners;
use crate::types::{MethodInfo, PaginationInfo, RepoMapOptions, RepoMapResult, RepoMapSummary};
use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use lru::LruCache;
use once_cell::sync::Lazy;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;

// Global cache for paginated results
static PAGINATION_CACHE: Lazy<Arc<Mutex<LruCache<String, CachedMapResult>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(100).unwrap()))));

#[derive(Clone)]
struct CachedMapResult {
    methods_by_file: HashMap<String, Vec<MethodInfo>>,
    summary: RepoMapSummary,
    timestamp: std::time::Instant,
}

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
        let summary = self.generate_summary(&methods_by_file);

        // Generate cache key if pagination might be needed
        let cache_key = if self.options.files_per_page.is_some() || 
                          self.estimate_total_lines(&methods_by_file) > self.options.max_output_lines.unwrap_or(usize::MAX) {
            Some(self.generate_cache_key(repo_path))
        } else {
            None
        };

        // Store in cache if needed
        if let Some(ref key) = cache_key {
            let cached_result = CachedMapResult {
                methods_by_file: methods_by_file.clone(),
                summary: summary.clone(),
                timestamp: std::time::Instant::now(),
            };
            
            let mut cache = PAGINATION_CACHE.lock().unwrap();
            cache.put(key.clone(), cached_result);
        }

        // Generate formatted output with pagination
        let (map_content, pagination) = self.format_results_paginated(
            &methods_by_file,
            self.options.page,
            self.options.files_per_page,
        );

        Ok(RepoMapResult {
            map_content,
            summary,
            methods_by_file: if pagination.is_none() { methods_by_file } else { HashMap::new() },
            pagination,
            cache_key,
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

        for entry in WalkBuilder::new(path)
            .hidden(false)
            .ignore(true)
            .build()
            .flatten()
        {
            if entry.file_type().is_some_and(|ft| ft.is_file()) {
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
            if self.options.include_docstrings && (line.trim().starts_with("///")
                    || line.trim().starts_with("//")
                    || line.trim().starts_with('#')
                    || line.trim().starts_with("/*")
                    || line.trim().starts_with('*')
                    || line.trim().starts_with("\"\"\""))
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

    /// Format the results into a readable map with pagination support
    fn format_results_paginated(
        &self, 
        methods_by_file: &HashMap<String, Vec<MethodInfo>>,
        page: Option<usize>,
        files_per_page: Option<usize>,
    ) -> (String, Option<PaginationInfo>) {
        // Sort files intelligently if smart_sort is enabled
        let sorted_files = if self.options.smart_sort {
            self.smart_sort_files(methods_by_file)
        } else {
            let mut files: Vec<_> = methods_by_file.keys().cloned().collect();
            files.sort();
            files
        };

        // Calculate pagination
        let total_files = sorted_files.len();
        let (files_to_display, pagination_info) = if let Some(fpp) = files_per_page {
            let current_page = page.unwrap_or(1).max(1);
            let total_pages = total_files.div_ceil(fpp);
            let start_idx = (current_page - 1) * fpp;
            let end_idx = (start_idx + fpp).min(total_files);
            
            let files_in_page: Vec<String> = sorted_files[start_idx..end_idx].to_vec();
            
            let pagination = PaginationInfo {
                total_files,
                total_lines: self.estimate_total_lines(methods_by_file),
                files_per_page: fpp,
                current_page,
                total_pages,
                has_next: current_page < total_pages,
                has_previous: current_page > 1,
                files_in_page: files_in_page.clone(),
            };
            
            (files_in_page, Some(pagination))
        } else {
            // Check if we need auto-pagination based on line count
            let estimated_lines = self.estimate_total_lines(methods_by_file);
            if let Some(max_lines) = self.options.max_output_lines {
                if estimated_lines > max_lines {
                    // Auto-enable pagination
                    let files_per_page = self.calculate_files_per_page(methods_by_file, max_lines);
                    return self.format_results_paginated(methods_by_file, Some(1), Some(files_per_page));
                }
            }
            (sorted_files, None)
        };

        // Generate output for selected files
        let mut output = String::new();
        output.push_str("=== Repository Method Map ===\n");
        
        if let Some(ref pagination) = pagination_info {
            output.push_str(&format!(
                "\n📄 Page {} of {} (Files {}-{} of {})\n",
                pagination.current_page,
                pagination.total_pages,
                ((pagination.current_page - 1) * pagination.files_per_page) + 1,
                ((pagination.current_page - 1) * pagination.files_per_page) + pagination.files_in_page.len(),
                pagination.total_files
            ));
        }

        for file in files_to_display {
            if let Some(methods) = methods_by_file.get(&file) {
                self.format_file_methods(&mut output, &file, methods);
            }
        }

        (output, pagination_info)
    }

    /// Format methods for a single file
    fn format_file_methods(&self, output: &mut String, file: &str, methods: &[MethodInfo]) {
        if methods.is_empty() {
            return;
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

    /// Smart sort files to show most important ones first
    fn smart_sort_files(&self, methods_by_file: &HashMap<String, Vec<MethodInfo>>) -> Vec<String> {
        let mut files: Vec<(String, i32)> = methods_by_file.keys().map(|f| {
            let mut priority = 0;
            
            // Entry point patterns
            if f.contains("main") || f.contains("index") || f.contains("app") {
                priority += 100;
            }
            
            // Configuration files
            if f.ends_with("config") || f.contains("settings") {
                priority += 50;
            }
            
            // Test files (lower priority)
            if f.contains("test") || f.contains("spec") {
                priority -= 50;
            }
            
            // Method count (more methods = higher priority)
            if let Some(methods) = methods_by_file.get(f) {
                priority += methods.len() as i32;
            }
            
            (f.clone(), priority)
        }).collect();
        
        // Sort by priority (descending) then by name
        files.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
        });
        
        files.into_iter().map(|(f, _)| f).collect()
    }

    /// Estimate total lines for the output
    fn estimate_total_lines(&self, methods_by_file: &HashMap<String, Vec<MethodInfo>>) -> usize {
        let mut lines = 3; // Header lines
        
        for methods in methods_by_file.values() {
            lines += 2; // File header
            for method in methods {
                lines += 1; // Method line
                if self.options.verbosity >= 1 {
                    if method.docstring.is_some() {
                        lines += 1;
                    }
                    if !method.calls.is_empty() {
                        lines += 1;
                    }
                }
                if self.options.verbosity >= 2 {
                    lines += method.context.lines().count() + 1;
                }
            }
        }
        
        lines
    }

    /// Calculate appropriate files per page based on max lines
    fn calculate_files_per_page(&self, methods_by_file: &HashMap<String, Vec<MethodInfo>>, max_lines: usize) -> usize {
        let avg_lines_per_file = self.estimate_total_lines(methods_by_file) / methods_by_file.len().max(1);
        (max_lines / avg_lines_per_file).max(5) // At least 5 files per page
    }

    /// Generate a cache key for the repository scan
    fn generate_cache_key(&self, repo_path: &Path) -> String {
        let mut hasher = DefaultHasher::new();
        repo_path.hash(&mut hasher);
        self.options.file_extension.hash(&mut hasher);
        self.options.content_pattern.hash(&mut hasher);
        self.options.paths.hash(&mut hasher);
        
        format!("repo_map_{:x}", hasher.finish())
    }
}

/// New public function to retrieve a specific page from cache
pub fn get_cached_page(
    cache_key: &str,
    page: usize,
    files_per_page: usize,
) -> Result<RepoMapResult, RepoMapperError> {
    let cache = PAGINATION_CACHE.lock().unwrap();
    
    if let Some(cached) = cache.peek(cache_key) {
        // Check if cache is still fresh (15 minutes)
        if cached.timestamp.elapsed() < std::time::Duration::from_secs(900) {
            let mapper = RepoMapper::new(RepoMapOptions {
                page: Some(page),
                files_per_page: Some(files_per_page),
                ..Default::default()
            });
            
            let (map_content, pagination) = mapper.format_results_paginated(
                &cached.methods_by_file,
                Some(page),
                Some(files_per_page),
            );
            
            return Ok(RepoMapResult {
                map_content,
                summary: cached.summary.clone(),
                methods_by_file: HashMap::new(),
                pagination,
                cache_key: Some(cache_key.to_string()),
            });
        }
    }
    
    Err(RepoMapperError::CacheExpired(cache_key.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::generate_repo_map;
    
    #[test]
    fn test_auto_pagination_triggers() {
        // Create a repo with many files
        let temp_dir = tempdir().unwrap();
        for i in 0..50 {
            let file = temp_dir.path().join(format!("file{}.rs", i));
            fs::write(&file, format!("fn func{}() {{}}", i)).unwrap();
        }
        
        let options = RepoMapOptions {
            max_output_lines: Some(100),
            ..Default::default()
        };
        
        let result = generate_repo_map(temp_dir.path(), options).unwrap();
        assert!(result.pagination.is_some());
        assert!(result.cache_key.is_some());
        
        let pagination = result.pagination.unwrap();
        assert!(pagination.total_files >= 50);
        assert!(pagination.has_next);
        assert_eq!(pagination.current_page, 1);
    }
    
    #[test]
    fn test_manual_pagination() {
        let temp_dir = tempdir().unwrap();
        for i in 0..30 {
            let file = temp_dir.path().join(format!("test{}.py", i));
            fs::write(&file, format!("def test_func_{}():\n    pass", i)).unwrap();
        }
        
        // Test with explicit pagination parameters
        let options = RepoMapOptions {
            files_per_page: Some(10),
            page: Some(2),
            ..Default::default()
        };
        
        let result = generate_repo_map(temp_dir.path(), options).unwrap();
        assert!(result.pagination.is_some());
        
        let pagination = result.pagination.unwrap();
        assert_eq!(pagination.files_per_page, 10);
        assert_eq!(pagination.current_page, 2);
        assert_eq!(pagination.total_pages, 3);
        assert!(pagination.has_previous);
        assert!(pagination.has_next);
        assert_eq!(pagination.files_in_page.len(), 10);
    }
    
    #[test]
    fn test_smart_sorting() {
        let temp_dir = tempdir().unwrap();
        
        // Create files with different priority patterns
        fs::write(temp_dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("index.js"), "function index() {}").unwrap();
        fs::write(temp_dir.path().join("app.py"), "def app(): pass").unwrap();
        fs::write(temp_dir.path().join("config.rs"), "struct Config {}").unwrap();
        fs::write(temp_dir.path().join("test_utils.rs"), "fn test() {}").unwrap();
        fs::write(temp_dir.path().join("regular.rs"), "fn regular() {}").unwrap();
        
        let options = RepoMapOptions {
            smart_sort: true,
            files_per_page: Some(10),
            ..Default::default()
        };
        
        let result = generate_repo_map(temp_dir.path(), options).unwrap();
        let pagination = result.pagination.unwrap();
        
        // Check that high-priority files appear first
        let files = &pagination.files_in_page;
        
        // main.rs, index.js, and app.py should be among the first files
        let main_index = files.iter().position(|f| f.contains("main.rs"));
        let index_index = files.iter().position(|f| f.contains("index.js"));
        let app_index = files.iter().position(|f| f.contains("app.py"));
        let test_index = files.iter().position(|f| f.contains("test_utils.rs"));
        
        assert!(main_index.is_some());
        assert!(index_index.is_some());
        assert!(app_index.is_some());
        assert!(test_index.is_some());
        
        // Test files should appear later than main/index/app files
        if let (Some(main_pos), Some(test_pos)) = (main_index, test_index) {
            assert!(main_pos < test_pos);
        }
    }
    
    #[test]
    fn test_no_pagination_small_output() {
        let temp_dir = tempdir().unwrap();
        
        // Create just a few files
        for i in 0..3 {
            let file = temp_dir.path().join(format!("small{}.js", i));
            fs::write(&file, format!("function small{}() {{}}", i)).unwrap();
        }
        
        let options = RepoMapOptions {
            max_output_lines: Some(1000), // High limit
            ..Default::default()
        };
        
        let result = generate_repo_map(temp_dir.path(), options).unwrap();
        assert!(result.pagination.is_none());
        assert!(result.cache_key.is_none());
        assert!(!result.methods_by_file.is_empty());
    }
} 