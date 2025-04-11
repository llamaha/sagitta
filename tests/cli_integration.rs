use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use std::error::Error;
use std::path::PathBuf;

// Helper function to get the path to the compiled binary
fn get_binary_path() -> Result<String, Box<dyn Error>> {
    Ok(assert_cmd::cargo::cargo_bin("vectordb-cli").to_string_lossy().to_string())
}

// Checks for default ONNX files and returns their paths if found.
// Returns Ok(None) if files are not found.
fn check_default_onnx_files() -> Result<Option<(PathBuf, PathBuf)>, Box<dyn Error>> {
    let default_model_path = Path::new("onnx/all-minilm-l12-v2.onnx").canonicalize();
    let default_tokenizer_path = Path::new("onnx/minilm_tokenizer.json").canonicalize();

    match (default_model_path, default_tokenizer_path) {
        (Ok(model_path), Ok(tokenizer_path)) => {
            if model_path.exists() && tokenizer_path.exists() {
                Ok(Some((model_path, tokenizer_path)))
            } else {
                Ok(None) // One or both files don't exist
            }
        }
        _ => Ok(None), // Error during canonicalization likely means they don't exist at the relative path
    }
}

// Helper to set up ONNX files (dummy or real)
fn setup_onnx_files(temp_dir: &Path) -> Result<(String, String), Box<dyn Error>> {
    let onnx_dir = temp_dir.join("onnx_test_files");
    fs::create_dir_all(&onnx_dir)?;

    let model_path = onnx_dir.join("test_model.onnx");
    let tokenizer_dir = onnx_dir.join("test_tokenizer_dir");
    fs::create_dir_all(&tokenizer_dir)?;
    let tokenizer_file_path = tokenizer_dir.join("tokenizer.json");

    // Try to use default files if they exist, otherwise create dummies
    let default_model_path = Path::new("onnx/all-minilm-l12-v2.onnx");
    let default_tokenizer_path = Path::new("onnx/minilm_tokenizer.json");

    if default_model_path.exists() && default_tokenizer_path.exists() {
        fs::copy(default_model_path, &model_path)?;
        fs::copy(default_tokenizer_path, &tokenizer_file_path)?;
        println!("Using default ONNX files for integration test.");
    } else {
        println!("Default ONNX files not found, creating dummies for integration test.");
        fs::write(&model_path, "dummy_model_data")?;
        // Use the minimal valid JSON structure found during unit testing
        let tokenizer_json = "{\n\"model\": {\n\"vocab\": {\n\"hello\": 0,\n\"world\": 1\n}\n}\n}";
        fs::write(&tokenizer_file_path, tokenizer_json)?;
    }

    Ok((
        model_path.to_string_lossy().to_string(),
        tokenizer_dir.to_string_lossy().to_string(), // Return the directory path
    ))
}


#[test]
fn test_cli_index_list_remove() -> Result<(), Box<dyn Error>> {
    // Check if default ONNX files exist. Skip test if not found.
    let onnx_paths = match check_default_onnx_files()? {
        Some((model, tokenizer)) => (model, tokenizer),
        None => {
            eprintln!("Skipping integration test: Default ONNX model/tokenizer not found in ./onnx/");
            return Ok(());
        }
    };
    let model_path = onnx_paths.0.to_string_lossy();
    // Use the *directory* containing the tokenizer file
    let tokenizer_path = onnx_paths.1.parent().ok_or("Tokenizer path has no parent directory")?.to_string_lossy();

    let bin_path = get_binary_path()?;
    let temp_dir = tempdir()?;
    let db_path = temp_dir.path().join("test_db.json");
    let (model_path, tokenizer_path) = setup_onnx_files(temp_dir.path())?;

    let repo_a = temp_dir.path().join("repo_a");
    let repo_b = temp_dir.path().join("repo_b");
    fs::create_dir_all(&repo_a)?;
    fs::create_dir_all(&repo_b)?;
    fs::write(repo_a.join("file_a.txt"), "Content A")?;
    fs::write(repo_b.join("file_b.txt"), "Content B")?;

    let repo_a_str = repo_a.to_string_lossy();
    let repo_b_str = repo_b.to_string_lossy();
    let db_path_str = db_path.to_string_lossy();

    // 1. Index repo_a
    Command::new(&bin_path)
        .arg("--db-path")
        .arg(&*db_path_str)
        .arg("index")
        .arg(&*repo_a_str)
        .arg("--onnx-model")
        .arg(&model_path)
        .arg("--onnx-tokenizer")
        .arg(&tokenizer_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Finished indexing for: {}", repo_a_str)));

    // 2. Index repo_b
    Command::new(&bin_path)
        .arg("--db-path")
        .arg(&*db_path_str)
        .arg("index")
        .arg(&*repo_b_str)
        .arg("--onnx-model")
        .arg(&model_path)
        .arg("--onnx-tokenizer")
        .arg(&tokenizer_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("Finished indexing for: {}", repo_b_str)));

    // 3. List - check both exist
    Command::new(&bin_path)
        .arg("--db-path")
        .arg(&*db_path_str)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains(&*repo_a_str))
        .stdout(predicate::str::contains(&*repo_b_str));

    // 4. Remove repo_a
    Command::new(&bin_path)
        .arg("--db-path")
        .arg(&*db_path_str)
        .arg("remove")
        .arg(&*repo_a_str)
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Successfully removed directory '{}'",
            repo_a_str
        )));

    // 5. List - check only repo_b exists
    Command::new(&bin_path)
        .arg("--db-path")
        .arg(&*db_path_str)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains(&*repo_b_str))
        .stdout(predicate::str::contains(&*repo_a_str).not());

    // 6. Remove non-existent repo (should fail)
    let non_existent_repo = temp_dir.path().join("non_existent");
    let non_existent_repo_str = non_existent_repo.to_string_lossy();
     // Create the directory so canonicalize works, but it's not indexed
    fs::create_dir(&non_existent_repo)?;

    Command::new(&bin_path)
        .arg("--db-path")
        .arg(&*db_path_str)
        .arg("remove")
        .arg(&*non_existent_repo_str)
        .assert()
        .failure() // Expecting failure
        .stderr(predicate::str::contains(format!( // Error should be printed to stderr
            "Directory '{}' is not present in the index",
             non_existent_repo.canonicalize()?.to_string_lossy()
        )));

    Ok(())
} 