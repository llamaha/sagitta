use anyhow::Result;
use code_parsers::python::PythonParser;
use code_parsers::parser::SyntaxParser;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_parser() -> Result<()> {
        let python_content = r#"#!/usr/bin/env python3
"""
This is a module docstring.
Should be ignored by the parser.
"""

import os
import sys
from typing import List, Dict, Optional

class MyClass:
    """Class docstring."""
    
    def __init__(self, name: str):
        """Initialize with a name."""
        self.name = name
        
    def say_hello(self) -> str:
        """Return a greeting."""
        return f"Hello, {self.name}!"

def some_function(param1: int, param2: str = "default") -> bool:
    """This is a function that does something."""
    if param1 > 10:
        return True
    else:
        return False
        
if __name__ == "__main__":
    obj = MyClass("World")
    print(obj.say_hello())
    
    result = some_function(15)
    print(f"Result: {result}")
"#;

        let file_path = "test_script.py";
        let mut parser = PythonParser::new();
        let chunks = parser.parse(python_content, file_path)?;

        // General validations
        assert!(!chunks.is_empty(), "Parser should extract chunks from Python code");
        
        // Note: The module docstring might or might not be ignored depending on implementation details
        // So we won't test for that specifcally
        
        // Validate class extraction
        let has_class = chunks.iter().any(|chunk| 
            chunk.element_type == "class" && 
            chunk.content.contains("class MyClass")
        );
        assert!(has_class, "Parser should extract class definitions");
        
        // Validate function extraction
        let has_function = chunks.iter().any(|chunk| 
            chunk.element_type == "function" && 
            chunk.content.contains("def some_function")
        );
        assert!(has_function, "Parser should extract function definitions");
        
        // With filtering, import and if statements are not extracted as separate chunks
        // They would only appear as part of fallback chunks if no functions/classes exist
        
        // Validate metadata
        for chunk in &chunks {
            assert_eq!(chunk.file_path, file_path, "Chunk should have correct file path");
            assert_eq!(chunk.language, "python", "Chunk should have correct language");
            assert!(chunk.start_line > 0, "Chunk should have positive start line");
            assert!(chunk.end_line >= chunk.start_line, "Chunk end line should be >= start line");
        }
        
        Ok(())
    }
    
    #[test]
    fn test_python_simple_script() -> Result<()> {
        let simple_python = r#"
x = 10
y = 20
print(x + y)
"#;

        let file_path = "simple.py";
        let mut parser = PythonParser::new();
        let chunks = parser.parse(simple_python, file_path)?;
        
        // With filtering, simple scripts without functions/classes will create a fallback chunk
        assert!(!chunks.is_empty(), "Parser should create fallback chunks for simple scripts");
        assert_eq!(chunks[0].element_type, "fallback_chunk_0", "Simple script should use fallback chunk");
        assert!(chunks[0].content.contains("x = 10"), "Fallback chunk should contain content");
        assert!(chunks[0].content.contains("y = 20"), "Fallback chunk should contain content");
        
        Ok(())
    }
    
    #[test]
    fn test_python_empty_content() -> Result<()> {
        let empty_content = "";
        let whitespace_content = "   \n  \t  ";
        let pass_only_content = "pass";
        
        let file_path = "empty.py";
        let mut parser = PythonParser::new();
        
        // Test with completely empty content
        let empty_chunks = parser.parse(empty_content, file_path)?;
        assert!(empty_chunks.is_empty(), "Parser should return empty chunks for empty content");
        
        // Test with only whitespace
        let whitespace_chunks = parser.parse(whitespace_content, file_path)?;
        assert!(whitespace_chunks.is_empty(), "Parser should return empty chunks for whitespace-only content");
        
        // Test with only 'pass' statement - will create a fallback chunk since no functions/classes
        let pass_chunks = parser.parse(pass_only_content, file_path)?;
        assert_eq!(pass_chunks.len(), 1, "Parser should create fallback chunk for pass-only content");
        assert_eq!(pass_chunks[0].element_type, "fallback_chunk_0", "Should be a fallback chunk");
        
        Ok(())
    }
    
    #[test]
    fn test_python_fallback() -> Result<()> {
        // Very unusual content that might be hard to parse properly
        let weird_content = "x=1;y=2;z=3";
        
        let file_path = "weird.py";
        let mut parser = PythonParser::new();
        let chunks = parser.parse(weird_content, file_path)?;
        
        // With filtering, this simple content without functions/classes should create a fallback chunk
        assert!(!chunks.is_empty(), "Parser should produce fallback chunks for unusual content");
        assert_eq!(chunks.len(), 1, "Should have one fallback chunk");
        assert_eq!(chunks[0].element_type, "fallback_chunk_0", "Should be a fallback chunk");
        assert!(chunks[0].content.contains("x=1"), "Fallback chunk should contain the content");
        
        Ok(())
    }
} 