use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
//use std::path::Path;
//use tempfile::tempdir;
use std::path::PathBuf;
use tempfile::TempDir;
use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};
use git2::{Repository, Signature, Commit, Oid};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{CountPointsBuilder, Filter, Condition};
use vectordb_lib::config::AppConfig;
use vectordb_lib::cli::commands::{FIELD_FILE_PATH, FIELD_BRANCH, FIELD_COMMIT_HASH};

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
    let config_dir = temp_dir.path().join("config"); // Use separate config for test
    let data_dir = temp_dir.path().join("data");
    std::env::set_var("XDG_CONFIG_HOME", config_dir.to_str().unwrap());
    std::env::set_var("XDG_DATA_HOME", data_dir.to_str().unwrap());

    // Test that clear works without active repo now (clears simple index)
    println!("Testing clear -y without active repo (should succeed for simple index)...");
    Command::new(&bin_path)
        .arg("clear")
        .arg("-y")
        .assert()
        .success() // Should now succeed
        .stdout(predicate::str::contains("Deleting collection 'vectordb-code-search'")); // Check it targets the correct collection
        // .stderr(...); // No specific error expected

    // Test cancellation prompt
    println!("Setting up dummy repo for cancellation test...");
    let dummy_repo_url = "https://github.com/git-fixtures/basic.git"; // Use a known small public repo
    Command::new(&bin_path)
        .arg("repo")
        .arg("add")
        .arg("--url")
        .arg(dummy_repo_url)
        .arg("--name")
        .arg("dummy-for-clear")
        .assert()
        .success();

    println!("Testing clear without -y (expect cancellation prompt)...");
    Command::new(&bin_path)
        .arg("clear") // No -y
        .assert()
        .success() // Should exit successfully after cancellation
        .stdout(predicate::str::contains("Operation cancelled"));

    // Cleanup: Remove dummy repo
    Command::new(&bin_path)
        .arg("repo")
        .arg("remove")
        .arg("dummy-for-clear")
        .assert()
        .success();

    Ok(())
}

#[test]
fn test_index_rejects_local_onnx_args() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let bin_path = get_binary_path()?;
    let config_dir = temp_dir.path().join("config");
    let data_dir = temp_dir.path().join("data");
    std::env::set_var("XDG_CONFIG_HOME", config_dir.to_str().unwrap());
    std::env::set_var("XDG_DATA_HOME", data_dir.to_str().unwrap());

    // Setup dummy ONNX files
    let dummy_model_path = temp_dir.path().join("dummy.onnx");
    fs::write(&dummy_model_path, "dummy_model_data")?;
    let dummy_tokenizer_dir = temp_dir.path().join("dummy_tokenizer");
    fs::create_dir_all(&dummy_tokenizer_dir)?;
    fs::write(dummy_tokenizer_dir.join("tokenizer.json"), "{}")?;
    let dummy_model_path_str = dummy_model_path.to_string_lossy();
    let dummy_tokenizer_dir_str = dummy_tokenizer_dir.to_string_lossy();

    // 1. Add a dummy repo
    println!("Adding dummy repo...");
    let dummy_repo_url = "https://github.com/git-fixtures/basic.git";
    Command::new(&bin_path)
        .arg("repo")
        .arg("add")
        .arg("--url")
        .arg(dummy_repo_url)
        .arg("--name")
        .arg("dummy-for-index-reject")
        .assert()
        .success();

    // 2. Attempt to run index providing BOTH args and env vars
    println!("Attempting index with conflicting ONNX args/env vars...");
    Command::new(&bin_path)
        .env("VECTORDB_ONNX_MODEL", &*dummy_model_path_str)
        .env("VECTORDB_ONNX_TOKENIZER_DIR", &*dummy_tokenizer_dir_str)
        .arg("index") // Index command
        .arg(temp_dir.path().to_str().unwrap()) // Add dummy path argument
        .arg("--onnx-model")
        .arg(&*dummy_model_path_str)
        .arg("--onnx-tokenizer-dir")
        .arg(&*dummy_tokenizer_dir_str)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot provide ONNX model path via both")
            .or(predicate::str::contains("Cannot provide ONNX tokenizer dir via both")));

    // Remove the potentially problematic cleanup step
    // // 3. Cleanup: Remove dummy repo
    // println!("Removing dummy repo...");
    // Command::new(&bin_path)
    //     .arg("repo")
    //     .arg("remove")
    //     .arg("dummy-for-index-reject")
    //     .assert()
    //     .success();

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

// --- Helper Functions for Repo Sync Tests --- 

// Helper to create a unique suffix for collections/repos
fn unique_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis()
        .to_string()
}

// Helper to create and commit a file in a Git repo
fn create_and_commit(repo: &Repository, file_name: &str, content: &str, message: &str) -> Result<Oid> {
    let repo_path = repo.path().parent().unwrap(); // Get the workdir
    let file_path = repo_path.join(file_name);
    fs::write(&file_path, content)?;

    let mut index = repo.index()?;
    // Need to convert path relative to workdir
    let relative_path = file_path.strip_prefix(repo_path)?;
    index.add_path(relative_path)?;
    index.write()?;

    let oid = index.write_tree()?;
    let signature = Signature::now("Test User", "test@example.com")?;
    let parent_commit = find_last_commit(repo).ok();
    let parents = parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();

    // Convert Vec<&Commit> to Vec<&Commit> for the commit call
    let parents_ref: Vec<&Commit> = parents.iter().map(|&c| c).collect();

    let tree = repo.find_tree(oid)?;

    repo.commit(
        Some("HEAD"),      // point HEAD to our new commit
        &signature,      // author
        &signature,      // committer
        message,         // message
        &tree,           // tree
        &parents_ref,    // parents
    ).map_err(anyhow::Error::from) // Convert git2::Error to anyhow::Error
}

// Helper to find the last commit
fn find_last_commit(repo: &Repository) -> Result<Commit<'_>, git2::Error> {
    let obj = repo.head()?.resolve()?.peel(git2::ObjectType::Commit)?;
    obj.into_commit().map_err(|_| git2::Error::from_str("Couldn't find commit"))
}

// Helper to read the app config
fn read_config(config_path: &PathBuf) -> Result<AppConfig> {
    let content = fs::read_to_string(config_path)?; 
    let config: AppConfig = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse config TOML: {}", e))?;
    Ok(config)
}

// Helper to get Qdrant point count for a specific file/branch/commit
async fn get_qdrant_point_count(
    client: &Qdrant, 
    collection_name: &str, 
    file_path: Option<&str>, 
    branch_name: Option<&str>,
    commit_hash: Option<&str>
) -> Result<u64> {
    let mut filters = Vec::new();
    if let Some(file) = file_path {
        filters.push(Condition::matches(FIELD_FILE_PATH, file.to_string()));
    }
    if let Some(branch) = branch_name {
        filters.push(Condition::matches(FIELD_BRANCH, branch.to_string()));
    }
    if let Some(commit) = commit_hash {
        filters.push(Condition::matches(FIELD_COMMIT_HASH, commit.to_string()));
    }

    // Construct the final filter
    let filter: Option<Filter> = if filters.is_empty() { 
        None 
    } else { 
        // Correct: 'must' expects Vec<Condition>
        Some(Filter { must: filters, ..Default::default() })
    };

    // Conditionally apply the filter to the builder
    let mut count_request = CountPointsBuilder::new(collection_name);
    if let Some(f) = filter {
        count_request = count_request.filter(f);
    }
    count_request = count_request.exact(true);

    let count_result = client.count(count_request).await?;
    Ok(count_result.result.unwrap().count)
}

// --- Main Repo Sync Test --- 

#[tokio::test]
async fn test_repo_sync_scenarios() -> Result<()> {
    let onnx_paths_opt = get_default_onnx_paths();
    if onnx_paths_opt.is_none() {
        println!("Skipping test_repo_sync_scenarios: Default ONNX files not found.");
        return Ok(());
    }
    let (model_path, tokenizer_path) = onnx_paths_opt.unwrap();
    let tokenizer_dir = tokenizer_path.parent().ok_or(anyhow::anyhow!("Tokenizer path has no parent"))?;

    let suffix = unique_suffix();
    let repo_name = format!("test-repo-{}", suffix);
    let collection_name = format!("repo_{}", repo_name);
    println!("Using repo name: {} and collection: {}", repo_name, collection_name);

    let temp_dir = TempDir::new()?;
    let bin_path = get_binary_path()?;
    let repo_local_path = temp_dir.path().join(&repo_name);
    let config_dir = temp_dir.path().join("config");
    let config_path = config_dir.join("vectordb-cli/config.toml");

    fs::create_dir_all(config_dir.join("vectordb-cli"))?;

    // Set env vars for config/data paths
    std::env::set_var("XDG_CONFIG_HOME", config_dir.to_str().unwrap());

    // Initialize Qdrant client
    let qdrant_url = std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string());
    let client = Qdrant::from_url(&qdrant_url).build()?;

    // Clean up collection if it exists from a previous failed run
    if client.collection_exists(&collection_name).await? {
        println!("Deleting existing collection: {}", collection_name);
        client.delete_collection(&collection_name).await?;
    }

    // 1. Setup: Init Git repo and add it
    println!("Initializing Git repo at {}", repo_local_path.display());
    let repo = Repository::init(&repo_local_path)?;
    let initial_commit_oid = create_and_commit(&repo, "README.md", "Initial commit", "feat: Initial commit")?;
    let initial_commit_hash = initial_commit_oid.to_string();
    println!("Initial commit: {}", initial_commit_hash);

    Command::new(&bin_path)
        .env("VECTORDB_ONNX_MODEL", model_path.to_str().unwrap())
        .env("VECTORDB_ONNX_TOKENIZER_DIR", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", &qdrant_url)
        .arg("repo")
        .arg("add")
        .arg(&repo_name)
        .arg("file://".to_owned() + repo_local_path.to_str().unwrap()) // Use file URL for local repo
        .arg("--local-path")
        .arg(repo_local_path.to_str().unwrap())
        .arg("--track-branch")
        .arg("main") // Assuming default branch is main
        .assert()
        .success();

    // --- Add Second Repo with Custom Remote ---
    let custom_remote_repo_name = format!("custom-remote-{}", suffix);
    let custom_remote_collection_name = format!("repo_{}", custom_remote_repo_name);
    let remote_name_arg = "test-remote";
    // Add the second remote manually to the existing local repo
    repo.remote(remote_name_arg, &("file://".to_owned() + repo_local_path.to_str().unwrap()))?;
    println!("Adding repo '{}' with remote '{}'...", custom_remote_repo_name, remote_name_arg);
    Command::new(&bin_path)
        .env("VECTORDB_ONNX_MODEL", model_path.to_str().unwrap()) // Needed if collection needs creation
        .env("VECTORDB_ONNX_TOKENIZER_DIR", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", &qdrant_url)
        .arg("repo")
        .arg("add")
        .arg(&custom_remote_repo_name) // New name
        .arg("file://".to_owned() + repo_local_path.to_str().unwrap()) // Same URL
        .arg("--local-path") // Specify same local path
        .arg(repo_local_path.to_str().unwrap())
        .arg("--remote") // Specify the custom remote
        .arg(remote_name_arg)
        .arg("--track-branch") // Need to specify branch if using same local path
        .arg("main")
        .assert()
        .success();
    println!("Repo with custom remote added.");
    // Clean up custom remote collection before potentially syncing later
    if client.collection_exists(&custom_remote_collection_name).await? {
        println!("Deleting existing custom remote collection: {}", custom_remote_collection_name);
        client.delete_collection(&custom_remote_collection_name).await?;
    }

    // Set repo as active (the first one for initial tests)
    Command::new(&bin_path)
        .arg("repo")
        .arg("use")
        .arg(&repo_name)
        .assert()
        .success();

    // 2. Initial Sync
    println!("Running initial sync...");
    Command::new(&bin_path)
        .env("VECTORDB_ONNX_MODEL", model_path.to_str().unwrap())
        .env("VECTORDB_ONNX_TOKENIZER_DIR", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", &qdrant_url)
        .arg("repo")
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("Indexing 1 added/renamed files"));

    // Verify config updated
    let config = read_config(&config_path)?;
    let repo_cfg = config.repositories.iter().find(|r| r.name == repo_name).expect("Repo config not found");
    assert_eq!(repo_cfg.last_synced_commits.get("main"), Some(&initial_commit_hash));
    // Verify points in Qdrant
    let initial_count = get_qdrant_point_count(&client, &collection_name, Some("README.md"), Some("main"), Some(&initial_commit_hash)).await?;
    assert!(initial_count > 0, "No points found after initial sync");

    // 3. No Change Sync
    println!("Running sync with no changes...");
    Command::new(&bin_path)
        .env("QDRANT_URL", &qdrant_url) // No model needed if no diff
        .arg("repo")
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("already up-to-date"))
        // Add assertion: Ensure no indexing happens when already up-to-date
        .stdout(predicate::str::contains("Indexing all relevant files").not())
        .stdout(predicate::str::contains("Indexing 1 added/renamed files").not())
        .stdout(predicate::str::contains("Indexing 1 modified files").not());

    // 4. Add File
    println!("Adding new file and syncing...");
    let add_commit_oid = create_and_commit(&repo, "src/main.rs", "fn main() { println!(\"Hello\"); }", "feat: Add main.rs")?;
    let add_commit_hash = add_commit_oid.to_string();
    Command::new(&bin_path)
        .env("VECTORDB_ONNX_MODEL", model_path.to_str().unwrap())
        .env("VECTORDB_ONNX_TOKENIZER_DIR", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", &qdrant_url)
        .arg("repo")
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("Indexing 1 added/renamed files"));
    let config = read_config(&config_path)?;
    let repo_cfg = config.repositories.iter().find(|r| r.name == repo_name).expect("Repo config not found");
    assert_eq!(repo_cfg.last_synced_commits.get("main"), Some(&add_commit_hash));
    let add_count = get_qdrant_point_count(&client, &collection_name, Some("src/main.rs"), Some("main"), Some(&add_commit_hash)).await?;
    assert!(add_count > 0, "No points found for added file");
    let readme_count_after_add = get_qdrant_point_count(&client, &collection_name, Some("README.md"), Some("main"), Some(&add_commit_hash)).await?;
    assert!(readme_count_after_add > 0, "README.md points disappeared after add sync"); // Ensure old file points still exist

    // 5. Modify File
    println!("Modifying file and syncing...");
    let modify_commit_oid = create_and_commit(&repo, "README.md", "Updated content", "docs: Update README")?;
    let modify_commit_hash = modify_commit_oid.to_string();
    Command::new(&bin_path)
        .env("VECTORDB_ONNX_MODEL", model_path.to_str().unwrap())
        .env("VECTORDB_ONNX_TOKENIZER_DIR", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", &qdrant_url)
        .arg("repo")
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("Indexing 1 modified files"));
    let config = read_config(&config_path)?;
    let repo_cfg = config.repositories.iter().find(|r| r.name == repo_name).expect("Repo config not found");
    assert_eq!(repo_cfg.last_synced_commits.get("main"), Some(&modify_commit_hash));
    // Check old points gone
    let old_readme_count = get_qdrant_point_count(&client, &collection_name, Some("README.md"), Some("main"), Some(&add_commit_hash)).await?;
    assert_eq!(old_readme_count, 0, "Old points for modified file were not deleted");
    // Check new points added
    let new_readme_count = get_qdrant_point_count(&client, &collection_name, Some("README.md"), Some("main"), Some(&modify_commit_hash)).await?;
    assert!(new_readme_count > 0, "New points for modified file were not added");

    // 6. Delete File
    println!("Deleting file and syncing...");
    let mut index = repo.index()?;
    index.remove_path(PathBuf::from("src/main.rs").as_path())?;
    index.write()?;
    let delete_tree_oid = index.write_tree()?;
    let delete_tree = repo.find_tree(delete_tree_oid)?;
    let delete_sig = Signature::now("Test User", "test@example.com")?;
    let delete_parent = find_last_commit(&repo)?;
    let delete_commit_oid = repo.commit(Some("HEAD"), &delete_sig, &delete_sig, "refactor: Remove main.rs", &delete_tree, &[&delete_parent])?;
    let delete_commit_hash = delete_commit_oid.to_string();
    Command::new(&bin_path)
        .env("QDRANT_URL", &qdrant_url) // No model needed for deletion only
        .arg("repo")
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("Removing data for 1 deleted/renamed files"));
    let config = read_config(&config_path)?;
    let repo_cfg = config.repositories.iter().find(|r| r.name == repo_name).expect("Repo config not found");
    assert_eq!(repo_cfg.last_synced_commits.get("main"), Some(&delete_commit_hash));
    let deleted_file_count = get_qdrant_point_count(&client, &collection_name, Some("src/main.rs"), Some("main"), None).await?; // Check across all commits
    assert_eq!(deleted_file_count, 0, "Points for deleted file were not removed");

    // 7. Rename File
    println!("Renaming file and syncing...");
    // Git rename: remove old, add new
    let mut index = repo.index()?;
    index.remove_path(PathBuf::from("README.md").as_path())?;
    fs::write(repo_local_path.join("NEW_README.md"), "Renamed content")?;
    index.add_path(PathBuf::from("NEW_README.md").as_path())?;
    index.write()?;
    let rename_tree_oid = index.write_tree()?;
    let rename_tree = repo.find_tree(rename_tree_oid)?;
    let rename_sig = Signature::now("Test User", "test@example.com")?;
    let rename_parent = find_last_commit(&repo)?;
    let rename_commit_oid = repo.commit(Some("HEAD"), &rename_sig, &rename_sig, "refactor: Rename README", &rename_tree, &[&rename_parent])?;
    let rename_commit_hash = rename_commit_oid.to_string();

    Command::new(&bin_path)
        .env("VECTORDB_ONNX_MODEL", model_path.to_str().unwrap())
        .env("VECTORDB_ONNX_TOKENIZER_DIR", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", &qdrant_url)
        .arg("repo")
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("Removing data for 1 deleted/renamed files"))
        .stdout(predicate::str::contains("Indexing 1 added/renamed files"));

    let config = read_config(&config_path)?;
    let repo_cfg = config.repositories.iter().find(|r| r.name == repo_name).expect("Repo config not found");
    assert_eq!(repo_cfg.last_synced_commits.get("main"), Some(&rename_commit_hash));
    let old_rename_count = get_qdrant_point_count(&client, &collection_name, Some("README.md"), Some("main"), None).await?;
    assert_eq!(old_rename_count, 0, "Points for old renamed file were not removed");
    let new_rename_count = get_qdrant_point_count(&client, &collection_name, Some("NEW_README.md"), Some("main"), Some(&rename_commit_hash)).await?;
    assert!(new_rename_count > 0, "Points for new renamed file were not added");

    // 8. Test Custom Remote Sync
    println!("Activating custom remote repo '{}'...", custom_remote_repo_name);
    Command::new(&bin_path)
        .arg("repo")
        .arg("use")
        .arg(&custom_remote_repo_name)
        .assert()
        .success();

    println!("Adding another commit to local repo for custom remote sync test...");
    let final_commit_oid = create_and_commit(&repo, "final.txt", "Final content", "feat: Add final file")?;
    let final_commit_hash = final_commit_oid.to_string();

    println!("Running sync for custom remote repo '{}'...", custom_remote_repo_name);
    Command::new(&bin_path)
        .env("VECTORDB_ONNX_MODEL", model_path.to_str().unwrap()) 
        .env("VECTORDB_ONNX_TOKENIZER_DIR", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", &qdrant_url)
        .arg("repo")
        .arg("sync") // Should sync the active repo (custom-remote-repo)
        .assert()
        .success()
        .stdout(predicate::str::contains("synced successfully"))
        .stdout(predicate::str::contains(format!("remote '{}'", remote_name_arg))); // Check fetch message mentions the correct remote
        
    println!("Sync for custom remote repo completed.");
    let config = read_config(&config_path)?;
    let custom_repo_cfg = config.repositories.iter().find(|r| r.name == custom_remote_repo_name).expect("Custom remote repo config not found");
    assert_eq!(custom_repo_cfg.last_synced_commits.get("main"), Some(&final_commit_hash)); // Check latest commit synced

    let custom_remote_point_count = get_qdrant_point_count(&client, &custom_remote_collection_name, Some("final.txt"), Some("main"), Some(&final_commit_hash)).await?;
    assert!(custom_remote_point_count > 0, "Sync via custom remote did not add point for final commit"); 
    println!("Point count for final commit in custom remote collection: {}", custom_remote_point_count);

    // 9. Switch Branch (Untracked)
    println!("Switching to untracked branch and attempting sync...");
    let develop_branch_name = "develop";
    let head_commit = find_last_commit(&repo)?;
    repo.branch(develop_branch_name, &head_commit, false)?;
    repo.checkout_tree(head_commit.as_object(), None)?;
    repo.set_head(&format!("refs/heads/{}", develop_branch_name))?;
    let _branch_commit = create_and_commit(&repo, "dev_file.txt", "Dev content", "feat: Add dev file")?;

    Command::new(&bin_path)
        .env("QDRANT_URL", &qdrant_url)
        .arg("repo")
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("branch '{}' is not tracked", develop_branch_name)));

    // 10. Switch Branch (Tracked)
    println!("Tracking new branch and syncing...");
    Command::new(&bin_path)
        .arg("repo")
        .arg("use-branch")
        .arg(develop_branch_name)
        .assert()
        .success();

    // Now sync should work for the develop branch
    Command::new(&bin_path)
        .env("VECTORDB_ONNX_MODEL", model_path.to_str().unwrap())
        .env("VECTORDB_ONNX_TOKENIZER_DIR", tokenizer_dir.to_str().unwrap())
        .env("QDRANT_URL", &qdrant_url)
        .arg("repo")
        .arg("sync")
        .assert()
        .success()
        .stdout(predicate::str::contains("Indexing 1 added/renamed files")); // Only dev_file.txt is new for this branch sync
    
    let config = read_config(&config_path)?;
    let repo_cfg = config.repositories.iter().find(|r| r.name == repo_name).expect("Repo config not found");
    let develop_sync_hash = repo_cfg.last_synced_commits.get(develop_branch_name).expect("Develop branch not synced");
    let dev_file_count = get_qdrant_point_count(&client, &collection_name, Some("dev_file.txt"), Some(develop_branch_name), Some(develop_sync_hash)).await?;
    assert!(dev_file_count > 0, "Points for dev branch file not found");
    // Ensure main branch points are still there but not associated with develop commit
    let main_file_count = get_qdrant_point_count(&client, &collection_name, Some("NEW_README.md"), Some("main"), None).await?;
    assert!(main_file_count > 0, "Main branch file points disappeared");
    let main_file_develop_commit_count = get_qdrant_point_count(&client, &collection_name, Some("NEW_README.md"), Some(develop_branch_name), Some(develop_sync_hash)).await?;
    assert_eq!(main_file_develop_commit_count, 0, "Main branch file points associated with develop commit");

    // Final Teardown
    println!("Cleaning up collection: {}", collection_name);
    client.delete_collection(&collection_name).await?;
    println!("Test completed successfully.");

    Ok(())
}

#[test]
fn test_cross_repo_query() -> Result<()> {
    // ... existing code ...
    Ok(())
} 