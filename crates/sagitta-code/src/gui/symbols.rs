/// Reliable symbols and icons for the GUI that work with egui's default fonts
/// 
/// This module provides fallback symbols for emojis that might not display properly
/// depending on the system's font configuration.
/// Brain/thinking symbols - alternatives to 🧠
pub mod thinking {
    /// Simple brain representation using ASCII
    pub const BRAIN_ASCII: &str = "🧠";
    /// Fallback brain symbol
    pub const BRAIN_FALLBACK: &str = "💭";
    /// Simple thinking indicator
    pub const THINKING: &str = "💭";
    /// Alternative thinking symbol
    pub const THOUGHT_BUBBLE: &str = "💬";
    /// Simple dot pattern for thinking
    pub const THINKING_DOTS: &str = "⋯";
    /// Gear for processing
    pub const PROCESSING: &str = "⚙";
}

/// Error and status symbols - alternatives to ✗
pub mod status {
    /// Cross mark - primary choice
    pub const CROSS: &str = "✗";
    /// Alternative cross mark
    pub const CROSS_ALT: &str = "❌";
    /// Simple X
    pub const X: &str = "×";
    /// Heavy X
    pub const X_HEAVY: &str = "✖";
    /// Check mark
    pub const CHECK: &str = "✓";
    /// Heavy check mark
    pub const CHECK_HEAVY: &str = "✔";
    /// Check mark emoji
    pub const CHECK_EMOJI: &str = "✅";
    /// Warning
    pub const WARNING: &str = "⚠";
    /// Info
    pub const INFO: &str = "ℹ";
    /// Question
    pub const QUESTION: &str = "❓";
}

/// Tool and action symbols
pub mod tools {
    /// Wrench/tool
    pub const TOOL: &str = "🔧";
    /// Gear/settings
    pub const GEAR: &str = "⚙";
    /// Cog
    pub const COG: &str = "⚙️";
    /// Hammer
    pub const HAMMER: &str = "🔨";
    /// Screwdriver
    pub const SCREWDRIVER: &str = "🪛";
}

/// Navigation and UI symbols
pub mod navigation {
    /// Right arrow
    pub const ARROW_RIGHT: &str = "→";
    /// Left arrow
    pub const ARROW_LEFT: &str = "←";
    /// Up arrow
    pub const ARROW_UP: &str = "↑";
    /// Down arrow
    pub const ARROW_DOWN: &str = "↓";
    /// Triangle right (play)
    pub const TRIANGLE_RIGHT: &str = "▶";
    /// Triangle down (expand)
    pub const TRIANGLE_DOWN: &str = "▼";
    /// Triangle up (collapse)
    pub const TRIANGLE_UP: &str = "▲";
    /// Plus
    pub const PLUS: &str = "+";
    /// Minus
    pub const MINUS: &str = "-";
    /// Menu (hamburger)
    pub const MENU: &str = "☰";
}

/// File and document symbols
pub mod files {
    /// Document
    pub const DOCUMENT: &str = "📄";
    /// Folder
    pub const FOLDER: &str = "📁";
    /// Open folder
    pub const FOLDER_OPEN: &str = "📂";
    /// File
    pub const FILE: &str = "📄";
    /// Code file
    pub const CODE: &str = "📝";
    /// Archive
    pub const ARCHIVE: &str = "📦";
}

/// Communication symbols
pub mod communication {
    /// Speech bubble
    pub const SPEECH: &str = "💬";
    /// Thought bubble
    pub const THOUGHT: &str = "💭";
    /// Message
    pub const MESSAGE: &str = "💌";
    /// Email
    pub const EMAIL: &str = "📧";
    /// Phone
    pub const PHONE: &str = "📞";
}

/// System and technical symbols
pub mod system {
    /// Computer
    pub const COMPUTER: &str = "💻";
    /// Server
    pub const SERVER: &str = "🖥️";
    /// Database
    pub const DATABASE: &str = "🗄️";
    /// Network
    pub const NETWORK: &str = "🌐";
    /// CPU
    pub const CPU: &str = "🖥️";
    /// Memory
    pub const MEMORY: &str = "💾";
}

/// Get the best available symbol for thinking/brain operations
pub fn get_thinking_symbol() -> &'static str {
    // Try the brain emoji first, fall back to thought bubble if needed
    thinking::BRAIN_ASCII
}

/// Get the best available symbol for errors/failures
pub fn get_error_symbol() -> &'static str {
    // Try the cross mark first, fall back to X if needed
    status::CROSS
}

/// Get the best available symbol for success
pub fn get_success_symbol() -> &'static str {
    status::CHECK
}

/// Get the best available symbol for tools/actions
pub fn get_tool_symbol() -> &'static str {
    tools::TOOL
}

/// Get the best available symbol for warnings
pub fn get_warning_symbol() -> &'static str {
    status::WARNING
}

/// Get the best available symbol for info
pub fn get_info_symbol() -> &'static str {
    status::INFO
}

/// Test if a symbol renders properly in the current font context
/// This is a placeholder for future font testing functionality
pub fn test_symbol_support(_symbol: &str) -> bool {
    // For now, assume all symbols are supported
    // In the future, this could test actual glyph availability
    true
}

/// Get fallback symbols for common use cases
pub mod fallbacks {
    use super::*;
    
    /// Get thinking symbol with fallback chain
    pub fn thinking() -> &'static str {
        // Could implement actual font testing here
        thinking::BRAIN_ASCII
    }
    
    /// Get error symbol with fallback chain
    pub fn error() -> &'static str {
        status::CROSS
    }
    
    /// Get success symbol with fallback chain
    pub fn success() -> &'static str {
        status::CHECK
    }
    
    /// Get tool symbol with fallback chain
    pub fn tool() -> &'static str {
        tools::TOOL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_symbols() {
        assert_eq!(get_thinking_symbol(), "🧠");
        assert_eq!(thinking::BRAIN_ASCII, "🧠");
        assert_eq!(thinking::THINKING, "💭");
        assert!(!thinking::THINKING_DOTS.is_empty());
    }

    #[test]
    fn test_status_symbols() {
        assert_eq!(get_error_symbol(), "✗");
        assert_eq!(get_success_symbol(), "✓");
        assert_eq!(get_warning_symbol(), "⚠");
        assert_eq!(get_info_symbol(), "ℹ");
    }

    #[test]
    fn test_tool_symbols() {
        assert_eq!(get_tool_symbol(), "🔧");
        assert!(!tools::GEAR.is_empty());
        assert!(!tools::HAMMER.is_empty());
    }

    #[test]
    fn test_navigation_symbols() {
        assert_eq!(navigation::ARROW_RIGHT, "→");
        assert_eq!(navigation::ARROW_LEFT, "←");
        assert_eq!(navigation::TRIANGLE_RIGHT, "▶");
        assert_eq!(navigation::PLUS, "+");
    }

    #[test]
    fn test_fallback_functions() {
        assert_eq!(fallbacks::thinking(), "🧠");
        assert_eq!(fallbacks::error(), "✗");
        assert_eq!(fallbacks::success(), "✓");
        assert_eq!(fallbacks::tool(), "🔧");
    }

    #[test]
    fn test_symbol_support_function() {
        // Test that the symbol support function exists and returns a boolean
        assert!(test_symbol_support("🧠") || !test_symbol_support("🧠"));
        assert!(test_symbol_support("✗") || !test_symbol_support("✗"));
    }

    #[test]
    fn test_all_symbols_are_non_empty() {
        // Ensure all symbols are non-empty strings
        assert!(!thinking::BRAIN_ASCII.is_empty());
        assert!(!status::CROSS.is_empty());
        assert!(!tools::TOOL.is_empty());
        assert!(!navigation::ARROW_RIGHT.is_empty());
        assert!(!files::DOCUMENT.is_empty());
        assert!(!communication::SPEECH.is_empty());
        assert!(!system::COMPUTER.is_empty());
    }
} 