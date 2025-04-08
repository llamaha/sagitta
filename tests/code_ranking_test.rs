use anyhow::Result;
use std::fs;
use std::io::Write;
use tempfile::tempdir;
use vectordb_cli::vectordb::code_ranking::{CodeRankingEngine, FileCategory};
use vectordb_cli::vectordb::search::SearchResult;

#[test]
fn test_code_ranking_categorization() {
    let engine = CodeRankingEngine::new();

    // Test files
    assert_eq!(
        engine.categorize_file("src/test/test_parser.rs"),
        FileCategory::Test
    );
    assert_eq!(
        engine.categorize_file("tests/integration_tests/parser_test.go"),
        FileCategory::Test
    );
    assert_eq!(
        engine.categorize_file("spec/models/user_spec.rb"),
        FileCategory::Test
    );

    // Mock files
    assert_eq!(
        engine.categorize_file("src/mocks/mock_database.rs"),
        FileCategory::Mock
    );
    assert_eq!(
        engine.categorize_file("test/stubs/stub_client.rb"),
        FileCategory::Test
    );

    // Documentation
    assert_eq!(
        engine.categorize_file("docs/API.md"),
        FileCategory::Documentation
    );
    assert_eq!(
        engine.categorize_file("README.md"),
        FileCategory::Documentation
    );

    // Configuration
    assert_eq!(
        engine.categorize_file("config/app.yaml"),
        FileCategory::Configuration
    );
    assert_eq!(
        engine.categorize_file(".gitignore"),
        FileCategory::Configuration
    );
    assert_eq!(
        engine.categorize_file("Dockerfile"),
        FileCategory::Configuration
    );

    // Implementation
    assert_eq!(
        engine.categorize_file("src/models/user.rb"),
        FileCategory::MainImplementation
    );
    assert_eq!(
        engine.categorize_file("lib/parser.rs"),
        FileCategory::MainImplementation
    );
}

#[test]
fn test_code_ranking() -> Result<()> {
    // Create a temporary directory for test files
    let dir = tempdir()?;

    // Create some test files
    let main_impl_path = dir.path().join("main_impl.rs");
    let test_file_path = dir.path().join("test_main.rs");
    let mock_file_path = dir.path().join("mock_service.rs");
    let interface_path = dir.path().join("interface.rs");
    let doc_file_path = dir.path().join("README.md");

    // Write content to the files
    let main_impl_content = r#"
fn main() {
    println!("This is a main implementation file");
}

pub struct User {
    name: String,
    age: u32,
}

impl User {
    pub fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }
    
    pub fn greet(&self) {
        println!("Hello, {}!", self.name);
    }
}
    "#;

    let test_file_content = r#"
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_user_creation() {
        let user = User::new("Alice".to_string(), 30);
        assert_eq!(user.name, "Alice");
        assert_eq!(user.age, 30);
    }
}
    "#;

    let mock_file_content = r#"
pub struct MockUser {
    name: String,
    age: u32,
    greet_called: bool,
}

impl MockUser {
    pub fn new() -> Self {
        Self {
            name: "Test".to_string(),
            age: 0,
            greet_called: false,
        }
    }
    
    pub fn greet(&mut self) {
        self.greet_called = true;
    }
    
    pub fn was_greet_called(&self) -> bool {
        self.greet_called
    }
}
    "#;

    let interface_content = r#"
pub trait UserTrait {
    fn new(name: String, age: u32) -> Self;
    fn greet(&self);
    fn get_name(&self) -> &str;
    fn get_age(&self) -> u32;
}
    "#;

    let doc_content = "# User Module\n\nThis is documentation for the User module.";

    fs::write(&main_impl_path, main_impl_content)?;
    fs::write(&test_file_path, test_file_content)?;
    fs::write(&mock_file_path, mock_file_content)?;
    fs::write(&interface_path, interface_content)?;
    fs::write(&doc_file_path, doc_content)?;

    // Create search results with equal initial scores
    let mut results = vec![
        SearchResult {
            file_path: main_impl_path.to_string_lossy().to_string(),
            similarity: 0.8,
            snippet: "".to_string(),
            code_context: None,
            repository: None,
            branch: None,
            commit: None,
        },
        SearchResult {
            file_path: test_file_path.to_string_lossy().to_string(),
            similarity: 0.8,
            snippet: "".to_string(),
            code_context: None,
            repository: None,
            branch: None,
            commit: None,
        },
        SearchResult {
            file_path: mock_file_path.to_string_lossy().to_string(),
            similarity: 0.8,
            snippet: "".to_string(),
            code_context: None,
            repository: None,
            branch: None,
            commit: None,
        },
        SearchResult {
            file_path: interface_path.to_string_lossy().to_string(),
            similarity: 0.8,
            snippet: "".to_string(),
            code_context: None,
            repository: None,
            branch: None,
            commit: None,
        },
        SearchResult {
            file_path: doc_file_path.to_string_lossy().to_string(),
            similarity: 0.8,
            snippet: "".to_string(),
            code_context: None,
            repository: None,
            branch: None,
            commit: None,
        },
    ];

    // Rank the results
    let mut engine = CodeRankingEngine::new();
    engine.rank_results(&mut results, "user")?;

    // Check if the results are now ranked correctly
    // Main implementation should be ranked higher than tests and mocks
    let main_impl_idx = results
        .iter()
        .position(|r| r.file_path.contains("main_impl.rs"))
        .unwrap();
    let test_file_idx = results
        .iter()
        .position(|r| r.file_path.contains("test_main.rs"))
        .unwrap();
    let mock_file_idx = results
        .iter()
        .position(|r| r.file_path.contains("mock_service.rs"))
        .unwrap();
    let doc_file_idx = results
        .iter()
        .position(|r| r.file_path.contains("README.md"))
        .unwrap();

    // Verify main implementation is ranked higher than test file
    assert!(
        main_impl_idx < test_file_idx,
        "Main implementation should be ranked higher than test file"
    );

    // Verify main implementation is ranked higher than mock file
    assert!(
        main_impl_idx < mock_file_idx,
        "Main implementation should be ranked higher than mock file"
    );

    // Verify documentation is ranked lowest
    assert!(
        doc_file_idx > main_impl_idx
            && doc_file_idx > test_file_idx
            && doc_file_idx > mock_file_idx,
        "Documentation should be ranked lowest"
    );

    // Verify the explanation factors were added
    engine.add_explanation_factors(&mut results);
    for result in &results {
        assert!(
            result.code_context.is_some(),
            "Explanation factors should have been added to each result"
        );
    }

    Ok(())
}

#[test]
fn test_complexity_calculation() -> Result<()> {
    // Create a temporary directory
    let dir = tempdir()?;

    // Create files with different complexity levels
    let simple_file_path = dir.path().join("simple.rs");
    let medium_file_path = dir.path().join("medium.rs");
    let complex_file_path = dir.path().join("complex.rs");

    // Simple file with few lines and one function
    let simple_content = r#"
fn main() {
    println!("This is a simple file");
}
    "#;

    // Medium complexity file with multiple functions and types
    let medium_content = r#"
struct User {
    name: String,
    age: u32,
}

impl User {
    pub fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }
    
    pub fn greet(&self) {
        println!("Hello, {}!", self.name);
    }
}

fn process_user(user: &User) {
    println!("Processing user: {}", user.name);
    user.greet();
}

fn main() {
    let user = User::new("Alice".to_string(), 30);
    process_user(&user);
}
    "#;

    // Complex file with many functions, types, and imports
    let complex_content = r#"
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone)]
struct User {
    id: u64,
    name: String,
    age: u32,
    email: Option<String>,
    roles: HashSet<String>,
    metadata: HashMap<String, String>,
}

impl User {
    pub fn new(id: u64, name: String, age: u32) -> Self {
        Self {
            id,
            name,
            age,
            email: None,
            roles: HashSet::new(),
            metadata: HashMap::new(),
        }
    }
    
    pub fn with_email(mut self, email: String) -> Self {
        self.email = Some(email);
        self
    }
    
    pub fn add_role(&mut self, role: String) {
        self.roles.insert(role);
    }
    
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
    
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_string(self).unwrap();
        fs::write(path, json)
    }
    
    pub fn load(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        let user = serde_json::from_str(&content).unwrap();
        Ok(user)
    }
}

struct UserService {
    users: HashMap<u64, User>,
    next_id: u64,
}

impl UserService {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            next_id: 1,
        }
    }
    
    pub fn create_user(&mut self, name: String, age: u32) -> User {
        let id = self.next_id;
        self.next_id += 1;
        
        let user = User::new(id, name, age);
        self.users.insert(id, user.clone());
        user
    }
    
    pub fn get_user(&self, id: u64) -> Option<&User> {
        self.users.get(&id)
    }
    
    pub fn delete_user(&mut self, id: u64) -> bool {
        self.users.remove(&id).is_some()
    }
    
    pub fn find_users_by_age(&self, age: u32) -> Vec<&User> {
        self.users.values().filter(|u| u.age == age).collect()
    }
}

fn process_users_parallel(service: Arc<Mutex<UserService>>, ages: Vec<u32>) -> HashMap<u32, Vec<User>> {
    let results = Arc::new(Mutex::new(HashMap::new()));
    
    let handles: Vec<_> = ages.into_iter().map(|age| {
        let service_clone = Arc::clone(&service);
        let results_clone = Arc::clone(&results);
        
        thread::spawn(move || {
            let service = service_clone.lock().unwrap();
            let users = service.find_users_by_age(age);
            let users_cloned: Vec<User> = users.into_iter().cloned().collect();
            
            let mut results = results_clone.lock().unwrap();
            results.insert(age, users_cloned);
        })
    }).collect();
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    Arc::try_unwrap(results).unwrap().into_inner().unwrap()
}

fn main() {
    let service = Arc::new(Mutex::new(UserService::new()));
    
    // Create some users
    {
        let mut service = service.lock().unwrap();
        service.create_user("Alice".to_string(), 30);
        service.create_user("Bob".to_string(), 25);
        service.create_user("Charlie".to_string(), 30);
        service.create_user("Dave".to_string(), 40);
        service.create_user("Eve".to_string(), 25);
    }
    
    // Process users in parallel
    let results = process_users_parallel(service, vec![25, 30, 40]);
    
    // Print results
    for (age, users) in results {
        println!("Users with age {}: {}", age, users.len());
    }
}
    "#;

    fs::write(&simple_file_path, simple_content)?;
    fs::write(&medium_file_path, medium_content)?;
    fs::write(&complex_file_path, complex_content)?;

    // Calculate complexity
    let mut engine = CodeRankingEngine::new();
    let simple_complexity = engine.calculate_complexity(&simple_file_path.to_string_lossy());
    let medium_complexity = engine.calculate_complexity(&medium_file_path.to_string_lossy());
    let complex_complexity = engine.calculate_complexity(&complex_file_path.to_string_lossy());

    // Verify simple file has lower complexity
    assert!(
        simple_complexity < medium_complexity,
        "Simple file should have lower complexity than medium file"
    );

    // Verify complex file has higher complexity
    assert!(
        complex_complexity > medium_complexity,
        "Complex file should have higher complexity than medium file"
    );

    // Check if values are in expected ranges
    assert!(
        simple_complexity < 0.3,
        "Simple file should have low complexity score"
    );

    Ok(())
}
