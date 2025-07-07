#[cfg(test)]
mod tests {
    use super::super::scan_line;
    use crate::types::MethodType;

    #[test]
    fn test_scan_level_1_header() {
        let mut methods = Vec::new();
        scan_line(
            "# Introduction",
            "# Introduction\n\nThis is the introduction section.",
            None,
            &mut methods,
            1,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Introduction");
        assert_eq!(methods[0].method_type, MethodType::MarkdownHeader);
        assert_eq!(methods[0].params, "level 1");
        assert_eq!(methods[0].line_number, Some(1));
    }

    #[test]
    fn test_scan_level_2_header() {
        let mut methods = Vec::new();
        scan_line(
            "## Getting Started",
            "## Getting Started\n\nFollow these steps...",
            Some("Setup instructions".to_string()),
            &mut methods,
            5,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Getting Started");
        assert_eq!(methods[0].method_type, MethodType::MarkdownHeader);
        assert_eq!(methods[0].params, "level 2");
        assert_eq!(methods[0].docstring, Some("Setup instructions".to_string()));
    }

    #[test]
    fn test_scan_level_3_header() {
        let mut methods = Vec::new();
        scan_line(
            "### Installation",
            "### Installation\n\n```bash\nnpm install\n```",
            None,
            &mut methods,
            10,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Installation");
        assert_eq!(methods[0].params, "level 3");
    }

    #[test]
    fn test_scan_level_4_header() {
        let mut methods = Vec::new();
        scan_line(
            "#### Prerequisites",
            "#### Prerequisites",
            None,
            &mut methods,
            15,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Prerequisites");
        assert_eq!(methods[0].params, "level 4");
    }

    #[test]
    fn test_scan_level_5_header() {
        let mut methods = Vec::new();
        scan_line(
            "##### Note",
            "##### Note\n\nImportant information here.",
            None,
            &mut methods,
            20,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Note");
        assert_eq!(methods[0].params, "level 5");
    }

    #[test]
    fn test_scan_level_6_header() {
        let mut methods = Vec::new();
        scan_line(
            "###### Copyright",
            "###### Copyright",
            None,
            &mut methods,
            25,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Copyright");
        assert_eq!(methods[0].params, "level 6");
    }

    #[test]
    fn test_scan_header_with_trailing_whitespace() {
        let mut methods = Vec::new();
        scan_line(
            "## Configuration   ",
            "## Configuration   \n\nConfigure your app...",
            None,
            &mut methods,
            30,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Configuration");
        assert_eq!(methods[0].params, "level 2");
    }

    #[test]
    fn test_scan_header_with_special_characters() {
        let mut methods = Vec::new();
        scan_line(
            "### API Reference (v2.0)",
            "### API Reference (v2.0)",
            None,
            &mut methods,
            35,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "API Reference (v2.0)");
        assert_eq!(methods[0].params, "level 3");
    }

    #[test]
    fn test_scan_header_with_code_inline() {
        let mut methods = Vec::new();
        scan_line(
            "## Using `config.json`",
            "## Using `config.json`",
            None,
            &mut methods,
            40,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Using `config.json`");
        assert_eq!(methods[0].params, "level 2");
    }

    #[test]
    fn test_scan_header_with_emojis() {
        let mut methods = Vec::new();
        scan_line(
            "# 🚀 Quick Start",
            "# 🚀 Quick Start",
            None,
            &mut methods,
            45,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "🚀 Quick Start");
        assert_eq!(methods[0].params, "level 1");
    }

    #[test]
    fn test_scan_non_header_lines() {
        let mut methods = Vec::new();
        
        // Regular text
        scan_line(
            "This is regular text",
            "This is regular text",
            None,
            &mut methods,
            50,
            10,
        );
        assert_eq!(methods.len(), 0);
        
        // Code block
        scan_line(
            "```javascript",
            "```javascript\nconst x = 1;\n```",
            None,
            &mut methods,
            51,
            10,
        );
        assert_eq!(methods.len(), 0);
        
        // List item
        scan_line(
            "- Item 1",
            "- Item 1\n- Item 2",
            None,
            &mut methods,
            52,
            10,
        );
        assert_eq!(methods.len(), 0);
        
        // Hash not at start of line
        scan_line(
            "Use the # symbol for headers",
            "Use the # symbol for headers",
            None,
            &mut methods,
            53,
            10,
        );
        assert_eq!(methods.len(), 0);
    }

    #[test]
    fn test_scan_atx_header_closing_hashes() {
        let mut methods = Vec::new();
        scan_line(
            "## Section 2 ##",
            "## Section 2 ##",
            None,
            &mut methods,
            60,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Section 2 ##");
        assert_eq!(methods[0].params, "level 2");
    }

    #[test]
    fn test_scan_header_empty_title() {
        let mut methods = Vec::new();
        scan_line(
            "### ",
            "### ",
            None,
            &mut methods,
            65,
            10,
        );
        
        // Should not match because there's no title after the hashes
        assert_eq!(methods.len(), 0);
    }

    #[test]
    fn test_max_calls_parameter_ignored() {
        let mut methods = Vec::new();
        // max_calls parameter should be ignored for markdown
        scan_line(
            "# Test Header",
            "# Test Header",
            None,
            &mut methods,
            70,
            0, // max_calls = 0, but should still work
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "Test Header");
    }

    #[test]
    fn test_context_preservation() {
        let mut methods = Vec::new();
        let context = "# Main Title\n\n## Subsection\n\nSome content here.";
        
        scan_line(
            "## Subsection",
            context,
            None,
            &mut methods,
            3,
            10,
        );
        
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].context, context);
    }
}