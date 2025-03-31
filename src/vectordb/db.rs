use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};
use walkdir::WalkDir;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::cache::EmbeddingCache;
use crate::vectordb::error::{Result, VectorDBError};

#[derive(Serialize, Deserialize)]
struct DBFile {
    embeddings: HashMap<String, Vec<f32>>,
}

pub struct VectorDB {
    pub embeddings: HashMap<String, Vec<f32>>,
    db_path: String,
    cache: EmbeddingCache,
}

impl VectorDB {
    pub fn new(db_path: String) -> Result<Self> {
        let embeddings = if Path::new(&db_path).exists() {
            let contents = fs::read_to_string(&db_path)
                .map_err(|e| VectorDBError::FileReadError {
                    path: Path::new(&db_path).to_path_buf(),
                    source: e,
                })?;
            serde_json::from_str(&contents)
                .map_err(VectorDBError::SerializationError)?
        } else {
            HashMap::new()
        };

        // Create cache in the same directory as the database
        let cache_path = Path::new(&db_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("cache.json")
            .to_string_lossy()
            .to_string();
        
        let cache = EmbeddingCache::new(cache_path)?;

        Ok(Self {
            embeddings,
            db_path,
            cache,
        })
    }

    pub fn index_file(&mut self, file_path: &Path) -> Result<()> {
        let file_path_str = file_path.to_string_lossy().to_string();
        
        // Check cache first
        if let Some(cached_embedding) = self.cache.get(&file_path_str) {
            self.embeddings.insert(file_path_str.clone(), cached_embedding.to_vec());
            self.save()?;
            return Ok(());
        }

        // If not in cache, generate new embedding
        let model = EmbeddingModel::new()
            .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))?;
        let contents = fs::read_to_string(file_path)
            .map_err(|e| VectorDBError::FileReadError {
                path: file_path.to_path_buf(),
                source: e,
            })?;
        let embedding = model.embed(&contents)
            .map_err(|e| VectorDBError::EmbeddingError(e.to_string()))?;
        
        // Get file hash for cache
        let file_hash = EmbeddingCache::get_file_hash(file_path)?;
        
        // Store in both cache and database
        self.cache.insert(file_path_str.clone(), embedding.clone(), file_hash)?;
        self.embeddings.insert(file_path_str, embedding);
        self.save()?;
        
        Ok(())
    }

    pub fn index_directory(&mut self, dir: &str, file_types: &[String]) -> Result<()> {
        let dir_path = Path::new(dir);
        
        for entry in WalkDir::new(dir_path) {
            let entry = entry.map_err(|e| VectorDBError::DatabaseError(e.to_string()))?;
            if entry.file_type().is_file() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    let ext = ext.to_string_lossy().to_string();
                    if file_types.contains(&ext) {
                        self.index_file(path)?;
                    }
                }
            }
        }
        
        Ok(())
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = Path::new(&self.db_path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| VectorDBError::DirectoryCreationError {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
        }
        let db_file = DBFile {
            embeddings: self.embeddings.clone(),
        };
        let contents = serde_json::to_string_pretty(&db_file)
            .map_err(VectorDBError::SerializationError)?;
        fs::write(&self.db_path, contents)
            .map_err(|e| VectorDBError::FileWriteError {
                path: Path::new(&self.db_path).to_path_buf(),
                source: e,
            })?;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.embeddings.clear();
        self.cache.clear()?;
        if Path::new(&self.db_path).exists() {
            fs::remove_file(&self.db_path)
                .map_err(|e| VectorDBError::FileWriteError {
                    path: Path::new(&self.db_path).to_path_buf(),
                    source: e,
                })?;
        }
        Ok(())
    }

    pub fn stats(&self) -> DBStats {
        DBStats {
            indexed_files: self.embeddings.len(),
            embedding_dimension: self.embeddings.values().next().map(|v| v.len()).unwrap_or(0),
            db_path: self.db_path.clone(),
            cached_files: self.cache.len(),
        }
    }
}

pub struct DBStats {
    pub indexed_files: usize,
    pub embedding_dimension: usize,
    pub db_path: String,
    pub cached_files: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_vectordb() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("db.json");
        let mut db = VectorDB::new(db_path.to_string_lossy().to_string())?;

        // Create a test file
        let test_file = dir.path().join("test.txt");
        let mut file = File::create(&test_file)?;
        file.write_all(b"test content")?;

        // Test indexing
        db.index_file(&test_file)?;

        // Test stats
        let stats = db.stats();
        assert_eq!(stats.indexed_files, 1);
        assert!(stats.embedding_dimension > 0);
        assert!(stats.cached_files > 0);

        // Test clear
        db.clear()?;
        assert_eq!(db.embeddings.len(), 0);

        Ok(())
    }
} 