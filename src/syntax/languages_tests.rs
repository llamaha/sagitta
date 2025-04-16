#[cfg(test)]
mod tests {
    use super::super::languages::SUPPORTED_LANGUAGES;

    #[test]
    fn test_supported_languages_contains_expected_extensions() {
        // Check that the key languages are supported
        assert!(SUPPORTED_LANGUAGES.contains("rs"));
        assert!(SUPPORTED_LANGUAGES.contains("md"));
        assert!(SUPPORTED_LANGUAGES.contains("go"));
        assert!(SUPPORTED_LANGUAGES.contains("js"));
        assert!(SUPPORTED_LANGUAGES.contains("jsx"));
        assert!(SUPPORTED_LANGUAGES.contains("ts"));
        assert!(SUPPORTED_LANGUAGES.contains("tsx"));
        assert!(SUPPORTED_LANGUAGES.contains("yaml"));
        assert!(SUPPORTED_LANGUAGES.contains("yml"));
        assert!(SUPPORTED_LANGUAGES.contains("rb"));
        assert!(SUPPORTED_LANGUAGES.contains("py"));
    }

    #[test]
    fn test_supported_languages_does_not_contain_unknown_extensions() {
        // Ensure some common unsupported extensions are not included
        assert!(!SUPPORTED_LANGUAGES.contains("c"));
        assert!(!SUPPORTED_LANGUAGES.contains("cpp"));
        assert!(!SUPPORTED_LANGUAGES.contains("java"));
        assert!(!SUPPORTED_LANGUAGES.contains("txt"));
        assert!(!SUPPORTED_LANGUAGES.contains("html"));
    }
    
    #[test]
    fn test_supported_languages_expected_size() {
        // This test ensures that if languages are added or removed,
        // the test is updated accordingly
        assert_eq!(SUPPORTED_LANGUAGES.len(), 11);
    }
    
    #[test]
    fn test_all_extensions_are_lowercase() {
        // Ensure all extensions are stored in lowercase to ensure consistent matching
        for ext in SUPPORTED_LANGUAGES.iter() {
            assert_eq!(*ext, ext.to_lowercase());
        }
    }
} 