/// Font configuration and emoji support for the GUI
/// 
/// This module provides utilities for configuring fonts to ensure better
/// emoji and symbol support in egui applications.

use egui::{FontDefinitions, FontFamily, FontData};
use std::sync::Arc;

/// Configure fonts for better emoji support
pub fn configure_fonts_with_emoji_support() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    
    // The default egui fonts already include emoji support via:
    // - NotoEmoji-Regular for good emoji coverage
    // - emoji-icon-font for additional symbols
    // 
    // We can enhance this by ensuring emoji fonts are prioritized
    // in the font fallback chain
    
    // For proportional text, ensure emoji fonts come after the main font
    // but before other fallbacks
    if let Some(proportional_fonts) = fonts.families.get_mut(&FontFamily::Proportional) {
        // Move emoji fonts to higher priority if they exist
        let mut emoji_fonts = Vec::new();
        let mut other_fonts = Vec::new();
        
        for font_name in proportional_fonts.iter() {
            if font_name.contains("Emoji") || font_name.contains("emoji") {
                emoji_fonts.push(font_name.clone());
            } else {
                other_fonts.push(font_name.clone());
            }
        }
        
        // Rebuild the list with main font first, then emoji fonts, then others
        let mut new_order = Vec::new();
        if let Some(main_font) = other_fonts.first() {
            new_order.push(main_font.clone());
        }
        new_order.extend(emoji_fonts);
        new_order.extend(other_fonts.into_iter().skip(1));
        
        *proportional_fonts = new_order;
    }
    
    // Do the same for monospace fonts
    if let Some(monospace_fonts) = fonts.families.get_mut(&FontFamily::Monospace) {
        let mut emoji_fonts = Vec::new();
        let mut other_fonts = Vec::new();
        
        for font_name in monospace_fonts.iter() {
            if font_name.contains("Emoji") || font_name.contains("emoji") {
                emoji_fonts.push(font_name.clone());
            } else {
                other_fonts.push(font_name.clone());
            }
        }
        
        let mut new_order = Vec::new();
        if let Some(main_font) = other_fonts.first() {
            new_order.push(main_font.clone());
        }
        new_order.extend(emoji_fonts);
        new_order.extend(other_fonts.into_iter().skip(1));
        
        *monospace_fonts = new_order;
    }
    
    fonts
}

/// Apply font configuration to an egui context
pub fn apply_font_config(ctx: &egui::Context) {
    let fonts = configure_fonts_with_emoji_support();
    ctx.set_fonts(fonts);
}

/// Test if a specific character is supported by the current font configuration
pub fn test_character_support(ctx: &egui::Context, character: char) -> bool {
    // This is a simplified test - we'll check if the character can be measured
    // which indicates it has a glyph in the font
    ctx.fonts(|f| {
        let font_id = egui::FontId::default();
        // Use layout_no_wrap to test if the character can be rendered
        let galley = f.layout_no_wrap(character.to_string(), font_id, egui::Color32::WHITE);
        !galley.rows.is_empty() && galley.size().x > 0.0
    })
}

/// Get a list of problematic characters that might not render properly
pub fn get_problematic_characters() -> Vec<char> {
    vec![
        '🧠', // Brain emoji - might not be supported
        '✗',  // Cross mark - might not be supported
        '🔧', // Wrench - might not be supported
        '💭', // Thought bubble - usually supported
        '⚙',  // Gear - usually supported
        '✓',  // Check mark - usually supported
    ]
}

/// Test all problematic characters and return a report
pub fn test_emoji_support(ctx: &egui::Context) -> EmojiSupportReport {
    let problematic_chars = get_problematic_characters();
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();
    
    for &character in &problematic_chars {
        if test_character_support(ctx, character) {
            supported.push(character);
        } else {
            unsupported.push(character);
        }
    }
    
    EmojiSupportReport {
        supported,
        unsupported,
        total_tested: problematic_chars.len(),
    }
}

/// Report on emoji support status
#[derive(Debug, Clone)]
pub struct EmojiSupportReport {
    pub supported: Vec<char>,
    pub unsupported: Vec<char>,
    pub total_tested: usize,
}

impl EmojiSupportReport {
    /// Get the percentage of supported emojis
    pub fn support_percentage(&self) -> f32 {
        if self.total_tested == 0 {
            return 100.0;
        }
        (self.supported.len() as f32 / self.total_tested as f32) * 100.0
    }
    
    /// Check if all tested emojis are supported
    pub fn all_supported(&self) -> bool {
        self.unsupported.is_empty()
    }
    
    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "Emoji support: {}/{} ({:.1}%) - Supported: {:?}, Unsupported: {:?}",
            self.supported.len(),
            self.total_tested,
            self.support_percentage(),
            self.supported,
            self.unsupported
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_configuration() {
        let fonts = configure_fonts_with_emoji_support();
        
        // Ensure we have font families configured
        assert!(fonts.families.contains_key(&FontFamily::Proportional));
        assert!(fonts.families.contains_key(&FontFamily::Monospace));
        
        // Ensure we have some fonts configured
        assert!(!fonts.families[&FontFamily::Proportional].is_empty());
        assert!(!fonts.families[&FontFamily::Monospace].is_empty());
    }

    #[test]
    fn test_problematic_characters_list() {
        let chars = get_problematic_characters();
        assert!(!chars.is_empty());
        assert!(chars.contains(&'🧠'));
        assert!(chars.contains(&'✗'));
    }

    #[test]
    fn test_emoji_support_report() {
        let report = EmojiSupportReport {
            supported: vec!['✓', '⚙'],
            unsupported: vec!['🧠', '✗'],
            total_tested: 4,
        };
        
        assert_eq!(report.support_percentage(), 50.0);
        assert!(!report.all_supported());
        assert!(report.summary().contains("50.0%"));
    }

    #[test]
    fn test_empty_report() {
        let report = EmojiSupportReport {
            supported: vec![],
            unsupported: vec![],
            total_tested: 0,
        };
        
        assert_eq!(report.support_percentage(), 100.0);
        assert!(report.all_supported());
    }
} 