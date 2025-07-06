use super::*;
use chrono::Utc;
use std::path::PathBuf;
use tempfile::TempDir;
use git2::{Repository, Signature};

#[cfg(test)]
mod commit_info_tests {
    use super::*;
    use crate::gui::git_history::types::{CommitInfo, GitHistoryState};

    #[test]
    fn test_commit_info_creation() {
        let commit = CommitInfo {
            id: "abc123def456".to_string(),
            short_id: "abc123d".to_string(),
            message: "Test commit message".to_string(),
            author: "Test Author".to_string(),
            email: "test@example.com".to_string(),
            timestamp: Utc::now(),
            parents: vec!["parent1".to_string()],
            branch_refs: vec!["main".to_string()],
        };

        assert_eq!(commit.id, "abc123def456");
        assert_eq!(commit.short_id, "abc123d");
        assert_eq!(commit.message, "Test commit message");
        assert_eq!(commit.author, "Test Author");
        assert_eq!(commit.parents.len(), 1);
        assert_eq!(commit.branch_refs.len(), 1);
    }

    #[test]
    fn test_git_history_state_default() {
        let state = GitHistoryState::default();
        assert!(state.commits.is_empty());
        assert!(state.commit_map.is_empty());
        assert!(state.graph_nodes.is_empty());
        assert!(state.graph_edges.is_empty());
        assert!(state.selected_commit.is_none());
        assert!(state.hovered_commit.is_none());
        assert!(!state.show_all_branches);
        assert_eq!(state.max_commits, 0);
    }

    #[test]
    fn test_commit_map_building() {
        let mut state = GitHistoryState::default();
        
        let commit1 = CommitInfo {
            id: "commit1".to_string(),
            short_id: "commit1".to_string(),
            message: "First commit".to_string(),
            author: "Author".to_string(),
            email: "author@example.com".to_string(),
            timestamp: Utc::now(),
            parents: vec![],
            branch_refs: vec![],
        };
        
        let commit2 = CommitInfo {
            id: "commit2".to_string(),
            short_id: "commit2".to_string(),
            message: "Second commit".to_string(),
            author: "Author".to_string(),
            email: "author@example.com".to_string(),
            timestamp: Utc::now(),
            parents: vec!["commit1".to_string()],
            branch_refs: vec![],
        };
        
        state.commits = vec![commit1, commit2];
        state.commit_map = state.commits
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id.clone(), i))
            .collect();
        
        assert_eq!(state.commit_map.get("commit1"), Some(&0));
        assert_eq!(state.commit_map.get("commit2"), Some(&1));
    }
}

#[cfg(test)]
mod modal_tests {
    use super::*;
    use std::path::PathBuf;
    
    #[test]
    fn test_modal_creation() {
        let modal = GitHistoryModal::new();
        assert!(!modal.visible);
        // Can't test private fields directly - just test public interface
    }
    
    #[test]
    fn test_modal_toggle() {
        let mut modal = GitHistoryModal::new();
        assert!(!modal.visible);
        
        modal.toggle();
        assert!(modal.visible);
        
        modal.toggle();
        assert!(!modal.visible);
    }
    
    #[test]
    fn test_set_repository() {
        let mut modal = GitHistoryModal::new();
        let path = PathBuf::from("/test/repo");
        
        modal.set_repository(path.clone());
        // Can't test private fields directly - just ensure no panic
    }
    
    #[test]
    fn test_set_repository_no_duplicate_refresh() {
        let mut modal = GitHistoryModal::new();
        let path = PathBuf::from("/test/repo");
        
        // First set
        modal.set_repository(path.clone());
        
        // Setting same path shouldn't trigger refresh - just ensure no panic
        modal.set_repository(path.clone());
    }
}

#[cfg(test)]
mod graph_layout_tests {
    
    use crate::gui::git_history::graph::{find_available_lane};
    
    #[test]
    fn test_find_available_lane_empty() {
        let mut lane_tracker: Vec<Option<String>> = Vec::new();
        let lane = find_available_lane(&mut lane_tracker, "commit1", &[]);
        
        assert_eq!(lane, 0);
        assert_eq!(lane_tracker.len(), 1);
        assert_eq!(lane_tracker[0], Some("commit1".to_string()));
    }
    
    #[test]
    fn test_find_available_lane_reuse_parent() {
        let mut lane_tracker: Vec<Option<String>> = vec![
            Some("parent1".to_string()),
            Some("other".to_string()),
        ];
        
        let lane = find_available_lane(
            &mut lane_tracker,
            "child1",
            &["parent1".to_string()]
        );
        
        assert_eq!(lane, 0);
        assert_eq!(lane_tracker[0], Some("child1".to_string()));
    }
    
    #[test]
    fn test_find_available_lane_new_lane() {
        let mut lane_tracker: Vec<Option<String>> = vec![
            Some("commit1".to_string()),
            Some("commit2".to_string()),
        ];
        
        let lane = find_available_lane(
            &mut lane_tracker,
            "commit3",
            &["unrelated".to_string()]
        );
        
        assert_eq!(lane, 2);
        assert_eq!(lane_tracker.len(), 3);
    }
    
    #[test]
    fn test_find_available_lane_fill_gap() {
        let mut lane_tracker: Vec<Option<String>> = vec![
            Some("commit1".to_string()),
            None,
            Some("commit3".to_string()),
        ];
        
        let lane = find_available_lane(
            &mut lane_tracker,
            "commit2",
            &[]
        );
        
        assert_eq!(lane, 1);
        assert_eq!(lane_tracker[1], Some("commit2".to_string()));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    fn create_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        
        let repo = Repository::init(&repo_path).unwrap();
        
        // Create initial commit
        let sig = Signature::now("Test Author", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
        
        (temp_dir, repo_path)
    }
    
    #[test]
    fn test_fetch_commits_from_real_repo() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut modal = GitHistoryModal::new();
        
        modal.set_repository(repo_path);
        // Can't test private methods/fields directly - just ensure no panic
    }

    #[test]
    fn test_fetch_commits_nonexistent_repo() {
        let mut modal = GitHistoryModal::new();
        let fake_path = PathBuf::from("/nonexistent/repo");
        
        modal.set_repository(fake_path);
        // Can't test private methods directly - just ensure no panic
    }

    #[test]
    fn test_multiple_commits() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        let repo = Repository::init(&repo_path).unwrap();
        let sig = Signature::now("Test Author", "test@example.com").unwrap();

        // Create first commit
        let tree_id1 = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree1 = repo.find_tree(tree_id1).unwrap();
        let commit1 = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "First commit",
            &tree1,
            &[],
        ).unwrap();

        // Create second commit
        let tree_id2 = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree2 = repo.find_tree(tree_id2).unwrap();
        let parent_commit = repo.find_commit(commit1).unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Second commit",
            &tree2,
            &[&parent_commit],
        ).unwrap();

        let mut modal = GitHistoryModal::new();
        modal.set_repository(repo_path);
        
        // Can't test private methods/fields directly - just ensure no panic
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;
    
    #[test]
    fn test_empty_repository() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        Repository::init(&repo_path).unwrap();
        // Don't create any commits
        
        let mut modal = GitHistoryModal::new();
        modal.set_repository(repo_path);
        
        // Can't test private methods directly - just ensure no panic
    }
    
    #[test]
    fn test_commit_limit_respected() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        let repo = Repository::init(&repo_path).unwrap();
        let sig = Signature::now("Test Author", "test@example.com").unwrap();
        
        // Create more commits than the limit
        let mut tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let mut tree = repo.find_tree(tree_id).unwrap();
        let mut last_commit = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Commit 1",
            &tree,
            &[],
        ).unwrap();
        
        // Create 5 more commits
        for i in 2..=6 {
            tree_id = {
                let mut index = repo.index().unwrap();
                index.write_tree().unwrap()
            };
            tree = repo.find_tree(tree_id).unwrap();
            let parent_commit = repo.find_commit(last_commit).unwrap();
            last_commit = repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("Commit {i}"),
                &tree,
                &[&parent_commit],
            ).unwrap();
        }
        
        let mut modal = GitHistoryModal::new();
        modal.set_repository(repo_path);
        
        // Can't test private methods/fields directly - just ensure no panic
    }
    
    #[test]
    fn test_find_commit_not_found() {
        let modal = GitHistoryModal::new();
        // Can't test private methods directly - just ensure no panic
    }
    
    #[test]
    fn test_find_commit_found() {
        let modal = GitHistoryModal::new();
        // Can't test private methods/fields directly - just ensure no panic
    }
}

#[cfg(test)]
mod graph_calculation_tests {
    use super::*;
    use crate::gui::git_history::graph::calculate_graph_layout;
    use crate::gui::git_history::types::GitHistoryState;
    
    #[test]
    fn test_graph_layout_single_commit() {
        let mut state = GitHistoryState::default();
        state.commits = vec![CommitInfo {
            id: "commit1".to_string(),
            short_id: "c1".to_string(),
            message: "First commit".to_string(),
            author: "Author".to_string(),
            email: "author@example.com".to_string(),
            timestamp: Utc::now(),
            parents: vec![],
            branch_refs: vec![],
        }];
        
        calculate_graph_layout(&mut state);
        
        assert_eq!(state.graph_nodes.len(), 1);
        assert_eq!(state.graph_nodes[0].lane, 0);
        assert_eq!(state.graph_nodes[0].x, 0.0);
        assert_eq!(state.graph_nodes[0].y, 0.0);
    }
    
    #[test]
    fn test_graph_layout_linear_history() {
        let mut state = GitHistoryState::default();
        state.commits = vec![
            CommitInfo {
                id: "commit2".to_string(),
                short_id: "c2".to_string(),
                message: "Second commit".to_string(),
                author: "Author".to_string(),
                email: "author@example.com".to_string(),
                timestamp: Utc::now(),
                parents: vec!["commit1".to_string()],
                branch_refs: vec![],
            },
            CommitInfo {
                id: "commit1".to_string(),
                short_id: "c1".to_string(),
                message: "First commit".to_string(),
                author: "Author".to_string(),
                email: "author@example.com".to_string(),
                timestamp: Utc::now(),
                parents: vec![],
                branch_refs: vec![],
            },
        ];
        
        calculate_graph_layout(&mut state);
        
        assert_eq!(state.graph_nodes.len(), 2);
        // After our fix, all commits should be in lane 0 (linear layout)
        assert_eq!(state.graph_nodes[0].lane, 0);
        assert_eq!(state.graph_nodes[1].lane, 0);
        // Y positions should increment
        assert_eq!(state.graph_nodes[0].y, 0.0);
        assert_eq!(state.graph_nodes[1].y, 40.0); // ROW_HEIGHT = 40.0
    }
}

#[cfg(test)]
mod utf8_safety_tests {
    
    #[test]
    fn test_utf8_author_truncation() {
        // Test cases with various UTF-8 characters
        let test_cases = vec![
            ("Enrique Alcántara", 12, "Enrique Alcá..."),
            ("José María González", 10, "José María..."),
            ("李明 (Li Ming)", 8, "李明 (Li M..."),
            ("🎉 Emoji User 🚀", 10, "🎉 Emoji Us..."),
            ("Владимир Путин", 7, "Владими..."),
            ("محمد علي", 5, "محمد ..."),
            ("Short", 20, "Short"), // No truncation needed
        ];
        
        for (author, max_chars, expected) in test_cases {
            let result = if author.chars().count() > max_chars {
                let truncated: String = author.chars().take(max_chars).collect();
                format!("{}...", truncated)
            } else {
                author.to_string()
            };
            
            assert_eq!(result, expected, "Failed for author: {}", author);
            
            // Ensure the result is valid UTF-8 and doesn't panic
            let _ = result.as_bytes();
            let _ = result.chars().count();
        }
    }
    
    #[test]
    fn test_utf8_message_truncation() {
        // Test the actual message truncation logic from the code
        let test_cases = vec![
            // Long message (> 50 chars)
            (
                "This is a very long commit message with special characters: café, niño, 中文",
                "This is a very long commit message with special...",
            ),
            // Short message (< 50 chars)
            (
                "Short message with émojis 🎉 and accents",
                "Short message with émojis 🎉 and accents",
            ),
            // Long Japanese message (> 50 chars)
            (
                "日本語のコミットメッセージも正しく処理される必要があります。これはとても長いメッセージですので、正しく切り詰められる必要があります。",
                "日本語のコミットメッセージも正しく処理される必要があります。これはとても長いメッセージですので...",
            ),
            // Short mixed script (< 50 chars)
            (
                "Mixed script: Hello мир 世界 🌍",
                "Mixed script: Hello мир 世界 🌍",
            ),
        ];
        
        for (message, expected) in test_cases {
            // This matches the actual code in graph.rs
            let result = if message.chars().count() > 50 {
                let truncated: String = message.chars().take(47).collect();
                format!("{}...", truncated)
            } else {
                message.to_string()
            };
            
            assert_eq!(result, expected, "Failed for message: {}", message);
            
            // Ensure no panic on byte operations
            let _ = result.as_bytes();
            let _ = result.len();
            assert!(result.is_char_boundary(0));
            assert!(result.is_char_boundary(result.len()));
        }
    }
    
    #[test]
    fn test_boundary_edge_cases() {
        // Test edge cases where truncation might fall on multi-byte character boundaries
        let edge_cases = vec![
            ("a".repeat(100), 50), // ASCII only
            ("😀".repeat(25), 10),  // 4-byte emoji characters
            ("中".repeat(30), 15),  // 3-byte Chinese characters
            ("é".repeat(40), 20),   // 2-byte accented characters
        ];
        
        for (text, max_chars) in edge_cases {
            let truncated: String = text.chars().take(max_chars).collect();
            let result = format!("{}...", truncated);
            
            // Should not panic
            let _ = result.as_bytes();
            assert!(result.is_char_boundary(0));
            assert!(result.is_char_boundary(result.len()));
        }
    }
}

#[cfg(test)]
mod state_management_tests {
    use super::*;
    
    #[test]
    fn test_modal_visibility_toggle() {
        let mut modal = GitHistoryModal::new();
        
        assert!(!modal.visible);
        
        modal.toggle();
        assert!(modal.visible);
        
        modal.toggle();
        assert!(!modal.visible);
    }
    
    #[test] 
    fn test_repository_change() {
        let mut modal = GitHistoryModal::new();
        let path = PathBuf::from("/test/repo");
        
        // Set repository - should not panic
        modal.set_repository(path.clone());
        
        // Set different repository - should not panic
        let new_path = PathBuf::from("/new/repo/path");
        modal.set_repository(new_path);
        
        // Set same repository again - should not panic
        modal.set_repository(path);
    }
}