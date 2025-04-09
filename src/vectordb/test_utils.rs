#[cfg(test)]
pub mod tests {
    use std::fs;
    use std::io::{self, Write};
    use std::path::PathBuf;

    pub fn create_test_files(base_path: &str, num_files: usize, content: &str) -> io::Result<()> {
        fs::create_dir_all(base_path)?;
        for i in 0..num_files {
            let file_path = PathBuf::from(base_path).join(format!("file{}.txt", i));
            let mut file = fs::File::create(&file_path)?;
            writeln!(file, "{}", content)?; // Use the provided content
        }
        Ok(())
    }
}
