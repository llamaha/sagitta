//! File processing implementation for CPU-intensive tasks with optimized threading.

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
/// Optimized for coordination with the embedding pool.
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
    /// Optimized for minimal memory allocation and CPU overhead.
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

        // Convert to ProcessedChunk instances with optimized memory usage
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

    /// Process files in optimized batches to reduce coordination overhead.
    async fn process_files_batched(
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
            message: Some(format!("Starting optimized file processing for {} files using {} CPU cores", 
                                  total_files, self.config.file_processing_concurrency)),
        }).await;

        // Calculate optimal batch size to reduce coordination overhead
        // Larger batches reduce task spawning overhead but may increase memory usage
        let optimal_batch_size = std::cmp::max(
            1,
            std::cmp::min(
                total_files / self.config.file_processing_concurrency,
                32 // Cap batch size to prevent excessive memory usage
            )
        );

        log::debug!("Using batch size {} for {} files with {} CPU cores", 
                    optimal_batch_size, total_files, self.config.file_processing_concurrency);

        // Create batches of files
        let file_batches: Vec<Vec<PathBuf>> = file_paths
            .chunks(optimal_batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        // Create semaphore to limit concurrent file processing
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.config.file_processing_concurrency));
        let files_completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        
        // Process file batches concurrently
        let tasks: Vec<_> = file_batches
            .into_iter()
            .enumerate()
            .map(|(batch_idx, file_batch)| {
                let config = self.config.clone();
                let syntax_parser_fn = self.syntax_parser_fn.clone();
                let semaphore = semaphore.clone();
                let progress_reporter = progress_reporter.clone();
                let files_completed = files_completed.clone();
                let batch_size = file_batch.len();
                
                async move {
                    let _permit = semaphore.acquire().await.map_err(|e| {
                        SagittaEmbedError::thread_safety(format!("Failed to acquire semaphore: {}", e))
                    })?;
                    
                    // Process entire batch in a single blocking task to reduce task overhead
                    let result = task::spawn_blocking(move || {
                        let mut processor = DefaultFileProcessor::new(config);
                        processor.syntax_parser_fn = syntax_parser_fn;
                        
                        let mut batch_chunks = Vec::new();
                        for file_path in file_batch {
                            match processor.process_file_sync(&file_path) {
                                Ok(mut chunks) => batch_chunks.append(&mut chunks),
                                Err(e) => {
                                    log::warn!("Failed to process file {}: {}", file_path.display(), e);
                                    // Continue processing other files in the batch
                                }
                            }
                        }
                        batch_chunks
                    })
                    .await
                    .map_err(|e| SagittaEmbedError::thread_safety(format!("File processing batch task failed: {}", e)))?;
                    
                    // Update completed count and report progress
                    let completed = files_completed.fetch_add(batch_size, std::sync::atomic::Ordering::Relaxed) + batch_size;
                    let elapsed_secs = start_time.elapsed().as_secs_f64();
                    let files_per_second = if elapsed_secs > 0.0 { Some(completed as f64 / elapsed_secs) } else { None };
                    
                    progress_reporter.report(ProcessingProgress {
                        stage: ProcessingStage::ProcessingFiles,
                        current_file: None,
                        files_completed: completed,
                        total_files,
                        files_per_second,
                        message: Some(format!("Processed batch {} - completed {} of {} files ({:.1} files/sec)", 
                                              batch_idx + 1, completed, total_files, 
                                              files_per_second.unwrap_or(0.0))),
                    }).await;
                    
                    Ok::<Vec<ProcessedChunk>, SagittaEmbedError>(result)
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
            message: Some(format!("Successfully processed {} files, generated {} chunks ({:.1} files/sec)", 
                                  total_files, all_chunks.len(), files_per_second.unwrap_or(0.0))),
        }).await;

        Ok(all_chunks)
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
        // Use the optimized batched processing
        self.process_files_batched(file_paths, progress_reporter).await
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
        assert_eq!(detect_element_type("fn main() {", "rust"), "function");
        assert_eq!(detect_element_type("struct User {", "rust"), "struct");
        assert_eq!(detect_element_type("impl MyStruct {", "rust"), "implementation");
        assert_eq!(detect_element_type("mod my_module {", "rust"), "module");
        assert_eq!(detect_element_type("let x = 5;", "rust"), "code");
        assert_eq!(detect_element_type("random text", "rust"), "code");
    }

    #[tokio::test]
    async fn test_process_single_file() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {\n    println!(\"Hello, world!\");\n}").unwrap();

        let processor = DefaultFileProcessor::with_defaults();
        let chunks = processor.process_file(&file_path).await.unwrap();
        
        assert!(!chunks.is_empty());
        assert!(chunks[0].content.contains("main"));
        assert_eq!(chunks[0].metadata.language, "rust");
        assert_eq!(chunks[0].metadata.file_extension, "rs");
    }

    #[tokio::test]
    async fn test_process_multiple_files() {
        let temp_dir = tempdir().unwrap();
        let file1 = temp_dir.path().join("test1.rs");
        let file2 = temp_dir.path().join("test2.rs");
        
        fs::write(&file1, "fn hello() {}").unwrap();
        fs::write(&file2, "fn world() {}").unwrap();

        let processor = DefaultFileProcessor::with_defaults();
        let files = vec![file1, file2];
        let chunks = processor.process_files(&files).await.unwrap();
        
        assert!(chunks.len() >= 2);
    }

    #[tokio::test]
    async fn test_file_size_limit() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("large.txt");
        
        // Create a file larger than the default limit
        let large_content = "x".repeat(100 * 1024 * 1024); // 100MB
        fs::write(&file_path, large_content).unwrap();

        let processor = DefaultFileProcessor::with_defaults();
        let result = processor.process_file(&file_path).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum size"));
    }

    #[test]
    fn test_processor_creation_and_configuration() {
        let mut config = ProcessingConfig::default();
        config.file_processing_concurrency = 8;
        config.max_file_size_bytes = 50 * 1024 * 1024;

        let processor = DefaultFileProcessor::new(config.clone());
        assert_eq!(processor.config().file_processing_concurrency, 8);
        assert_eq!(processor.config().max_file_size_bytes, 50 * 1024 * 1024);

        let processor_with_defaults = DefaultFileProcessor::with_defaults();
        assert_eq!(processor_with_defaults.config().file_processing_concurrency, ProcessingConfig::default().file_processing_concurrency);
    }

    #[test]
    fn test_processor_with_config() {
        let original_config = ProcessingConfig::default();
        let mut new_config = ProcessingConfig::default();
        new_config.file_processing_concurrency = 16;

        let processor = DefaultFileProcessor::new(original_config)
            .with_config(new_config.clone());
        
        assert_eq!(processor.config().file_processing_concurrency, 16);
    }

    #[tokio::test]
    async fn test_process_nonexistent_file() {
        let processor = DefaultFileProcessor::with_defaults();
        let nonexistent_path = PathBuf::from("/nonexistent/file.rs");
        
        let result = processor.process_file(&nonexistent_path).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to get metadata"));
    }

    #[tokio::test]
    async fn test_process_directory_as_file() {
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path().to_path_buf();

        let processor = DefaultFileProcessor::with_defaults();
        let result = processor.process_file(&dir_path).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Path is not a file"));
    }

    #[tokio::test]
    async fn test_process_empty_file_list() {
        let processor = DefaultFileProcessor::with_defaults();
        let files: Vec<PathBuf> = vec![];
        let chunks = processor.process_files(&files).await.unwrap();
        
        assert!(chunks.is_empty());
    }

    #[tokio::test]
    async fn test_process_files_with_progress() {
        let temp_dir = tempdir().unwrap();
        let file1 = temp_dir.path().join("test1.rs");
        let file2 = temp_dir.path().join("test2.py");
        
        fs::write(&file1, "fn main() {}").unwrap();
        fs::write(&file2, "def main(): pass").unwrap();

        let processor = DefaultFileProcessor::with_defaults();
        let progress_reporter = Arc::new(NoOpProgressReporter);
        let files = vec![file1, file2];
        
        let chunks = processor.process_files_with_progress(&files, progress_reporter).await.unwrap();
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_parse_content_into_chunks() {
        let content = "fn hello() {\n    println!(\"Hello\");\n}\n\nfn world() {\n    println!(\"World\");\n}";
        let chunks = parse_content_into_chunks(content, "rust").unwrap();
        
        assert!(chunks.len() >= 1);
        assert!(chunks[0].content.contains("hello") || chunks[0].content.contains("world"));
        assert_eq!(chunks[0].language, "rust");
    }

    #[test]
    fn test_parse_empty_content() {
        let chunks = parse_content_into_chunks("", "rust").unwrap();
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_parse_whitespace_only_content() {
        let chunks = parse_content_into_chunks("   \n\n   \t   ", "rust").unwrap();
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_detect_element_type_comprehensive() {
        // Test Rust patterns
        assert_eq!(detect_element_type("pub fn test() {", "rust"), "function");
        assert_eq!(detect_element_type("async fn async_test() {", "rust"), "function");
        assert_eq!(detect_element_type("const fn const_test() {", "rust"), "function");
        assert_eq!(detect_element_type("unsafe fn unsafe_test() {", "rust"), "function");
        assert_eq!(detect_element_type("pub struct MyStruct {", "rust"), "struct");
        assert_eq!(detect_element_type("pub enum MyEnum {", "rust"), "enum");
        assert_eq!(detect_element_type("pub trait MyTrait {", "rust"), "code"); // trait not supported
        assert_eq!(detect_element_type("mod my_module {", "rust"), "module");
        assert_eq!(detect_element_type("macro_rules! my_macro {", "rust"), "code"); // macro not supported
        assert_eq!(detect_element_type("type MyType = String;", "rust"), "code"); // type not supported
        assert_eq!(detect_element_type("static MY_STATIC: i32 = 42;", "rust"), "code"); // static not supported
        assert_eq!(detect_element_type("const MY_CONST: i32 = 42;", "rust"), "code"); // const not supported

        // Test Python patterns
        assert_eq!(detect_element_type("def my_function():", "python"), "function");
        assert_eq!(detect_element_type("class MyClass:", "python"), "class");
        assert_eq!(detect_element_type("async def async_func():", "python"), "function");
        assert_eq!(detect_element_type("import os", "python"), "code"); // import not supported
        assert_eq!(detect_element_type("from sys import path", "python"), "code"); // import not supported

        // Test JavaScript patterns
        assert_eq!(detect_element_type("function myFunction() {", "javascript"), "function");
        assert_eq!(detect_element_type("const myFunc = () => {", "javascript"), "function");
        assert_eq!(detect_element_type("class MyClass {", "javascript"), "class");
        assert_eq!(detect_element_type("export default", "javascript"), "code"); // export not supported
        assert_eq!(detect_element_type("import React from", "javascript"), "code"); // import not supported

        // Test general patterns - all return "code" since they're not specifically detected
        assert_eq!(detect_element_type("/* block comment */", "rust"), "code");
        assert_eq!(detect_element_type("# Python comment", "python"), "code");
        assert_eq!(detect_element_type("// Line comment", "javascript"), "code");
        assert_eq!(detect_element_type("let variable = 5;", "rust"), "code");
        assert_eq!(detect_element_type("const x = 10;", "javascript"), "code");
        assert_eq!(detect_element_type("var y = 15;", "javascript"), "code");
    }

    #[test]
    fn test_detect_language_from_extension_comprehensive() {
        // Test supported extensions
        assert_eq!(detect_language_from_extension("rs"), "rust");
        assert_eq!(detect_language_from_extension("py"), "python");
        assert_eq!(detect_language_from_extension("js"), "javascript");
        assert_eq!(detect_language_from_extension("ts"), "typescript");
        assert_eq!(detect_language_from_extension("go"), "go");
        assert_eq!(detect_language_from_extension("rb"), "ruby");
        assert_eq!(detect_language_from_extension("java"), "java");
        assert_eq!(detect_language_from_extension("cpp"), "cpp");
        assert_eq!(detect_language_from_extension("cc"), "cpp");
        assert_eq!(detect_language_from_extension("cxx"), "cpp");
        assert_eq!(detect_language_from_extension("c"), "c");
        assert_eq!(detect_language_from_extension("h"), "c");
        assert_eq!(detect_language_from_extension("cs"), "csharp");
        assert_eq!(detect_language_from_extension("php"), "php");
        assert_eq!(detect_language_from_extension("swift"), "swift");
        assert_eq!(detect_language_from_extension("kt"), "kotlin");
        assert_eq!(detect_language_from_extension("scala"), "scala");
        assert_eq!(detect_language_from_extension("clj"), "clojure");
        assert_eq!(detect_language_from_extension("hs"), "haskell");
        assert_eq!(detect_language_from_extension("ml"), "ocaml");
        assert_eq!(detect_language_from_extension("fs"), "fsharp");
        assert_eq!(detect_language_from_extension("r"), "r");
        assert_eq!(detect_language_from_extension("m"), "matlab");
        assert_eq!(detect_language_from_extension("sh"), "shell");
        assert_eq!(detect_language_from_extension("bash"), "shell");
        assert_eq!(detect_language_from_extension("zsh"), "shell");
        assert_eq!(detect_language_from_extension("ps1"), "powershell");
        assert_eq!(detect_language_from_extension("sql"), "sql");
        assert_eq!(detect_language_from_extension("html"), "html");
        assert_eq!(detect_language_from_extension("css"), "css");
        assert_eq!(detect_language_from_extension("scss"), "scss");
        assert_eq!(detect_language_from_extension("xml"), "xml");
        assert_eq!(detect_language_from_extension("json"), "json");
        assert_eq!(detect_language_from_extension("yaml"), "yaml");
        assert_eq!(detect_language_from_extension("yml"), "yaml");
        assert_eq!(detect_language_from_extension("toml"), "toml");
        assert_eq!(detect_language_from_extension("ini"), "ini");
        assert_eq!(detect_language_from_extension("md"), "markdown");
        assert_eq!(detect_language_from_extension("tex"), "latex");
        assert_eq!(detect_language_from_extension("dockerfile"), "dockerfile");
        assert_eq!(detect_language_from_extension("makefile"), "makefile");
        assert_eq!(detect_language_from_extension("cmake"), "cmake");
        // Unsupported extensions return "unknown"
        assert_eq!(detect_language_from_extension("unknown_ext"), "unknown");
        assert_eq!(detect_language_from_extension(""), "unknown");
    }

    #[tokio::test]
    async fn test_processor_with_custom_syntax_parser() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let custom_parser = |_path: &std::path::Path| -> Result<Vec<ParsedChunk>> {
            Ok(vec![ParsedChunk {
                content: "custom parsed content".to_string(),
                start_line: 1,
                end_line: 1,
                language: "rust".to_string(),
                element_type: "custom".to_string(),
            }])
        };

        let processor = DefaultFileProcessor::with_defaults()
            .with_syntax_parser(custom_parser);
        
        let chunks = processor.process_file(&file_path).await.unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "custom parsed content");
        assert_eq!(chunks[0].metadata.element_type, "custom");
    }

    #[tokio::test]
    async fn test_processor_debug_format() {
        let processor = DefaultFileProcessor::with_defaults();
        let debug_str = format!("{:?}", processor);
        assert!(debug_str.contains("DefaultFileProcessor"));
        assert!(debug_str.contains("config"));
        assert!(debug_str.contains("syntax_parser_fn"));
    }

    #[tokio::test]
    async fn test_large_number_of_files() {
        let temp_dir = tempdir().unwrap();
        let mut files = Vec::new();
        
        // Create many small files
        for i in 0..50 {
            let file_path = temp_dir.path().join(format!("test_{}.rs", i));
            fs::write(&file_path, format!("fn test_{}() {{}}", i)).unwrap();
            files.push(file_path);
        }

        let processor = DefaultFileProcessor::with_defaults();
        let chunks = processor.process_files(&files).await.unwrap();
        
        assert!(chunks.len() >= 50);
    }

    #[tokio::test]
    async fn test_concurrent_processing_config() {
        let mut config = ProcessingConfig::default();
        config.file_processing_concurrency = 1; // Force single-threaded
        
        let temp_dir = tempdir().unwrap();
        let file1 = temp_dir.path().join("test1.rs");
        let file2 = temp_dir.path().join("test2.rs");
        
        fs::write(&file1, "fn test1() {}").unwrap();
        fs::write(&file2, "fn test2() {}").unwrap();

        let processor = DefaultFileProcessor::new(config);
        let files = vec![file1, file2];
        let chunks = processor.process_files(&files).await.unwrap();
        
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_parsed_chunk_creation() {
        let chunk = ParsedChunk {
            content: "test content".to_string(),
            start_line: 1,
            end_line: 5,
            language: "rust".to_string(),
            element_type: "function".to_string(),
        };
        
        assert_eq!(chunk.content, "test content");
        assert_eq!(chunk.start_line, 1);
        assert_eq!(chunk.end_line, 5);
        assert_eq!(chunk.language, "rust");
        assert_eq!(chunk.element_type, "function");
    }
} 