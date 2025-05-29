use git_manager::{GitManager, MerkleManager};
use std::fs;
use tempfile::TempDir;
use git2::{Repository, Signature};

/// Helper function to create a test repository with files
fn create_test_repo_with_files() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();
    
    // Initialize repository
    let repo = Repository::init(&repo_path).unwrap();
    let signature = Signature::now("Test User", "test@example.com").unwrap();
    
    // Create multiple files
    fs::write(repo_path.join("file1.txt"), "Content of file 1").unwrap();
    fs::write(repo_path.join("file2.txt"), "Content of file 2").unwrap();
    fs::create_dir(repo_path.join("subdir")).unwrap();
    fs::write(repo_path.join("subdir/file3.txt"), "Content of file 3").unwrap();
    
    // Create initial commit
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("file1.txt")).unwrap();
    index.add_path(std::path::Path::new("file2.txt")).unwrap();
    index.add_path(std::path::Path::new("subdir/file3.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit with files",
        &tree,
        &[],
    ).unwrap();
    
    (temp_dir, repo_path)
}

#[tokio::test]
async fn test_merkle_calculation_with_real_repo() {
    let (_temp_dir, repo_path) = create_test_repo_with_files();
    let mut manager = GitManager::new();
    
    // Initialize repository
    manager.initialize_repository(&repo_path).await.unwrap();
    
    // Calculate merkle state
    let (merkle_root, file_hashes) = manager
        .merkle_manager()
        .calculate_merkle_state(&repo_path, None)
        .unwrap();
    
    assert!(!merkle_root.is_empty());
    assert_eq!(file_hashes.len(), 3); // file1.txt, file2.txt, subdir/file3.txt
    
    // Verify specific files are included
    assert!(file_hashes.contains_key(&std::path::PathBuf::from("file1.txt")));
    assert!(file_hashes.contains_key(&std::path::PathBuf::from("file2.txt")));
    assert!(file_hashes.contains_key(&std::path::PathBuf::from("subdir/file3.txt")));
}

#[tokio::test]
async fn test_merkle_change_detection() {
    let (_temp_dir, repo_path) = create_test_repo_with_files();
    let mut manager = GitManager::new();
    
    // Calculate initial merkle state
    let (initial_root, initial_hashes) = manager
        .merkle_manager()
        .calculate_merkle_state(&repo_path, None)
        .unwrap();
    
    // Modify a file
    fs::write(repo_path.join("file1.txt"), "Modified content of file 1").unwrap();
    
    // Calculate new merkle state
    let (new_root, new_hashes) = manager
        .merkle_manager()
        .calculate_merkle_state(&repo_path, None)
        .unwrap();
    
    // Roots should be different
    assert_ne!(initial_root, new_root);
    
    // Compare states
    let diff = manager
        .merkle_manager()
        .compare_states(&initial_hashes, &new_hashes);
    
    assert!(diff.has_changes());
    assert_eq!(diff.modified.len(), 1);
    assert!(diff.modified.contains(&std::path::PathBuf::from("file1.txt")));
}

#[tokio::test]
async fn test_merkle_with_file_additions_and_deletions() {
    let (_temp_dir, repo_path) = create_test_repo_with_files();
    let mut manager = GitManager::new();
    
    // Calculate initial merkle state
    let (_, initial_hashes) = manager
        .merkle_manager()
        .calculate_merkle_state(&repo_path, None)
        .unwrap();
    
    // Add a new file
    fs::write(repo_path.join("new_file.txt"), "New file content").unwrap();
    
    // Delete an existing file
    fs::remove_file(repo_path.join("file2.txt")).unwrap();
    
    // Calculate new merkle state
    let (_, new_hashes) = manager
        .merkle_manager()
        .calculate_merkle_state(&repo_path, None)
        .unwrap();
    
    // Compare states
    let diff = manager
        .merkle_manager()
        .compare_states(&initial_hashes, &new_hashes);
    
    assert!(diff.has_changes());
    assert_eq!(diff.added.len(), 1);
    assert_eq!(diff.deleted.len(), 1);
    assert!(diff.added.contains(&std::path::PathBuf::from("new_file.txt")));
    assert!(diff.deleted.contains(&std::path::PathBuf::from("file2.txt")));
}

#[tokio::test]
async fn test_merkle_ignore_patterns() {
    let (_temp_dir, repo_path) = create_test_repo_with_files();
    let mut manager = GitManager::new();
    
    // Create files that should be ignored
    fs::write(repo_path.join("temp.tmp"), "Temporary file").unwrap();
    fs::write(repo_path.join("debug.log"), "Log file").unwrap();
    fs::create_dir(repo_path.join("target")).unwrap();
    fs::write(repo_path.join("target/build.txt"), "Build artifact").unwrap();
    
    // Calculate merkle state (should ignore temp files)
    let (_, file_hashes) = manager
        .merkle_manager()
        .calculate_merkle_state(&repo_path, None)
        .unwrap();
    
    // Should not include ignored files
    assert!(!file_hashes.contains_key(&std::path::PathBuf::from("temp.tmp")));
    assert!(!file_hashes.contains_key(&std::path::PathBuf::from("debug.log")));
    assert!(!file_hashes.contains_key(&std::path::PathBuf::from("target/build.txt")));
    
    // Should still include regular files
    assert!(file_hashes.contains_key(&std::path::PathBuf::from("file1.txt")));
}

#[test]
fn test_merkle_deterministic_ordering() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();
    
    // Create files in different order
    fs::write(repo_path.join("z_file.txt"), "Z content").unwrap();
    fs::write(repo_path.join("a_file.txt"), "A content").unwrap();
    fs::write(repo_path.join("m_file.txt"), "M content").unwrap();
    
    let manager = MerkleManager::new();
    
    // Calculate merkle multiple times
    let (root1, _) = manager.calculate_merkle_state(repo_path, None).unwrap();
    let (root2, _) = manager.calculate_merkle_state(repo_path, None).unwrap();
    let (root3, _) = manager.calculate_merkle_state(repo_path, None).unwrap();
    
    // Should be deterministic
    assert_eq!(root1, root2);
    assert_eq!(root2, root3);
} 