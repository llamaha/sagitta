#[cfg(test)]
mod tests {
    use crate::syntax::parser::{CodeChunk, SyntaxParser};
    use crate::syntax::python::PythonParser;
    use anyhow::Result;

    fn create_parser() -> PythonParser {
        PythonParser::new()
    }

    // Helper to assert chunk properties
    fn assert_chunk(
        chunk: &CodeChunk,
        expected_content_trimmed: &str,
        expected_start: usize,
        expected_end: usize,
        expected_element: &str,
    ) {
        assert_eq!(chunk.content.trim(), expected_content_trimmed.trim());
        assert_eq!(chunk.start_line, expected_start);
        assert_eq!(chunk.end_line, expected_end);
        assert_eq!(chunk.language, "python");
        assert_eq!(chunk.element_type, expected_element);
        assert!(!chunk.file_path.is_empty());
    }

    #[test]
    fn test_parse_simple_function_py() -> Result<()> {
        let code = r#"
# A simple function
def greet(name):
    print(f"Hello, {name}!")

# Another comment
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;

        assert_eq!(chunks.len(), 1, "Expected one function chunk");
        assert_chunk(&chunks[0], "def greet(name):\n    print(f\"Hello, {name}!\")", 3, 4, "function");

        Ok(())
    }

    #[test]
    fn test_parse_class_py() -> Result<()> {
        let code = r#"
class MyClass:
    """A simple class"""
    def __init__(self, value):
        self.value = value

    def get_value(self):
        return self.value

# Top-level statement
instance = MyClass(10)
print(instance.get_value())
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;

        // Expecting Class, Assignment, Function Call (methods not captured)
        assert_eq!(chunks.len(), 3, "Expected class, assignment, print call");
        // Class chunk ends at line 8
        assert_chunk(&chunks[0], "class MyClass:\n    \"\"\"A simple class\"\"\"\n    def __init__(self, value):\n        self.value = value\n\n    def get_value(self):\n        return self.value", 2, 8, "class");
        // Assignment statement
        assert_chunk(&chunks[1], "instance = MyClass(10)", 11, 11, "statement");
        // Print call statement
        assert_chunk(&chunks[2], "print(instance.get_value())", 12, 12, "statement");

        Ok(())
    }

    #[test]
    fn test_parse_decorated_function_py() -> Result<()> {
        let code = r#"
import functools

@functools.lru_cache(maxsize=None)
def fibonacci(n):
    if n < 2:
        return n
    return fibonacci(n-1) + fibonacci(n-2)

result = fibonacci(5)
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;

        // Expecting Import, Decorated Function, Assignment
        assert_eq!(chunks.len(), 3, "Expected import, decorated function, assignment");
        assert_chunk(&chunks[0], "import functools", 2, 2, "statement");
        // Decorated function chunk ends at line 8
        assert_chunk(&chunks[1], "@functools.lru_cache(maxsize=None)\ndef fibonacci(n):\n    if n < 2:\n        return n\n    return fibonacci(n-1) + fibonacci(n-2)", 4, 8, "function");
        assert_chunk(&chunks[2], "result = fibonacci(5)", 10, 10, "statement");

        Ok(())
    }

    #[test]
    fn test_parse_top_level_statements_py() -> Result<()> {
        let code = r#"
"""Module docstring - should be ignored"""

import os

VARIABLE = "test"

def func():
    pass # Function definition is a chunk

print(VARIABLE)

if __name__ == "__main__":
    func()

pass # Top-level pass should be ignored
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;

        // Expecting Import, Assignment, Function Def, Print, If block
        assert_eq!(chunks.len(), 5, "Expected 5 top-level chunks");
        assert_chunk(&chunks[0], "import os", 4, 4, "statement");
        assert_chunk(&chunks[1], "VARIABLE = \"test\"", 6, 6, "statement");
        assert_chunk(&chunks[2], "def func():\n    pass # Function definition is a chunk", 8, 9, "function");
        assert_chunk(&chunks[3], "print(VARIABLE)", 11, 11, "statement");
        assert_chunk(&chunks[4], "if __name__ == \"__main__\":\n    func()", 13, 14, "statement");

        Ok(())
    }

     #[test]
    fn test_parse_empty_input_py() -> Result<()> {
        let code = "";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;
        assert!(chunks.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_comments_only_py() -> Result<()> {
        let code = r#"
# This is a comment
# Another comment
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;
        // Expect fallback to file chunk as comments aren't captured by the specific query
        assert_eq!(chunks.len(), 1, "Expected fallback file chunk");
        assert_eq!(chunks[0].element_type, "file", "Expected element type file");
        assert_eq!(chunks[0].language, "python");
        Ok(())
    }

     #[test]
    fn test_parse_pass_only_py() -> Result<()> {
        let code = r#"
pass
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;
        // Expect fallback to file chunk as top-level 'pass' is ignored by predicate
        assert_eq!(chunks.len(), 1, "Expected fallback file chunk");
        assert_eq!(chunks[0].element_type, "file", "Expected element type file");
        Ok(())
    }
} 