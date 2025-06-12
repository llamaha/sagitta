use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sagitta_embed::{EmbeddingHandler, EmbeddingConfig};
use crate::utils::errors::SagittaCodeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CommandRiskLevel {
    /// Safe commands that can auto-execute (ls, pwd, cat, etc.)
    Safe,
    /// Medium risk commands that need user approval but are common (mkdir, cp, mv)
    Medium,
    /// High risk commands that should always require approval (rm -rf, sudo, dd)
    High,
    /// System-level commands that could be destructive (format, fdisk, systemctl)
    Critical,
}

impl CommandRiskLevel {
    /// Check if this risk level should auto-execute
    pub fn should_auto_execute(&self) -> bool {
        matches!(self, CommandRiskLevel::Safe)
    }
    
    /// Get a human-readable description of the risk level
    pub fn description(&self) -> &'static str {
        match self {
            CommandRiskLevel::Safe => "Safe to execute automatically",
            CommandRiskLevel::Medium => "Common command requiring approval",
            CommandRiskLevel::High => "Potentially risky command",
            CommandRiskLevel::Critical => "Critical system command - use extreme caution",
        }
    }
}

#[derive(Debug)]
pub struct CommandRiskAnalyzer {
    embedding_handler: Option<EmbeddingHandler>,
    // Cache for recently analyzed commands to avoid re-embedding
    risk_cache: HashMap<String, CommandRiskLevel>,
    // Pre-defined patterns for instant classification (fallback)
    safe_patterns: Vec<String>,
    critical_patterns: Vec<String>,
}

impl CommandRiskAnalyzer {
    /// Create a new command risk analyzer with optional embedding support
    pub fn new(embedding_config: Option<EmbeddingConfig>) -> Result<Self, SagittaCodeError> {
        let embedding_handler = if let Some(config) = embedding_config {
            Some(EmbeddingHandler::new(&config)
                .map_err(|e| SagittaCodeError::ToolError(format!("Failed to initialize embedding handler: {}", e)))?)
        } else {
            None
        };

        let safe_patterns = vec![
            // File operations (read-only or minimal impact)
            "ls".to_string(), "pwd".to_string(), "cd ".to_string(),
            "cat ".to_string(), "head ".to_string(), "tail ".to_string(), "less ".to_string(), "more ".to_string(),
            "find ".to_string(), "grep ".to_string(), "awk ".to_string(), "sed ".to_string(),
            "echo ".to_string(), "printf ".to_string(), "which ".to_string(), "where ".to_string(),
            "type ".to_string(), "file ".to_string(), "stat ".to_string(), "wc ".to_string(),
            
            // System info (read-only)
            "uname".to_string(), "whoami".to_string(), "id".to_string(), "env".to_string(),
            "date".to_string(), "uptime".to_string(), "ps ".to_string(), "top".to_string(),
            "free".to_string(), "df ".to_string(), "du ".to_string(), "lscpu".to_string(),
            
            // Development tools (safe in most contexts) - be more specific
            "git status".to_string(), "git log".to_string(), "git diff".to_string(), "git show".to_string(),
            "cargo check".to_string(), "cargo test".to_string(),
            "npm test".to_string(),
        ];

        let critical_patterns = vec![
            // File system destruction
            "rm -rf".to_string(), "rmdir".to_string(), "del /f".to_string(), "format".to_string(),
            "fdisk".to_string(), "mkfs".to_string(), "dd".to_string(),
            
            // System administration
            "sudo".to_string(), "su".to_string(), "systemctl".to_string(), "service".to_string(),
            "chmod 777".to_string(), "chown".to_string(), "mount".to_string(), "umount".to_string(),
            
            // Network/security sensitive
            "iptables".to_string(), "firewall".to_string(), "netsh".to_string(),
            "regedit".to_string(), "reg delete".to_string(), "crontab".to_string(),
            
            // Package management (can modify system)
            "apt install".to_string(), "yum install".to_string(), "brew install".to_string(),
            "pip install".to_string(), "npm install -g".to_string(), "cargo install".to_string(),
        ];

        Ok(Self {
            embedding_handler,
            risk_cache: HashMap::new(),
            safe_patterns,
            critical_patterns,
        })
    }

    /// Analyze the risk level of a command using semantic embeddings
    pub async fn analyze_command(&mut self, command: &str) -> Result<CommandRiskLevel, SagittaCodeError> {
        // Check cache first
        if let Some(&risk_level) = self.risk_cache.get(command) {
            return Ok(risk_level);
        }

        // Quick pattern-based classification (fallback)
        let pattern_risk = self.classify_by_patterns(command);
        
        // If we have embedding support, use semantic analysis
        if let Some(ref handler) = self.embedding_handler {
            match self.semantic_risk_analysis(handler, command).await {
                Ok(semantic_risk) => {
                    // Combine pattern-based and semantic analysis
                    let final_risk = std::cmp::max(pattern_risk, semantic_risk);
                    self.risk_cache.insert(command.to_string(), final_risk);
                    return Ok(final_risk);
                }
                Err(e) => {
                    log::warn!("Semantic analysis failed, falling back to pattern matching: {}", e);
                }
            }
        }

        // Fall back to pattern-based classification
        self.risk_cache.insert(command.to_string(), pattern_risk);
        Ok(pattern_risk)
    }

    /// Classify command risk using pattern matching (fast fallback)
    fn classify_by_patterns(&self, command: &str) -> CommandRiskLevel {
        let cmd_lower = command.to_lowercase();
        
        // Check for critical patterns first
        for pattern in &self.critical_patterns {
            if cmd_lower.contains(pattern) {
                return CommandRiskLevel::Critical;
            }
        }
        
        // Check for explicitly safe patterns
        for pattern in &self.safe_patterns {
            if cmd_lower.starts_with(pattern) {
                return CommandRiskLevel::Safe;
            }
            // For patterns that don't end with space, also check if command is exactly the pattern
            if !pattern.ends_with(' ') && cmd_lower == *pattern {
                return CommandRiskLevel::Safe;
            }
        }
        
        // Detect some high-risk patterns
        if cmd_lower.contains("rm ") || cmd_lower.contains("delete") || cmd_lower.contains("destroy") ||
           cmd_lower.contains("kill") || cmd_lower.contains("stop") || cmd_lower.contains("reboot") ||
           cmd_lower.contains("shutdown") {
            return CommandRiskLevel::High;
        }
        
        // Common development operations
        if cmd_lower.contains("build") || cmd_lower.contains("test") || cmd_lower.contains("compile") ||
           cmd_lower.contains("run") || cmd_lower.contains("start") {
            return CommandRiskLevel::Medium;
        }
        
        // File system operations that need approval but are common
        if cmd_lower.starts_with("mkdir") || cmd_lower.starts_with("cp ") || cmd_lower.starts_with("mv ") ||
           cmd_lower.starts_with("touch") || cmd_lower.starts_with("ln ") || cmd_lower.starts_with("chmod") {
            return CommandRiskLevel::Medium;
        }
        
        // Default to medium risk for unknown commands
        CommandRiskLevel::Medium
    }

    /// Use semantic embeddings to analyze command risk
    async fn semantic_risk_analysis(&self, handler: &EmbeddingHandler, command: &str) -> Result<CommandRiskLevel, SagittaCodeError> {
        // Create reference embeddings for different risk categories
        let safe_commands = vec![
            "list files in current directory",
            "show file contents safely",
            "check git status without changes",
            "read system information",
            "display environment variables",
        ];
        
        let medium_commands = vec![
            "create new directory or file",
            "copy files to different location", 
            "move files around filesystem",
            "run development build process",
            "start local development server",
        ];
        
        let high_commands = vec![
            "delete files from filesystem",
            "remove directories permanently", 
            "modify system permissions",
            "kill running processes",
            "stop system services",
        ];
        
        let critical_commands = vec![
            "format hard drive completely",
            "delete all files recursively", 
            "modify system configuration",
            "install system-wide software",
            "change administrative privileges",
        ];

        // Generate embeddings
        let command_embedding = handler.embed(&[command])
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to embed command: {}", e)))?;
        
        let safe_embeddings = handler.embed(&safe_commands)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to embed safe commands: {}", e)))?;
        let medium_embeddings = handler.embed(&medium_commands)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to embed medium commands: {}", e)))?;
        let high_embeddings = handler.embed(&high_commands)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to embed high commands: {}", e)))?;
        let critical_embeddings = handler.embed(&critical_commands)
            .map_err(|e| SagittaCodeError::ToolError(format!("Failed to embed critical commands: {}", e)))?;

        if command_embedding.is_empty() {
            return Err(SagittaCodeError::ToolError("No embedding generated for command".to_string()));
        }

        let cmd_emb = &command_embedding[0];
        
        // Calculate average similarities to each risk category
        let safe_similarity = average_similarity(cmd_emb, &safe_embeddings);
        let medium_similarity = average_similarity(cmd_emb, &medium_embeddings);
        let high_similarity = average_similarity(cmd_emb, &high_embeddings);
        let critical_similarity = average_similarity(cmd_emb, &critical_embeddings);
        
        // Find the highest similarity
        let max_similarity = safe_similarity.max(medium_similarity).max(high_similarity).max(critical_similarity);
        
        // Classify based on highest similarity with confidence threshold
        if max_similarity < 0.6 {
            // Low confidence, default to medium risk
            Ok(CommandRiskLevel::Medium)
        } else if critical_similarity == max_similarity {
            Ok(CommandRiskLevel::Critical)
        } else if high_similarity == max_similarity {
            Ok(CommandRiskLevel::High)
        } else if medium_similarity == max_similarity {
            Ok(CommandRiskLevel::Medium)
        } else {
            Ok(CommandRiskLevel::Safe)
        }
    }

    /// Clear the risk cache (useful for testing or memory management)
    pub fn clear_cache(&mut self) {
        self.risk_cache.clear();
    }

    /// Get cache statistics
    pub fn cache_size(&self) -> usize {
        self.risk_cache.len()
    }
}

/// Calculate average cosine similarity between one embedding and a set of embeddings
fn average_similarity(target: &[f32], embeddings: &[Vec<f32>]) -> f32 {
    if embeddings.is_empty() {
        return 0.0;
    }
    
    let sum: f32 = embeddings.iter()
        .map(|emb| cosine_similarity(target, emb))
        .sum();
    
    sum / embeddings.len() as f32
}

/// Calculate cosine similarity between two embeddings
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot_product / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pattern_based_classification() {
        let mut analyzer = CommandRiskAnalyzer::new(None).expect("Failed to create analyzer");
        
        // Test safe commands
        assert_eq!(analyzer.analyze_command("ls -la").await.unwrap(), CommandRiskLevel::Safe);
        assert_eq!(analyzer.analyze_command("cat README.md").await.unwrap(), CommandRiskLevel::Safe);
        assert_eq!(analyzer.analyze_command("git status").await.unwrap(), CommandRiskLevel::Safe);
        
        // Test medium risk commands
        assert_eq!(analyzer.analyze_command("mkdir new_folder").await.unwrap(), CommandRiskLevel::Medium);
        assert_eq!(analyzer.analyze_command("cargo build --release").await.unwrap(), CommandRiskLevel::Medium);
        
        // Test high risk commands
        assert_eq!(analyzer.analyze_command("rm important_file.txt").await.unwrap(), CommandRiskLevel::High);
        assert_eq!(analyzer.analyze_command("kill -9 1234").await.unwrap(), CommandRiskLevel::High);
        
        // Test critical commands
        assert_eq!(analyzer.analyze_command("sudo rm -rf /").await.unwrap(), CommandRiskLevel::Critical);
        assert_eq!(analyzer.analyze_command("dd if=/dev/zero of=/dev/sda").await.unwrap(), CommandRiskLevel::Critical);
    }
    
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
        
        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&c, &d) - 0.0).abs() < 1e-6);
    }
    
    #[tokio::test]
    async fn test_cache_functionality() {
        let mut analyzer = CommandRiskAnalyzer::new(None).expect("Failed to create analyzer");
        
        assert_eq!(analyzer.cache_size(), 0);
        
        // Analyze a command
        let _ = analyzer.analyze_command("ls -la").await.unwrap();
        assert_eq!(analyzer.cache_size(), 1);
        
        // Analyze same command again (should hit cache)
        let _ = analyzer.analyze_command("ls -la").await.unwrap();
        assert_eq!(analyzer.cache_size(), 1);
        
        // Clear cache
        analyzer.clear_cache();
        assert_eq!(analyzer.cache_size(), 0);
    }
} 