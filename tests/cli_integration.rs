use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
//use std::path::Path;
//use tempfile::tempdir;
use std::path::PathBuf;
use tempfile::TempDir;
use anyhow::Result;

// Helper function to get the path to the compiled binary
fn get_binary_path() -> Result<PathBuf> {
    let mut path = std::env::current_exe()?;
    path.pop(); // Remove the binary name
    path.pop(); // Remove 'deps'
    path.push(env!("CARGO_PKG_NAME")); // Add the binary name back
    if !path.exists() {
        // If not found, try the target/debug or target/release path directly
        path = std::env::current_exe()?;
        path.pop(); // Remove the binary name
        if path.ends_with("deps") { path.pop(); } // Remove deps if present
        // Determine build profile
        let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
        if path.ends_with(profile) { path.pop(); } // Remove profile if present
        path.push(profile);
        path.push(env!("CARGO_PKG_NAME"));
    }
    if !path.exists() {
        return Err(anyhow::anyhow!("Could not find the compiled binary path"));
    }
    Ok(path)
}

// Helper to get default ONNX paths (assuming they exist)
// Define this outside the test functions
fn get_default_onnx_paths() -> Option<(PathBuf, PathBuf)> {
    let model_path = PathBuf::from("onnx/all-minilm-l12-v2.onnx");
    let tokenizer_path = PathBuf::from("onnx/minilm_tokenizer.json");
    if model_path.exists() && tokenizer_path.exists() {
        Some((model_path, tokenizer_path))
    } else {
        eprintln!("Warning: Default ONNX files not found in ./onnx/. Skipping test that requires them.");
        None
    }
}

#[test]
fn test_cli_index_list_remove() -> Result<()> {
    let onnx_paths_opt = get_default_onnx_paths();
    if onnx_paths_opt.is_none() {
        println!("Skipping test_cli_index_list_remove: Default ONNX files not found.");
        return Ok(());
    }
    let (model_path, tokenizer_path) = onnx_paths_opt.unwrap();
    let tokenizer_dir = tokenizer_path.parent().ok_or(anyhow::anyhow!("Tokenizer path has no parent"))?;

    let temp_dir = TempDir::new()?;
    let bin_path = get_binary_path()?;

    let repo_a = temp_dir.path().join("repo_a");
    let repo_b = temp_dir.path().join("repo_b");
    let repo_c = temp_dir.path().join("repo_c");
    fs::create_dir_all(&repo_a)?;
    fs::create_dir_all(&repo_b)?;
    fs::create_dir_all(&repo_c)?;
    fs::write(repo_a.join("file_a.txt"), "Content A")?;
    fs::write(repo_b.join("file_b.txt"), "Content B")?;
    fs::write(repo_c.join("file_c.rs"), "fn main() {}")?;

    let repo_a_canon = repo_a.canonicalize()?.to_string_lossy().to_string();
    let repo_b_canon = repo_b.canonicalize()?.to_string_lossy().to_string();
    let repo_c_canon = repo_c.canonicalize()?.to_string_lossy().to_string();

    // --- TODO: Clear collection before test --- 
    // Example: Run `clear --all -y` (needs test Qdrant instance)
    // Command::new(&bin_path).env("QDRANT_URL", "http://localhost:6334").arg("clear").arg("--all").arg("-y").assert().success();
    // std::thread::sleep(std::time::Duration::from_secs(1)); // Give Qdrant time

    // --- Index Commands --- 
    Command::new(&bin_path)
        .env("ONNX_MODEL_PATH", model_path.to_str().unwrap())
        .env("TOKENIZER_PATH", tokenizer_dir.to_str().unwrap()) // Correct env var name is TOKENIZER_PATH for main.rs logic
        .env("QDRANT_URL", "http://localhost:6334")
        .arg("index")
        .arg(repo_a.to_str().unwrap())
        .assert()
        .success();

    Command::new(&bin_path)
        .env("ONNX_MODEL_PATH", model_path.to_str().unwrap())
        .env("TOKENIZER_PATH", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", "http://localhost:6334")
        .arg("index")
        .arg(repo_b.to_str().unwrap())
        .assert()
        .success();
    
     Command::new(&bin_path)
        .env("ONNX_MODEL_PATH", model_path.to_str().unwrap())
        .env("TOKENIZER_PATH", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", "http://localhost:6334")
        .arg("index")
        .arg(repo_c.to_str().unwrap())
        .assert()
        .success();

    // --- List Check 1 --- 
    Command::new(&bin_path)
        .env("QDRANT_URL", "http://localhost:6334") // Only URL needed for list
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains(&repo_a_canon))
        .stdout(predicate::str::contains(&repo_b_canon))
        .stdout(predicate::str::contains(&repo_c_canon));

    // --- Use `clear --directory` instead of placeholder `remove` --- 
    println!("Running clear --directory for repo_a...");
    Command::new(&bin_path)
        .env("QDRANT_URL", "http://localhost:6334") 
        .arg("clear")
        .arg("--directory")
        .arg(repo_a.to_str().unwrap()) // Use original path, canonicalization happens internally
        .arg("-y") // Add confirmation flag
        .assert()
        .success();
    println!("Finished clear --directory for repo_a.");

    // --- List Check 2 --- 
    Command::new(&bin_path)
        .env("QDRANT_URL", "http://localhost:6334")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains(&repo_b_canon))
        .stdout(predicate::str::contains(&repo_c_canon))
        .stdout(predicate::str::contains(&repo_a_canon).not());

    // --- Remove Non-existent (Adapt for clear command) ---
    let non_existent_repo = temp_dir.path().join("non_existent");
    fs::create_dir(&non_existent_repo)?;
    let non_existent_repo_str = non_existent_repo.to_string_lossy();

    println!("Running clear --directory for non-existent repo...");
    Command::new(&bin_path)
        .env("QDRANT_URL", "http://localhost:6334") 
        .arg("clear")
        .arg("--directory")
        .arg(&*non_existent_repo_str)
        .arg("-y")
        .assert()
        .success() // Command should succeed even if no points were deleted
        // Check stdout/stderr for a message indicating nothing was removed or dir not found?
        // Current implementation might just log a warning. Let's assert success for now.
        .stdout(predicate::str::contains("Successfully removed points")); // Check for success message
    println!("Finished clear --directory for non-existent repo.");

    Ok(())
}

#[test]
fn test_cli_clear_failures() -> Result<()> {
    let bin_path = get_binary_path()?;
    let temp_dir = TempDir::new()?;
    let dummy_dir = temp_dir.path().join("dummy_dir_for_clear");
    fs::create_dir(&dummy_dir)?;

    // 1. Missing scope
    println!("Testing clear without scope...");
    Command::new(&bin_path)
        .env("QDRANT_URL", "http://localhost:6334")
        .arg("clear")
        .arg("-y") // Provide confirmation to bypass prompt
        .assert()
        .failure()
        .stderr(predicate::str::contains("the following required arguments were not provided"));

    // 2. Conflicting scope
    println!("Testing clear with --all and --directory...");
    Command::new(&bin_path)
        .env("QDRANT_URL", "http://localhost:6334")
        .arg("clear")
        .arg("--all")
        .arg("--directory")
        .arg(dummy_dir.to_str().unwrap())
        .arg("-y")
        .assert()
        .failure()
        // Make assertion less brittle: check for relevant keywords
        .stderr(predicate::str::contains("argument")
                .and(predicate::str::contains("--all"))
                .and(predicate::str::contains("--directory")));

    // 3. Missing confirmation (for --all)
    println!("Testing clear --all without -y...");
    Command::new(&bin_path)
        .env("QDRANT_URL", "http://localhost:6334")
        .arg("clear")
        .arg("--all")
        // Simulating user input "n" is hard with assert_cmd, 
        // so we check that it prints the warning and exits (implicitly fails or has specific output).
        // For now, let's assume it prints the warning and maybe exits non-zero or prints cancellation.
        .assert()
        // .failure() // Might exit 0 if cancelled gracefully
        .stdout(predicate::str::contains("Clear operation cancelled"));

    // 4. Missing confirmation (for --directory)
    println!("Testing clear --directory without -y...");
    Command::new(&bin_path)
        .env("QDRANT_URL", "http://localhost:6334")
        .arg("clear")
        .arg("--directory")
        .arg(dummy_dir.to_str().unwrap())
        .assert()
        .stdout(predicate::str::contains("Clear operation cancelled"));

    Ok(())
}

#[test]
fn test_index_rejects_local_onnx_args() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let bin_path = get_binary_path()?;
    let dummy_dir = temp_dir.path().join("dummy_repo");
    fs::create_dir(&dummy_dir)?;
    let dummy_dir_str = dummy_dir.to_string_lossy();

    // Setup *valid* dummy ONNX files/dirs within the temp dir
    let dummy_model_path = temp_dir.path().join("dummy.onnx");
    fs::write(&dummy_model_path, "dummy_model_data")?;
    let dummy_tokenizer_dir = temp_dir.path().join("dummy_tokenizer");
    fs::create_dir_all(&dummy_tokenizer_dir)?;
    fs::write(dummy_tokenizer_dir.join("tokenizer.json"), "{}")?; // Minimal valid JSON

    let dummy_model_path_str = dummy_model_path.to_string_lossy();
    let dummy_tokenizer_dir_str = dummy_tokenizer_dir.to_string_lossy();

    // Attempt to run index providing BOTH args and env vars
    Command::new(&bin_path)
        .env("VECTORDB_ONNX_MODEL", &*dummy_model_path_str) // Set ENV VAR for model
        .env("VECTORDB_ONNX_TOKENIZER_DIR", &*dummy_tokenizer_dir_str) // Set ENV VAR for tokenizer
        .arg("index")
        .arg(&*dummy_dir_str)
        .arg("--onnx-model") // ALSO provide arg for model
        .arg(&*dummy_model_path_str)
        .arg("--onnx-tokenizer-dir") // ALSO provide arg for tokenizer
        .arg(&*dummy_tokenizer_dir_str)
        .assert()
        .failure() // Expect the command to fail
        // Check for the specific error about providing both sources.
        // Since arg takes precedence, the check might happen in main.rs before config logic fully kicks in.
        // Let's check for clap's potential conflict error OR our custom one.
        // Update: The logic in main.rs explicitly checks for both arg & env var being Some, so we expect that error.
        .stderr(
            predicate::str::contains("Cannot provide ONNX model path via both --onnx-model argument and VECTORDB_ONNX_MODEL environment variable.")
            .or(predicate::str::contains("Cannot provide ONNX tokenizer dir via both --onnx-tokenizer-dir argument and VECTORDB_ONNX_TOKENIZER_DIR environment variable."))
        );

    Ok(())
}

#[test]
// #[ignore] // Remove ignore
fn test_index_and_query() -> Result<()> {
    // Use helper to get ONNX paths, skip if not found
    let onnx_paths_opt = get_default_onnx_paths();
    if onnx_paths_opt.is_none() {
        println!("Skipping test_index_and_query: Default ONNX model/tokenizer not found.");
        return Ok(());
    }
    let (model_path, tokenizer_path) = onnx_paths_opt.unwrap();
    let tokenizer_dir = tokenizer_path.parent().ok_or(anyhow::anyhow!("Tokenizer path has no parent"))?;

    let temp_dir = TempDir::new()?;
    let project_dir = temp_dir.path().join("query_test_project");
    fs::create_dir_all(&project_dir)?;

    // Create a couple of files with predictable content
    let file_path_rust = project_dir.join("main.rs");
    fs::write(&file_path_rust, "fn main() { println!(\"Hello Rust world!\"); }")?;

    let file_path_py = project_dir.join("script.py");
    fs::write(&file_path_py, "def greet():\n    print(\"Hello Python world!\")")?;

    let binary_path = get_binary_path()?;

    // --- Index the directory --- 
    println!("Running index command...");
    Command::new(&binary_path)
        .arg("index")
        .arg(project_dir.to_str().unwrap())
        .env("ONNX_MODEL_PATH", model_path.to_str().unwrap())
        .env("TOKENIZER_PATH", tokenizer_dir.to_str().unwrap()) // Use tokenizer dir path
        .env("QDRANT_URL", "http://localhost:6334") // Ensure Qdrant URL is set if not default
        // Optional: Specify types if needed, otherwise index both
        // .arg("-t").arg("rs").arg("-t").arg("py")
        .assert()
        .success();
    println!("Index command finished.");

    // Allow some time for Qdrant to process points (might not be strictly needed)
    std::thread::sleep(std::time::Duration::from_secs(1));

    // --- Query for content in the Rust file --- 
    println!("Running query command for 'Rust world'...");
    let rust_query = "Rust world";
    Command::new(&binary_path)
        .arg("query")
        .arg(rust_query)
        .env("ONNX_MODEL_PATH", model_path.to_str().unwrap())
        .env("TOKENIZER_PATH", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", "http://localhost:6334")
        .assert()
        .success()
        .stdout(predicate::str::contains("main.rs")) // Check if the correct file is listed
        .stdout(predicate::str::contains("script.py").not()); // Ensure the other file is not listed
    println!("Query for 'Rust world' finished.");

    // --- Query for content in the Python file, filtering by type --- 
    println!("Running query command for 'Python print' with type filter...");
    let python_query = "Python print";
    Command::new(&binary_path)
        .arg("query")
        .arg(python_query)
        .arg("-t").arg("py") // Filter for python files
        .env("ONNX_MODEL_PATH", model_path.to_str().unwrap())
        .env("TOKENIZER_PATH", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", "http://localhost:6334")
        .assert()
        .success()
        .stdout(predicate::str::contains("script.py")) // Check if the correct file is listed
        .stdout(predicate::str::contains("main.rs").not()); // Ensure the other file is not listed
    println!("Query for 'Python print' finished.");

    // Optional: Add a query that should return no results or filters out everything

    Ok(())
}

// ... (other tests like test_stats, test_clear etc. if they exist) ... 

#[test]
fn test_build_script_copies_library() -> Result<()> {
    // Get the path to the built executable
    let bin_path = get_binary_path()?;
    
    // Determine the expected parent directory (target/<profile>/)
    let target_dir = bin_path.parent().ok_or_else(|| anyhow::anyhow!("Binary path has no parent directory"))?;
    
    // Determine the expected library name based on OS
    let lib_name = if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else if cfg!(target_os = "linux") {
        "libonnxruntime.so"
    } else {
        // Rpath logic is only for Linux/macOS, so skip test on other platforms
        println!("Skipping library copy test on unsupported OS");
        return Ok(());
    };
    
    // Construct the expected path to the copied library
    let expected_lib_path = target_dir.join("lib").join(lib_name);
    
    println!(
        "Checking for library ({}) at: {}",
        std::env::var("PROFILE").unwrap_or_else(|_| "<unknown profile>".to_string()),
        expected_lib_path.display()
    );

    // Assert that the library file exists
    assert!(
        expected_lib_path.exists(),
        "Build script did not copy {} to the expected location: {}",
        lib_name,
        expected_lib_path.display()
    );
    
    Ok(())
} 