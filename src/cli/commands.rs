use clap::Parser;
use anyhow::Result;
use crate::vectordb::VectorDB;
use crate::vectordb::embedding::EmbeddingModel;
use crate::vectordb::search::Search;

#[derive(Parser, Debug)]
pub enum Command {
    /// Index files in a directory
    Index {
        /// Directory to index
        #[arg(required = true)]
        dir: String,

        /// File types to index (e.g. rs,py,txt)
        #[arg(short = 't', long = "file-types", value_delimiter = ',')]
        file_types: Vec<String>,
    },

    /// Search for files by content
    Query {
        /// Search query
        #[arg(required = true)]
        query: String,
    },

    /// Show database statistics
    Stats,

    /// Clear the database
    Clear,
}

pub fn execute_command(command: Command, mut db: VectorDB) -> Result<()> {
    match command {
        Command::Index { dir, file_types } => {
            println!("Indexing files in {}...", dir);
            db.index_directory(&dir, &file_types)?;
            println!("Indexing complete!");
        }
        Command::Query { query } => {
            let model = EmbeddingModel::new()?;
            let search = Search::new(db, model);
            let results = search.search(&query)?;

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            println!("\nSearch results for: {}\n", query);
            for (i, result) in results.iter().enumerate() {
                println!("{}. {} (similarity: {:.2})", i + 1, result.file_path, result.similarity);
                println!("   {}", result.snippet);
                println!();
            }
        }
        Command::Stats => {
            let stats = db.stats();
            println!("Indexed files: {}", stats.indexed_files);
            println!("Embedding dimension: {}", stats.embedding_dimension);
            println!("Database path: {}", stats.db_path);
        }
        Command::Clear => {
            db.clear()?;
            println!("Database cleared!");
        }
    }
    Ok(())
} 