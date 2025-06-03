//! File processing implementation for CPU-intensive tasks.

use crate::error::{Result, SagittaEmbedError};
use crate::processor::{
    FileProcessor, ProcessedChunk, ChunkMetadata, ProcessingConfig,
    ProgressReporter, ProcessingProgress, ProcessingStage, NoOpProgressReporter
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::task;
use futures::future::try_join_all;
use uuid::Uuid;

/// Default implementation of FileProcessor that handles file I/O, parsing, and chunking.
/// This is designed to scale to maximum CPU cores without GPU memory concerns.
pub struct DefaultFileProcessor {
    config: ProcessingConfig,
    /// Optional syntax parser integration for more sophisticated element type detection
    /// When None, falls back to simple heuristic-based parsing
    syntax_parser_fn: Option<Arc<dyn Fn(&std::path::Path) -> Result<Vec<ParsedChunk>> + Send + Sync>>,
}

impl std::fmt::Debug for DefaultFileProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultFileProcessor")
            .field("config", &self.config)
            .field("syntax_parser_fn", &self.syntax_parser_fn.as_ref().map(|_| "Some(...)"))
            .finish()
    }
}

impl DefaultFileProcessor {
    /// Create a new file processor with the given configuration.
    pub fn new(config: ProcessingConfig) -> Self {
        Self { 
            config,
            syntax_parser_fn: None,
        }
    }

    /// Create a new file processor with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ProcessingConfig::default())
    }

    /// Update the configuration.
    pub fn with_config(mut self, config: ProcessingConfig) -> Self {
        self.config = config;
        self
    }

    /// Set a custom syntax parser function for more sophisticated parsing.
    /// This allows integration with external syntax parsing systems.
    pub fn with_syntax_parser<F>(mut self, parser_fn: F) -> Self 
    where
        F: Fn(&std::path::Path) -> Result<Vec<ParsedChunk>> + Send + Sync + 'static,
    {
        self.syntax_parser_fn = Some(Arc::new(parser_fn));
        self
    }

    /// Process a single file synchronously (internal helper).
    fn process_file_sync(&self, file_path: &PathBuf) -> Result<Vec<ProcessedChunk>> {
        // Check file size first
        let metadata = std::fs::metadata(file_path).map_err(|e| {
            SagittaEmbedError::file_system(format!(
                "Failed to get metadata for {}: {}",
                file_path.display(),
                e
            ))
        })?;

        if metadata.len() > self.config.max_file_size_bytes {
            return Err(SagittaEmbedError::invalid_input(format!(
                "File {} exceeds maximum size of {} bytes",
                file_path.display(),
                self.config.max_file_size_bytes
            )));
        }

        if !metadata.is_file() {
            return Err(SagittaEmbedError::invalid_input(format!(
                "Path is not a file: {}",
                file_path.display()
            )));
        }

        // Extract file extension
        let file_extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();

        // Determine language from extension
        let language = detect_language_from_extension(&file_extension);

        // Parse content into chunks using syntax parser if available, otherwise fallback
        let chunks = if let Some(ref parser_fn) = self.syntax_parser_fn {
            parser_fn(file_path).map_err(|e| {
                SagittaEmbedError::file_system(format!(
                    "Syntax parsing failed for {}: {}",
                    file_path.display(),
                    e
                ))
            })?
        } else {
            // Fallback to simple parsing if no syntax parser provided
            let content = std::fs::read_to_string(file_path).map_err(|e| {
                SagittaEmbedError::file_system(format!(
                    "Failed to read file {}: {}",
                    file_path.display(),
                    e
                ))
            })?;
            parse_content_into_chunks(&content, &language)?
        };

        // Convert to ProcessedChunk instances
        let processed_chunks: Vec<ProcessedChunk> = chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| {
                let metadata = ChunkMetadata {
                    file_path: file_path.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    language: chunk.language,
                    file_extension: file_extension.clone(),
                    element_type: chunk.element_type,
                    context: None,
                };

                ProcessedChunk {
                    content: chunk.content,
                    metadata,
                    id: format!("{}_{}", Uuid::new_v4(), i),
                }
            })
            .collect();

        Ok(processed_chunks)
    }
}

#[async_trait::async_trait]
impl FileProcessor for DefaultFileProcessor {
    async fn process_file(&self, file_path: &PathBuf) -> Result<Vec<ProcessedChunk>> {
        let file_path = file_path.clone();
        let config = self.config.clone();
        let syntax_parser_fn = self.syntax_parser_fn.clone();
        
        // Run file processing in a blocking task to avoid blocking the async runtime
        task::spawn_blocking(move || {
            let mut processor = DefaultFileProcessor::new(config);
            processor.syntax_parser_fn = syntax_parser_fn;
            processor.process_file_sync(&file_path)
        })
        .await
        .map_err(|e| SagittaEmbedError::thread_safety(format!("File processing task failed: {}", e)))?
    }

    async fn process_files(&self, file_paths: &[PathBuf]) -> Result<Vec<ProcessedChunk>> {
        self.process_files_with_progress(file_paths, Arc::new(NoOpProgressReporter)).await
    }

    async fn process_files_with_progress(
        &self, 
        file_paths: &[PathBuf], 
        progress_reporter: Arc<dyn ProgressReporter>
    ) -> Result<Vec<ProcessedChunk>> {
        if file_paths.is_empty() {
            return Ok(Vec::new());
        }

        let start_time = Instant::now();
        let total_files = file_paths.len();
        
        // Report starting
        progress_reporter.report(ProcessingProgress {
            stage: ProcessingStage::Starting,
            current_file: None,
            files_completed: 0,
            total_files,
            files_per_second: None,
            message: Some(format!("Starting to process {} files", total_files)),
        }).await;

        // Create semaphore to limit concurrent file processing
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.config.file_processing_concurrency));
        let files_completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        
        // Process files concurrently with controlled concurrency
        let tasks: Vec<_> = file_paths
            .iter()
            .enumerate()
            .map(|(index, file_path)| {
                let file_path = file_path.clone();
                let config = self.config.clone();
                let syntax_parser_fn = self.syntax_parser_fn.clone();
                let semaphore = semaphore.clone();
                let progress_reporter = progress_reporter.clone();
                let files_completed = files_completed.clone();
                
                async move {
                    let _permit = semaphore.acquire().await.map_err(|e| {
                        SagittaEmbedError::thread_safety(format!("Failed to acquire semaphore: {}", e))
                    })?;
                    
                    // Report current file being processed
                    progress_reporter.report(ProcessingProgress {
                        stage: ProcessingStage::ProcessingFiles,
                        current_file: Some(file_path.clone()),
                        files_completed: files_completed.load(std::sync::atomic::Ordering::Relaxed),
                        total_files,
                        files_per_second: None,
                        message: Some(format!("Processing file {} of {}", index + 1, total_files)),
                    }).await;
                    
                    // Process file in blocking task
                    let result = task::spawn_blocking(move || {
                        let mut processor = DefaultFileProcessor::new(config);
                        processor.syntax_parser_fn = syntax_parser_fn;
                        processor.process_file_sync(&file_path)
                    })
                    .await
                    .map_err(|e| SagittaEmbedError::thread_safety(format!("File processing task failed: {}", e)))?;
                    
                    // Update completed count and report progress
                    let completed = files_completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    let elapsed_secs = start_time.elapsed().as_secs_f64();
                    let files_per_second = if elapsed_secs > 0.0 { Some(completed as f64 / elapsed_secs) } else { None };
                    
                    progress_reporter.report(ProcessingProgress {
                        stage: ProcessingStage::ProcessingFiles,
                        current_file: None,
                        files_completed: completed,
                        total_files,
                        files_per_second,
                        message: Some(format!("Completed {} of {} files", completed, total_files)),
                    }).await;
                    
                    result
                }
            })
            .collect();

        // Wait for all tasks to complete and collect results
        let results = try_join_all(tasks).await?;
        
        // Flatten all chunks into a single vector
        let mut all_chunks = Vec::new();
        for chunks in results {
            all_chunks.extend(chunks);
        }

        // Report completion
        let elapsed_secs = start_time.elapsed().as_secs_f64();
        let files_per_second = if elapsed_secs > 0.0 { Some(total_files as f64 / elapsed_secs) } else { None };
        
        progress_reporter.report(ProcessingProgress {
            stage: ProcessingStage::Completed,
            current_file: None,
            files_completed: total_files,
            total_files,
            files_per_second,
            message: Some(format!("Successfully processed {} files, generated {} chunks", total_files, all_chunks.len())),
        }).await;

        Ok(all_chunks)
    }

    fn config(&self) -> &ProcessingConfig {
        &self.config
    }
}

/// Internal chunk representation during parsing.
#[derive(Debug, Clone)]
pub struct ParsedChunk {
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub element_type: String,
}

/// Detect programming language from file extension.
fn detect_language_from_extension(extension: &str) -> String {
    match extension.to_lowercase().as_str() {
        "rs" => "rust".to_string(),
        "py" => "python".to_string(),
        "js" | "jsx" => "javascript".to_string(),
        "ts" | "tsx" => "typescript".to_string(),
        "java" => "java".to_string(),
        "cpp" | "cc" | "cxx" | "c++" => "cpp".to_string(),
        "c" | "h" => "c".to_string(),
        "go" => "go".to_string(),
        "rb" => "ruby".to_string(),
        "php" => "php".to_string(),
        "cs" => "csharp".to_string(),
        "swift" => "swift".to_string(),
        "kt" | "kts" => "kotlin".to_string(),
        "scala" => "scala".to_string(),
        "clj" | "cljs" | "cljc" => "clojure".to_string(),
        "hs" => "haskell".to_string(),
        "ml" | "mli" => "ocaml".to_string(),
        "fs" | "fsi" | "fsx" => "fsharp".to_string(),
        "elm" => "elm".to_string(),
        "ex" | "exs" => "elixir".to_string(),
        "erl" | "hrl" => "erlang".to_string(),
        "lua" => "lua".to_string(),
        "pl" | "pm" => "perl".to_string(),
        "r" => "r".to_string(),
        "m" => "matlab".to_string(),
        "sh" | "bash" | "zsh" => "shell".to_string(),
        "ps1" => "powershell".to_string(),
        "sql" => "sql".to_string(),
        "html" | "htm" => "html".to_string(),
        "css" => "css".to_string(),
        "scss" | "sass" => "scss".to_string(),
        "xml" => "xml".to_string(),
        "json" => "json".to_string(),
        "yaml" | "yml" => "yaml".to_string(),
        "toml" => "toml".to_string(),
        "ini" => "ini".to_string(),
        "md" | "markdown" => "markdown".to_string(),
        "tex" => "latex".to_string(),
        "dockerfile" => "dockerfile".to_string(),
        "makefile" | "mk" => "makefile".to_string(),
        "cmake" => "cmake".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Parse file content into logical chunks.
/// This is a simplified implementation - in practice, you might want to integrate
/// with the existing syntax parsing logic from the main codebase.
fn parse_content_into_chunks(content: &str, language: &str) -> Result<Vec<ParsedChunk>> {
    let lines: Vec<&str> = content.lines().collect();
    
    if lines.is_empty() {
        return Ok(Vec::new());
    }

    // For now, implement a simple line-based chunking strategy
    // This can be enhanced with language-specific parsing later
    let mut chunks = Vec::new();
    let chunk_size = 50; // Lines per chunk
    
    for (chunk_idx, line_chunk) in lines.chunks(chunk_size).enumerate() {
        let start_line = chunk_idx * chunk_size + 1;
        let end_line = start_line + line_chunk.len() - 1;
        let content = line_chunk.join("\n");
        
        // Skip empty chunks
        if content.trim().is_empty() {
            continue;
        }
        
        chunks.push(ParsedChunk {
            content: content.clone(),
            start_line,
            end_line,
            language: language.to_string(),
            element_type: detect_element_type(&content, language),
        });
    }

    Ok(chunks)
}

/// Detect the type of code element in a chunk.
fn detect_element_type(content: &str, language: &str) -> String {
    // Simple heuristic-based detection
    let trimmed = content.trim();
    
    match language {
        "rust" => {
            if trimmed.contains("fn ") {
                "function".to_string()
            } else if trimmed.contains("struct ") {
                "struct".to_string()
            } else if trimmed.contains("enum ") {
                "enum".to_string()
            } else if trimmed.contains("impl ") {
                "implementation".to_string()
            } else if trimmed.contains("mod ") {
                "module".to_string()
            } else {
                "code".to_string()
            }
        }
        "python" => {
            if trimmed.contains("def ") {
                "function".to_string()
            } else if trimmed.contains("class ") {
                "class".to_string()
            } else {
                "code".to_string()
            }
        }
        "javascript" | "typescript" => {
            if trimmed.contains("function ") || trimmed.contains("const ") && trimmed.contains("=>") {
                "function".to_string()
            } else if trimmed.contains("class ") {
                "class".to_string()
            } else {
                "code".to_string()
            }
        }
        _ => "code".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_detect_language_from_extension() {
        assert_eq!(detect_language_from_extension("rs"), "rust");
        assert_eq!(detect_language_from_extension("py"), "python");
        assert_eq!(detect_language_from_extension("js"), "javascript");
        assert_eq!(detect_language_from_extension("unknown"), "unknown");
    }

    #[test]
    fn test_detect_element_type() {
        assert_eq!(detect_element_type("fn main() {}", "rust"), "function");
        assert_eq!(detect_element_type("struct User {}", "rust"), "struct");
        assert_eq!(detect_element_type("def hello():", "python"), "function");
        assert_eq!(detect_element_type("class Test:", "python"), "class");
    }

    #[tokio::test]
    async fn test_process_single_file() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        
        let content = r#"
fn main() {
    println!("Hello, world!");
}

struct User {
    name: String,
}
"#;
        fs::write(&file_path, content).unwrap();
        
        let processor = DefaultFileProcessor::with_defaults();
        let chunks = processor.process_file(&file_path).await.unwrap();
        
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].metadata.language, "rust");
        assert_eq!(chunks[0].metadata.file_extension, "rs");
    }

    #[tokio::test]
    async fn test_process_multiple_files() {
        let temp_dir = tempdir().unwrap();
        
        let file1 = temp_dir.path().join("test1.py");
        let file2 = temp_dir.path().join("test2.js");
        
        fs::write(&file1, "def hello():\n    print('Hello')").unwrap();
        fs::write(&file2, "function hello() {\n    console.log('Hello');\n}").unwrap();
        
        let processor = DefaultFileProcessor::with_defaults();
        let chunks = processor.process_files(&[file1, file2]).await.unwrap();
        
        assert!(!chunks.is_empty());
        // Should have chunks from both files
        let languages: std::collections::HashSet<_> = chunks.iter()
            .map(|c| &c.metadata.language)
            .collect();
        assert!(languages.contains(&"python".to_string()));
        assert!(languages.contains(&"javascript".to_string()));
    }

    #[tokio::test]
    async fn test_file_size_limit() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("large.txt");
        
        // Create a file larger than the default limit
        let large_content = "x".repeat(10 * 1024 * 1024); // 10MB
        fs::write(&file_path, large_content).unwrap();
        
        let config = ProcessingConfig {
            max_file_size_bytes: 1024, // 1KB limit
            ..Default::default()
        };
        
        let processor = DefaultFileProcessor::new(config);
        let result = processor.process_file(&file_path).await;
        
        assert!(result.is_err());
    }
} 