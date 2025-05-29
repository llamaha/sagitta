use anyhow::Result;
use sagitta_search::syntax::python::PythonParser;
use sagitta_search::syntax::parser::SyntaxParser;

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
        
        // Validate statement extraction (import)
        let has_import = chunks.iter().any(|chunk| 
            chunk.element_type == "statement" && 
            chunk.content.contains("import os")
        );
        assert!(has_import, "Parser should extract import statements");
        
        // Validate statement extraction (if)
        let has_if_statement = chunks.iter().any(|chunk| 
            chunk.element_type == "statement" && 
            chunk.content.contains("if __name__ ==")
        );
        assert!(has_if_statement, "Parser should extract if statements");
        
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
        
        assert!(!chunks.is_empty(), "Parser should extract chunks even from simple scripts");
        
        // These should be extracted as statements or the file might be chunked as a whole
        let content_exists = if chunks[0].element_type == "file" {
            // If the whole file was chunked, check that the content is there
            chunks[0].content.contains("x = 10") && chunks[0].content.contains("y = 20")
        } else {
            // Otherwise check if statement extraction worked
            chunks.iter().any(|chunk| chunk.content.contains("x = 10"))
        };
        
        assert!(content_exists, "Parser should extract content from simple script");
        
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
        
        // Test with only 'pass' statement
        let pass_chunks = parser.parse(pass_only_content, file_path)?;
        assert!(pass_chunks.is_empty(), "Parser should skip top-level 'pass' statements");
        
        Ok(())
    }
    
    #[test]
    fn test_python_fallback() -> Result<()> {
        // Very unusual content that might be hard to parse properly
        let weird_content = "x=1;y=2;z=3";
        
        let file_path = "weird.py";
        let mut parser = PythonParser::new();
        let chunks = parser.parse(weird_content, file_path)?;
        
        // Should produce some chunks - either fallback or parsed statements
        assert!(!chunks.is_empty(), "Parser should produce chunks for unusual content");
        
        // Check that the content exists in some form
        let has_content = chunks.iter().any(|chunk| chunk.content.contains("x=1"));
        
        assert!(has_content, "Parser should capture unusual content in some form");
        
        Ok(())
    }
} 