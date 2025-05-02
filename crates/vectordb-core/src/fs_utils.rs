use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use glob::{Pattern, MatchOptions};
use walkdir::WalkDir;
use log;
use std::fs;
use std::io::{BufRead, BufReader};

/// Checks if a directory entry corresponds to a typical VCS-ignored directory.
/// Currently only checks for `.git`.
fn is_vcs_ignored(entry: &walkdir::DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s == ".git") // Simple check for .git directory
         .unwrap_or(false)
}

/// Finds files within a base directory matching a glob pattern.
///
/// # Arguments
///
/// * `search_path` - The base directory to search within.
/// * `pattern_str` - The glob pattern to match against relative file paths.
/// * `case_sensitive` - Whether the pattern matching should be case-sensitive.
///
/// # Returns
///
/// A `Result` containing a `Vec<PathBuf>` of relative file paths matching the pattern,
/// or an error if the search path is invalid or the pattern is malformed.
pub fn find_files_matching_pattern(
    search_path: &Path,
    pattern_str: &str,
    case_sensitive: bool,
) -> Result<Vec<PathBuf>> {
    if !search_path.exists() {
        return Err(anyhow!("Search path does not exist: {}", search_path.display()));
    }
    if !search_path.is_dir() {
        return Err(anyhow!("Search path is not a directory: {}", search_path.display()));
    }

    log::debug!("Searching in: {} for pattern: '{}' (case_sensitive: {})", search_path.display(), pattern_str, case_sensitive);

    let glob_pattern = Pattern::new(pattern_str)
        .with_context(|| format!("Invalid glob pattern: {}", pattern_str))?;

    let match_options = MatchOptions {
        case_sensitive,
        require_literal_separator: true, // Match path separators (/ or \) literally
        require_literal_leading_dot: false, // Allow matching hidden files by default
    };

    let mut matches = Vec::new();
    let walker = WalkDir::new(search_path).into_iter();

    // Filter out VCS directories like .git before iterating
    for entry_result in walker.filter_entry(|e| !is_vcs_ignored(e)) {
        match entry_result {
            Ok(entry) => {
                if entry.file_type().is_file() {
                    let absolute_path = entry.path();
                    // Get path relative to search_path for matching
                    if let Ok(relative_path) = absolute_path.strip_prefix(search_path) {
                        if glob_pattern.matches_path_with(relative_path, match_options) {
                            // Store the relative path
                            matches.push(relative_path.to_path_buf());
                        }
                    } else {
                         log::warn!("Could not strip prefix '{}' from path '{}'", search_path.display(), absolute_path.display());
                    }
                }
            }
            Err(err) => {
                // Log error walking directory but continue if possible
                log::error!("Error walking directory entry near '{}': {}", err.path().unwrap_or(search_path).display(), err);
                // Optionally, decide whether to return Err or just skip the problematic entry
                // For now, we log and continue.
            }
        }
    }

    log::debug!("Found {} matches for pattern '{}' in {}", matches.len(), pattern_str, search_path.display());
    Ok(matches)
}

/// Reads the content of a file, optionally extracting a specific line range.
/// Lines are 1-based indexed.
///
/// # Arguments
///
/// * `file_path` - The absolute path to the file to read.
/// * `start_line` - Optional 1-based start line (inclusive).
/// * `end_line` - Optional 1-based end line (inclusive).
///
/// # Returns
///
/// A `Result` containing the requested file content as a `String`,
/// or an error if the file cannot be read or the line range is invalid.
pub fn read_file_range(
    file_path: &Path,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<String> {
    if !file_path.exists() {
        return Err(anyhow!("File not found: {}", file_path.display()));
    }
    if !file_path.is_file() {
        return Err(anyhow!("Path is not a file: {}", file_path.display()));
    }

    let file = fs::File::open(file_path)
        .with_context(|| format!("Failed to open file: {}", file_path.display()))?;
    let reader = BufReader::new(file);

    let start = start_line.unwrap_or(1);
    let end = end_line.unwrap_or(usize::MAX);

    if start == 0 {
        return Err(anyhow!("Start line must be 1-based"));
    }
    if end < start {
        return Err(anyhow!("End line ({}) cannot be less than start line ({})", end, start));
    }

    let mut content = String::new();
    for (index, line_result) in reader.lines().enumerate() {
        let current_line_num = index + 1;
        if current_line_num >= start && current_line_num <= end {
            let line = line_result.with_context(|| format!("Failed to read line {} from file: {}", current_line_num, file_path.display()))?;
            content.push_str(&line);
            content.push('\n');
        }
        if current_line_num > end {
            break; // Stop reading if we've passed the desired end line
        }
    }

    // Remove the trailing newline if content is not empty
    if !content.is_empty() {
        content.pop();
    }

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use std::io::Write; // Bring Write into scope for writeln!

    fn setup_test_dir() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let base_path = dir.path().to_path_buf();

        // Create dummy files and directories
        fs::create_dir_all(base_path.join("src")).unwrap();
        fs::create_dir_all(base_path.join("tests")).unwrap();
        fs::create_dir_all(base_path.join(".git")).unwrap(); // Ignored dir
        fs::write(base_path.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(base_path.join("src/lib.rs"), "pub fn func() {}").unwrap();
        fs::write(base_path.join("README.md"), "# Test Repo").unwrap();
        fs::write(base_path.join("tests/test1.rs"), "#[test]").unwrap();
        fs::write(base_path.join(".gitignore"), "target/").unwrap(); // A file to potentially match
        fs::write(base_path.join(".git/config"), "[core]").unwrap(); // File inside ignored dir

        (dir, base_path)
    }

    fn setup_view_test_file(dir: &Path) -> PathBuf {
        let file_path = dir.join("view_test.txt");
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3 - with content").unwrap();
        writeln!(file, "Line 4").unwrap();
        writeln!(file, "Line 5 - the end").unwrap();
        file_path
    }

    #[test]
    fn test_find_all_rs_files() {
        let (_dir, base_path) = setup_test_dir();
        let pattern = "**/*.rs";
        let result = find_files_matching_pattern(&base_path, pattern, true).unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&PathBuf::from("src/main.rs")));
        assert!(result.contains(&PathBuf::from("src/lib.rs")));
        assert!(result.contains(&PathBuf::from("tests/test1.rs")));
    }

    #[test]
    fn test_find_specific_file() {
        let (_dir, base_path) = setup_test_dir();
        let pattern = "README.md";
        let result = find_files_matching_pattern(&base_path, pattern, true).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from("README.md"));
    }
     #[test]
    fn test_find_files_in_src() {
        let (_dir, base_path) = setup_test_dir();
        let pattern = "src/*.rs";
        let result = find_files_matching_pattern(&base_path, pattern, true).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&PathBuf::from("src/main.rs")));
        assert!(result.contains(&PathBuf::from("src/lib.rs")));
    }

    #[test]
    fn test_no_matches() {
        let (_dir, base_path) = setup_test_dir();
        let pattern = "*.nonexistent";
        let result = find_files_matching_pattern(&base_path, pattern, true).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_case_insensitivity() {
        let (_dir, base_path) = setup_test_dir();
        let pattern = "readme.md";
        // Case sensitive should fail
        let result_sensitive = find_files_matching_pattern(&base_path, pattern, true).unwrap();
        assert!(result_sensitive.is_empty());
        // Case insensitive should succeed
        let result_insensitive = find_files_matching_pattern(&base_path, pattern, false).unwrap();
        assert_eq!(result_insensitive.len(), 1);
        assert_eq!(result_insensitive[0], PathBuf::from("README.md"));
    }

    #[test]
    fn test_ignores_git_dir() {
        let (_dir, base_path) = setup_test_dir();
        // This pattern would match .git/config if .git wasn't ignored
        let pattern = "**/*config*"; 
        let result = find_files_matching_pattern(&base_path, pattern, true).unwrap();
        // Expecting 0 matches because .git is ignored by filter_entry
        assert!(result.is_empty()); 
    }
     #[test]
    fn test_find_dotfile() {
        let (_dir, base_path) = setup_test_dir();
        let pattern = ".gitignore"; // Should match dotfiles in root
        let result = find_files_matching_pattern(&base_path, pattern, true).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], PathBuf::from(".gitignore"));
    }

    #[test]
    fn test_invalid_pattern() {
        let (_dir, base_path) = setup_test_dir();
        let pattern = "["; // Invalid glob pattern
        let result = find_files_matching_pattern(&base_path, pattern, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid glob pattern"));
    }

    #[test]
    fn test_invalid_search_path() {
        let dir = tempdir().unwrap();
        let invalid_path = dir.path().join("nonexistent_dir");
        let pattern = "*.rs";
        let result = find_files_matching_pattern(&invalid_path, pattern, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Search path does not exist"));
    }
     #[test]
    fn test_search_path_is_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("a_file.txt");
        fs::write(&file_path, "content").unwrap();
        let pattern = "*.txt";
        let result = find_files_matching_pattern(&file_path, pattern, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Search path is not a directory"));
    }

    #[test]
    fn test_read_full_file() {
        let dir = tempdir().unwrap();
        let file_path = setup_view_test_file(dir.path());
        let content = read_file_range(&file_path, None, None).unwrap();
        let expected = "Line 1\nLine 2\nLine 3 - with content\nLine 4\nLine 5 - the end";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_read_from_start_line() {
        let dir = tempdir().unwrap();
        let file_path = setup_view_test_file(dir.path());
        let content = read_file_range(&file_path, Some(3), None).unwrap();
        let expected = "Line 3 - with content\nLine 4\nLine 5 - the end";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_read_to_end_line() {
        let dir = tempdir().unwrap();
        let file_path = setup_view_test_file(dir.path());
        let content = read_file_range(&file_path, None, Some(2)).unwrap();
        let expected = "Line 1\nLine 2";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_read_specific_range() {
        let dir = tempdir().unwrap();
        let file_path = setup_view_test_file(dir.path());
        let content = read_file_range(&file_path, Some(2), Some(4)).unwrap();
        let expected = "Line 2\nLine 3 - with content\nLine 4";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_read_single_line() {
        let dir = tempdir().unwrap();
        let file_path = setup_view_test_file(dir.path());
        let content = read_file_range(&file_path, Some(3), Some(3)).unwrap();
        let expected = "Line 3 - with content";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_read_range_out_of_bounds_end() {
        let dir = tempdir().unwrap();
        let file_path = setup_view_test_file(dir.path());
        let content = read_file_range(&file_path, Some(4), Some(10)).unwrap(); // Only lines 4 and 5 exist
        let expected = "Line 4\nLine 5 - the end";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_read_range_out_of_bounds_start() {
        let dir = tempdir().unwrap();
        let file_path = setup_view_test_file(dir.path());
        let content = read_file_range(&file_path, Some(6), Some(10)).unwrap(); // Starts after last line
        assert!(content.is_empty());
    }

     #[test]
    fn test_read_file_not_found() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("nonexistent.txt");
        let result = read_file_range(&file_path, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File not found"));
    }

    #[test]
    fn test_read_path_is_directory() {
        let dir = tempdir().unwrap();
        let result = read_file_range(dir.path(), None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Path is not a file"));
    }

    #[test]
    fn test_read_invalid_range_start_zero() {
        let dir = tempdir().unwrap();
        let file_path = setup_view_test_file(dir.path());
        let result = read_file_range(&file_path, Some(0), Some(2));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Start line must be 1-based"));
    }

    #[test]
    fn test_read_invalid_range_end_before_start() {
        let dir = tempdir().unwrap();
        let file_path = setup_view_test_file(dir.path());
        let result = read_file_range(&file_path, Some(3), Some(2));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("End line (2) cannot be less than start line (3)"));
    }
} 