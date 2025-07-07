use crate::html::HtmlParser;
use crate::parser::SyntaxParser;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_html() {
        let mut parser = HtmlParser::new();
        let result = parser.parse("", "test.html");
        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        // Fallback parser creates one empty chunk for empty files
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "");
        assert_eq!(chunks[0].language, "html");
        assert_eq!(chunks[0].element_type, "fallback_chunk");
    }

    #[test]
    fn test_parse_simple_html() {
        let mut parser = HtmlParser::new();
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Test Page</title>
</head>
<body>
    <h1>Hello World</h1>
    <p>This is a test.</p>
</body>
</html>"#;
        
        let result = parser.parse(html, "test.html");
        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        // Since it uses fallback parser, it creates chunks based on lines
        assert!(!chunks.is_empty());
        
        // Verify all chunks are marked as HTML
        for chunk in &chunks {
            assert_eq!(chunk.language, "html");
            assert!(chunk.element_type.starts_with("fallback_chunk_"));
        }
    }

    #[test]
    fn test_parse_html_with_script_and_style() {
        let mut parser = HtmlParser::new();
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Test</title>
    <style>
        body { margin: 0; }
        h1 { color: blue; }
    </style>
    <script>
        function greet() {
            console.log("Hello!");
        }
    </script>
</head>
<body>
    <h1 onclick="greet()">Click me</h1>
</body>
</html>"#;
        
        let result = parser.parse(html, "complex.html");
        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        // Verify chunks exist and are HTML
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert_eq!(chunk.language, "html");
        }
    }

    #[test]
    fn test_parse_malformed_html() {
        let mut parser = HtmlParser::new();
        // Malformed HTML should still parse (fallback doesn't validate)
        let html = r#"<div>
    <p>Unclosed paragraph
    <span>Unclosed span
</div>
<script>
    // Missing closing script tag"#;
        
        let result = parser.parse(html, "malformed.html");
        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        // Should still create chunks
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert_eq!(chunk.language, "html");
        }
    }

    #[test]
    fn test_parse_html_with_comments() {
        let mut parser = HtmlParser::new();
        let html = r#"<!-- Header comment -->
<html>
<head>
    <!-- Meta tags here -->
    <meta charset="UTF-8">
</head>
<body>
    <!-- Main content -->
    <div>Content</div>
    <!-- Footer will go here -->
</body>
</html>"#;
        
        let result = parser.parse(html, "commented.html");
        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert_eq!(chunk.language, "html");
        }
    }

    #[test]
    fn test_html_parser_default() {
        // Both should create parsers with fallback parser
        // We can't directly compare them, but we can verify they work the same
        
        let mut p1 = HtmlParser::new();
        let mut p2 = HtmlParser::default();
        
        let html = "<p>Test</p>";
        let r1 = p1.parse(html, "test.html");
        let r2 = p2.parse(html, "test.html");
        
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert_eq!(r1.unwrap().len(), r2.unwrap().len());
    }

    #[test]
    fn test_parse_single_line_html() {
        let mut parser = HtmlParser::new();
        let html = "<html><head><title>Test</title></head><body><p>Hello</p></body></html>";
        
        let result = parser.parse(html, "oneline.html");
        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        // Single line should create one chunk with 0-based index
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].language, "html");
        assert_eq!(chunks[0].element_type, "fallback_chunk_0");
    }

    #[test]
    fn test_parse_html_with_special_characters() {
        let mut parser = HtmlParser::new();
        let html = r#"<html>
<body>
    <p>&lt;Special&gt; &amp; "Characters"</p>
    <div data-value='single quotes'>Test</div>
</body>
</html>"#;
        
        let result = parser.parse(html, "special.html");
        assert!(result.is_ok());
        let chunks = result.unwrap();
        
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert_eq!(chunk.language, "html");
        }
    }
}