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
def simple_func():
    """This is a docstring."""
    print("Hello")

# A comment after
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;

        assert_eq!(chunks.len(), 1, "Expected function");
        assert_chunk(&chunks[0], "def simple_func():\n    \"\"\"This is a docstring.\"\"\"\n    print(\"Hello\")", 2, 4, "function");
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
    fn test_parse_async_function_py() -> Result<()> {
        let code = r#"
import asyncio

async def fetch_data():
    print("Fetching...")
    await asyncio.sleep(1)
    print("Fetched!")
    return {"data": 123}

async def main():
    data = await fetch_data()
    print(data)

asyncio.run(main())
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;

        // Expecting Import, async function, async function, statement
        assert_eq!(chunks.len(), 4);
        assert_chunk(&chunks[0], "import asyncio", 2, 2, "statement");
        assert_chunk(&chunks[1], "async def fetch_data():\n    print(\"Fetching...\")\n    await asyncio.sleep(1)\n    print(\"Fetched!\")\n    return {\"data\": 123}", 4, 8, "function");
        assert_chunk(&chunks[2], "async def main():\n    data = await fetch_data()\n    print(data)", 10, 12, "function");
        assert_chunk(&chunks[3], "asyncio.run(main())", 14, 14, "statement");

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
""" This is a
multiline docstring
"""
pass # A simple pass statement
"#;
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;
        // The current query/logic skips top-level docstrings and pass, leaving no chunks
        assert!(chunks.is_empty(), "Expected no chunks for comments, docstrings, and pass");
        Ok(())
    }

    #[test]
    fn test_skip_top_level_pass_py() -> Result<()> {
        let code = "pass";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;
        assert!(chunks.is_empty(), "Expected top-level pass to be skipped");
        Ok(())
    }

    #[test]
    fn test_skip_top_level_docstring_py() -> Result<()> {
        let code = "\"\"\"Module docstring\"\"\"";
        let mut parser = create_parser();
        let chunks = parser.parse(code, "test.py")?;
        assert!(chunks.is_empty(), "Expected top-level docstring to be skipped");
        Ok(())
    }
} 