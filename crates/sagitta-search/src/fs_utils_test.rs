#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_files_matching_pattern_star_rs() {
        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create some test files
        fs::write(temp_path.join("main.rs"), "fn main() {}").unwrap();
        fs::write(temp_path.join("lib.rs"), "pub fn lib() {}").unwrap();
        fs::write(temp_path.join("test.txt"), "not a rust file").unwrap();
        
        // Create a subdirectory with more .rs files
        let sub_dir = temp_path.join("src");
        fs::create_dir(&sub_dir).unwrap();
        fs::write(sub_dir.join("mod.rs"), "mod tests;").unwrap();

        // Test with *.rs pattern
        let matches = find_files_matching_pattern(temp_path, "*.rs", false).unwrap();
        
        // Should find main.rs and lib.rs in root
        assert_eq!(matches.len(), 2, "Should find 2 .rs files in root");
        
        let mut match_names: Vec<String> = matches.iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        match_names.sort();
        
        assert_eq!(match_names, vec!["lib.rs", "main.rs"]);
        
        // Test with **/*.rs pattern to include subdirectories
        let all_matches = find_files_matching_pattern(temp_path, "**/*.rs", false).unwrap();
        assert_eq!(all_matches.len(), 3, "Should find 3 .rs files total with **/*.rs");
    }

    #[test]
    fn test_find_files_case_sensitivity() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create files with different cases
        fs::write(temp_path.join("Main.rs"), "fn main() {}").unwrap();
        fs::write(temp_path.join("main.RS"), "fn main() {}").unwrap();
        
        // Case sensitive search
        let case_sensitive = find_files_matching_pattern(temp_path, "*.rs", true).unwrap();
        assert_eq!(case_sensitive.len(), 0, "Case sensitive should find 0 files with *.rs");
        
        // Case insensitive search
        let case_insensitive = find_files_matching_pattern(temp_path, "*.rs", false).unwrap();
        assert_eq!(case_insensitive.len(), 2, "Case insensitive should find 2 files");
    }
}