//! Manages the vocabulary mapping tokens to IDs for sparse vectors.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Error as IoError, ErrorKind as IoErrorKind};
use std::path::Path;
use serde::{Deserialize, Serialize};

/// Manages a vocabulary, mapping unique string tokens to u32 IDs.
/// Provides persistence via saving/loading to a file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VocabularyManager {
    token_to_id: HashMap<String, u32>,
    id_to_token: Vec<String>,
    // next_id is implicitly id_to_token.len()
}

impl VocabularyManager {
    /// Creates a new, empty VocabularyManager.
    pub fn new() -> Self {
        VocabularyManager {
            token_to_id: HashMap::new(),
            id_to_token: Vec::new(),
        }
    }

    /// Adds a token to the vocabulary if it doesn't exist.
    /// Returns the unique ID associated with the token.
    pub fn add_token(&mut self, token: &str) -> u32 {
        *self.token_to_id.entry(token.to_string()).or_insert_with(|| {
            let id = self.id_to_token.len() as u32;
            self.id_to_token.push(token.to_string());
            id
        })
    }

    /// Gets the ID associated with a given token, if it exists.
    pub fn get_id(&self, token: &str) -> Option<u32> {
        self.token_to_id.get(token).copied()
    }

    /// Gets the token associated with a given ID, if it exists.
    pub fn get_token(&self, id: u32) -> Option<&str> {
        self.id_to_token.get(id as usize).map(|s| s.as_str())
    }

    /// Returns the number of unique tokens in the vocabulary.
    pub fn len(&self) -> usize {
        self.id_to_token.len()
    }

    /// Checks if the vocabulary is empty.
     pub fn is_empty(&self) -> bool {
        self.id_to_token.is_empty()
    }

    /// Saves the vocabulary to a specified JSON file.
    pub fn save(&self, path: &Path) -> Result<(), IoError> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)
            .map_err(|e| IoError::new(IoErrorKind::Other, e))
    }

    /// Loads the vocabulary from a specified JSON file.
    /// Returns an error if the file doesn't exist or cannot be parsed.
    pub fn load(path: &Path) -> Result<Self, IoError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader)
            .map_err(|e| IoError::new(IoErrorKind::InvalidData, e))
    }
}

impl Default for VocabularyManager {
    fn default() -> Self {
        Self::new()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_add_and_get() {
        let mut vocab = VocabularyManager::new();
        assert_eq!(vocab.len(), 0);
        assert!(vocab.is_empty());

        let id1 = vocab.add_token("hello");
        let id2 = vocab.add_token("world");
        let id3 = vocab.add_token("hello"); // Add existing token

        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        assert_eq!(id3, 0); // Should return the existing ID

        assert_eq!(vocab.len(), 2);
        assert!(!vocab.is_empty());

        assert_eq!(vocab.get_id("hello"), Some(0));
        assert_eq!(vocab.get_id("world"), Some(1));
        assert_eq!(vocab.get_id("goodbye"), None);

        assert_eq!(vocab.get_token(0), Some("hello"));
        assert_eq!(vocab.get_token(1), Some("world"));
        assert_eq!(vocab.get_token(2), None);
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("vocab.json");

        let mut vocab_to_save = VocabularyManager::new();
        vocab_to_save.add_token("apple");
        vocab_to_save.add_token("banana");
        vocab_to_save.add_token("cherry");

        // Save
        vocab_to_save.save(&file_path).expect("Failed to save vocab");

        // Load
        let loaded_vocab = VocabularyManager::load(&file_path).expect("Failed to load vocab");

        // Verify contents
        assert_eq!(loaded_vocab.len(), 3);
        assert_eq!(loaded_vocab.get_id("apple"), Some(0));
        assert_eq!(loaded_vocab.get_id("banana"), Some(1));
        assert_eq!(loaded_vocab.get_id("cherry"), Some(2));
        assert_eq!(loaded_vocab.get_token(0), Some("apple"));
        assert_eq!(loaded_vocab.get_token(1), Some("banana"));
        assert_eq!(loaded_vocab.get_token(2), Some("cherry"));

        // Verify internal consistency
        assert_eq!(loaded_vocab.token_to_id.len(), 3);
        assert_eq!(loaded_vocab.id_to_token.len(), 3);
    }

     #[test]
    fn test_load_non_existent() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("non_existent_vocab.json");
        let result = VocabularyManager::load(&file_path);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().kind(), IoErrorKind::NotFound);
    }
} 