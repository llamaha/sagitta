use egui::{Color32, Visuals, Style};
use serde::{Serialize, Deserialize, Serializer, Deserializer};

/// Custom serialization for Color32
mod color32_serde {
    use super::*;
    
    #[derive(Serialize, Deserialize)]
    struct Color32Rgba {
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    }
    
    pub fn serialize<S>(color: &Color32, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let rgba = Color32Rgba {
            r: color.r(),
            g: color.g(),
            b: color.b(),
            a: color.a(),
        };
        rgba.serialize(serializer)
    }
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Color32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let rgba = Color32Rgba::deserialize(deserializer)?;
        Ok(Color32::from_rgba_unmultiplied(rgba.r, rgba.g, rgba.b, rgba.a))
    }
}

/// Theme options for the application - includes custom theme support
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTheme {
    Dark,
    Light,
    Custom,
}

/// Customizable theme colors for all UI components
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomThemeColors {
    // Background colors
    #[serde(with = "color32_serde")]
    pub panel_background: Color32,
    #[serde(with = "color32_serde")]
    pub input_background: Color32,
    #[serde(with = "color32_serde")]
    pub button_background: Color32,
    #[serde(with = "color32_serde")]
    pub code_background: Color32,
    #[serde(with = "color32_serde")]
    pub thinking_background: Color32,
    
    // Text colors
    #[serde(with = "color32_serde")]
    pub text_color: Color32,
    #[serde(with = "color32_serde")]
    pub hint_text_color: Color32,
    #[serde(with = "color32_serde")]
    pub code_text_color: Color32,
    #[serde(with = "color32_serde")]
    pub thinking_text_color: Color32,
    #[serde(with = "color32_serde")]
    pub timestamp_color: Color32,
    
    // Accent and highlight colors
    #[serde(with = "color32_serde")]
    pub accent_color: Color32,
    #[serde(with = "color32_serde")]
    pub success_color: Color32,
    #[serde(with = "color32_serde")]
    pub warning_color: Color32,
    #[serde(with = "color32_serde")]
    pub error_color: Color32,
    
    // Border and stroke colors
    #[serde(with = "color32_serde")]
    pub border_color: Color32,
    #[serde(with = "color32_serde")]
    pub focus_border_color: Color32,
    
    // Button states
    #[serde(with = "color32_serde")]
    pub button_hover_color: Color32,
    #[serde(with = "color32_serde")]
    pub button_disabled_color: Color32,
    #[serde(with = "color32_serde")]
    pub button_text_color: Color32,
    #[serde(with = "color32_serde")]
    pub button_disabled_text_color: Color32,
    
    // Font sizes
    pub base_font_size: f32,
    pub header_font_size: f32,
    pub code_font_size: f32,
    pub small_font_size: f32,
    
    // Author colors
    #[serde(with = "color32_serde")]
    pub user_color: Color32,
    #[serde(with = "color32_serde")]
    pub agent_color: Color32,
    #[serde(with = "color32_serde")]
    pub system_color: Color32,
    #[serde(with = "color32_serde")]
    pub tool_color: Color32,
    
    // Status indicators
    #[serde(with = "color32_serde")]
    pub streaming_color: Color32,
    #[serde(with = "color32_serde")]
    pub thinking_indicator_color: Color32,
    #[serde(with = "color32_serde")]
    pub complete_color: Color32,
    
    // Diff colors
    #[serde(with = "color32_serde")]
    pub diff_added_bg: Color32,
    #[serde(with = "color32_serde")]
    pub diff_removed_bg: Color32,
    #[serde(with = "color32_serde")]
    pub diff_added_text: Color32,
    #[serde(with = "color32_serde")]
    pub diff_removed_text: Color32,
}

impl Default for CustomThemeColors {
    fn default() -> Self {
        // Default to dark theme colors
        Self {
            // Background colors
            panel_background: Color32::from_rgb(27, 27, 27),
            input_background: Color32::from_rgb(40, 40, 40),
            button_background: Color32::from_rgb(60, 60, 60),
            code_background: Color32::from_rgb(35, 35, 35),
            thinking_background: Color32::from_rgb(45, 45, 45),
            
            // Text colors
            text_color: Color32::from_rgb(220, 220, 220),
            hint_text_color: Color32::from_rgb(128, 128, 128),
            code_text_color: Color32::from_rgb(200, 200, 200),
            thinking_text_color: Color32::from_rgb(180, 180, 180),
            timestamp_color: Color32::from_rgb(128, 128, 128),
            
            // Accent and highlight colors
            accent_color: Color32::from_rgb(100, 149, 237),
            success_color: Color32::from_rgb(50, 205, 50),
            warning_color: Color32::from_rgb(255, 215, 0),
            error_color: Color32::from_rgb(255, 69, 0),
            
            // Border and stroke colors
            border_color: Color32::from_rgb(60, 60, 60),
            focus_border_color: Color32::from_rgb(100, 149, 237),
            
            // Button states
            button_hover_color: Color32::from_rgb(80, 80, 80),
            button_disabled_color: Color32::from_rgb(60, 60, 60),
            button_text_color: Color32::WHITE,
            button_disabled_text_color: Color32::from_rgb(180, 180, 180),
            
            // Font sizes
            base_font_size: 14.0,
            header_font_size: 16.0,
            code_font_size: 13.0,
            small_font_size: 11.0,
            
            // Author colors
            user_color: Color32::from_rgb(255, 255, 255),
            agent_color: Color32::from_rgb(0, 255, 0),
            system_color: Color32::from_rgb(255, 0, 0),
            tool_color: Color32::from_rgb(255, 255, 0),
            
            // Status indicators
            streaming_color: Color32::from_rgb(150, 255, 150),
            thinking_indicator_color: Color32::from_rgb(100, 150, 255),
            complete_color: Color32::from_rgb(100, 255, 100),
            
            // Diff colors
            diff_added_bg: Color32::from_rgb(0, 80, 0),     // Dark green background
            diff_removed_bg: Color32::from_rgb(80, 0, 0),   // Dark red background
            diff_added_text: Color32::from_rgb(100, 200, 100), // Light green text
            diff_removed_text: Color32::from_rgb(200, 100, 100), // Light red text
        }
    }
}

// Global custom theme colors - will be used when AppTheme::Custom is selected
static mut CUSTOM_THEME_COLORS: Option<CustomThemeColors> = None;

/// Get the current custom theme colors
pub fn get_custom_theme_colors() -> CustomThemeColors {
    unsafe {
        CUSTOM_THEME_COLORS.clone().unwrap_or_default()
    }
}

/// Set the custom theme colors
pub fn set_custom_theme_colors(colors: CustomThemeColors) {
    unsafe {
        CUSTOM_THEME_COLORS = Some(colors);
    }
}

impl Default for AppTheme {
    fn default() -> Self {
        AppTheme::Dark
    }
}

impl AppTheme {
    /// Convert to egui Visuals
    pub fn to_egui_visuals(self) -> Visuals {
        match self {
            AppTheme::Dark => Visuals::dark(),
            AppTheme::Light => Visuals::light(),
            AppTheme::Custom => Visuals::dark(), // Custom theme is treated as dark
        }
    }

    /// Convert to egui Style with proper visuals
    pub fn to_egui_style(self) -> Style {
        let mut style = Style::default();
        style.visuals = self.to_egui_visuals();
        style
    }

    /// Get all available themes
    pub fn all() -> impl Iterator<Item = Self> {
        [AppTheme::Dark, AppTheme::Light, AppTheme::Custom].iter().copied()
    }

    /// Get human-readable theme name
    pub fn name(&self) -> &'static str {
        match self {
            AppTheme::Dark => "Dark",
            AppTheme::Light => "Light",
            AppTheme::Custom => "Custom",
        }
    }

    /// Check if theme is light
    pub fn is_light(&self) -> bool {
        matches!(self, AppTheme::Light)
    }

    /// Check if theme is dark
    pub fn is_dark(&self) -> bool {
        matches!(self, AppTheme::Dark)
    }

    /// Apply the theme to the egui context
    pub fn apply_to_context(&self, ctx: &egui::Context) {
        ctx.set_visuals(self.to_egui_visuals());
        
        // Apply font sizes
        let mut style = (*ctx.style()).clone();
        
        // Get font sizes based on theme
        let (base_size, header_size, code_size, small_size) = match self {
            AppTheme::Custom => {
                let custom = get_custom_theme_colors();
                (custom.base_font_size, custom.header_font_size, custom.code_font_size, custom.small_font_size)
            },
            _ => (14.0, 16.0, 13.0, 11.0), // Default sizes for built-in themes
        };
        
        // Apply font sizes to text styles
        use egui::{TextStyle, FontId, FontFamily};
        style.text_styles = [
            (TextStyle::Small, FontId::new(small_size, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(base_size, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(base_size, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(header_size, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(code_size, FontFamily::Monospace)),
        ].iter().cloned().collect();
        
        ctx.set_style(style);
    }

    /// Get background color for panels
    pub fn panel_background(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(27, 27, 27),
            AppTheme::Light => Color32::from_rgb(248, 248, 248),
            AppTheme::Custom => get_custom_theme_colors().panel_background,
        }
    }

    /// Get text color
    pub fn text_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(220, 220, 220),
            AppTheme::Light => Color32::from_rgb(60, 60, 60),
            AppTheme::Custom => get_custom_theme_colors().text_color,
        }
    }

    /// Get accent color for highlights
    pub fn accent_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(100, 149, 237),  // Cornflower blue
            AppTheme::Light => Color32::from_rgb(70, 130, 180),  // Steel blue
            AppTheme::Custom => get_custom_theme_colors().accent_color,
        }
    }

    /// Get input background color
    pub fn input_background(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(40, 40, 40),
            AppTheme::Light => Color32::from_rgb(255, 255, 255),
            AppTheme::Custom => get_custom_theme_colors().input_background,
        }
    }

    /// Get border color
    pub fn border_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(60, 60, 60),
            AppTheme::Light => Color32::from_rgb(200, 200, 200),
            AppTheme::Custom => get_custom_theme_colors().border_color,
        }
    }

    /// Get button background color
    pub fn button_background(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(60, 60, 60),
            AppTheme::Light => Color32::from_rgb(240, 240, 240),
            AppTheme::Custom => get_custom_theme_colors().button_background,
        }
    }

    /// Get code background color
    pub fn code_background(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(35, 35, 35),
            AppTheme::Light => Color32::from_rgb(250, 250, 250),
            AppTheme::Custom => get_custom_theme_colors().code_background,
        }
    }

    /// Get thinking background color
    pub fn thinking_background(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(45, 45, 45),
            AppTheme::Light => Color32::from_rgb(245, 245, 245),
            AppTheme::Custom => get_custom_theme_colors().thinking_background,
        }
    }

    /// Get hint text color
    pub fn hint_text_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(128, 128, 128),
            AppTheme::Light => Color32::from_rgb(128, 128, 128),
            AppTheme::Custom => get_custom_theme_colors().hint_text_color,
        }
    }

    /// Get code text color
    pub fn code_text_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(200, 200, 200),
            AppTheme::Light => Color32::from_rgb(40, 40, 40),
            AppTheme::Custom => get_custom_theme_colors().code_text_color,
        }
    }

    /// Get thinking text color
    pub fn thinking_text_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(180, 180, 180),
            AppTheme::Light => Color32::from_rgb(80, 80, 80),
            AppTheme::Custom => get_custom_theme_colors().thinking_text_color,
        }
    }

    /// Get timestamp color
    pub fn timestamp_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(128, 128, 128),
            AppTheme::Light => Color32::from_rgb(128, 128, 128),
            AppTheme::Custom => get_custom_theme_colors().timestamp_color,
        }
    }

    /// Get success color
    pub fn success_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(50, 205, 50),
            AppTheme::Light => Color32::from_rgb(34, 139, 34),
            AppTheme::Custom => get_custom_theme_colors().success_color,
        }
    }

    /// Get warning color
    pub fn warning_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(255, 215, 0),
            AppTheme::Light => Color32::from_rgb(255, 140, 0),
            AppTheme::Custom => get_custom_theme_colors().warning_color,
        }
    }

    /// Get error color
    pub fn error_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(255, 69, 0),
            AppTheme::Light => Color32::from_rgb(220, 20, 60),
            AppTheme::Custom => get_custom_theme_colors().error_color,
        }
    }

    /// Get focus border color
    pub fn focus_border_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(100, 149, 237),
            AppTheme::Light => Color32::from_rgb(70, 130, 180),
            AppTheme::Custom => get_custom_theme_colors().focus_border_color,
        }
    }

    /// Get button hover color
    pub fn button_hover_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(80, 80, 80),
            AppTheme::Light => Color32::from_rgb(230, 230, 230),
            AppTheme::Custom => get_custom_theme_colors().button_hover_color,
        }
    }

    /// Get button disabled color
    pub fn button_disabled_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(60, 60, 60),
            AppTheme::Light => Color32::from_rgb(200, 200, 200),
            AppTheme::Custom => get_custom_theme_colors().button_disabled_color,
        }
    }

    /// Get button text color
    pub fn button_text_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::WHITE,
            AppTheme::Light => Color32::from_rgb(60, 60, 60),
            AppTheme::Custom => get_custom_theme_colors().button_text_color,
        }
    }

    /// Get button disabled text color
    pub fn button_disabled_text_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(180, 180, 180),
            AppTheme::Light => Color32::from_rgb(120, 120, 120),
            AppTheme::Custom => get_custom_theme_colors().button_disabled_text_color,
        }
    }

    /// Get user color
    pub fn user_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(255, 255, 255),
            AppTheme::Light => Color32::from_rgb(60, 60, 60),
            AppTheme::Custom => get_custom_theme_colors().user_color,
        }
    }

    /// Get agent color
    pub fn agent_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(0, 255, 0),
            AppTheme::Light => Color32::from_rgb(34, 139, 34),
            AppTheme::Custom => get_custom_theme_colors().agent_color,
        }
    }

    /// Get system color
    pub fn system_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(255, 0, 0),
            AppTheme::Light => Color32::from_rgb(220, 20, 60),
            AppTheme::Custom => get_custom_theme_colors().system_color,
        }
    }

    /// Get tool color
    pub fn tool_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(255, 255, 0),
            AppTheme::Light => Color32::from_rgb(255, 140, 0),
            AppTheme::Custom => get_custom_theme_colors().tool_color,
        }
    }

    /// Get streaming color
    pub fn streaming_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(150, 255, 150),
            AppTheme::Light => Color32::from_rgb(34, 139, 34),
            AppTheme::Custom => get_custom_theme_colors().streaming_color,
        }
    }

    /// Get thinking indicator color
    pub fn thinking_indicator_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(100, 150, 255),
            AppTheme::Light => Color32::from_rgb(70, 130, 180),
            AppTheme::Custom => get_custom_theme_colors().thinking_indicator_color,
        }
    }

    /// Get complete color
    pub fn complete_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(100, 255, 100),
            AppTheme::Light => Color32::from_rgb(34, 139, 34),
            AppTheme::Custom => get_custom_theme_colors().complete_color,
        }
    }

    /// Get diff added background color
    pub fn diff_added_bg(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(0, 80, 0),     // Dark green
            AppTheme::Light => Color32::from_rgb(200, 255, 200), // Light green
            AppTheme::Custom => get_custom_theme_colors().diff_added_bg,
        }
    }

    /// Get diff removed background color
    pub fn diff_removed_bg(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(80, 0, 0),     // Dark red
            AppTheme::Light => Color32::from_rgb(255, 200, 200), // Light red
            AppTheme::Custom => get_custom_theme_colors().diff_removed_bg,
        }
    }

    /// Get diff added text color
    pub fn diff_added_text(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(120, 255, 120),  // Light green text
            AppTheme::Light => Color32::from_rgb(0, 100, 0),     // Dark green text
            AppTheme::Custom => get_custom_theme_colors().diff_added_text,
        }
    }

    /// Get diff removed text color
    pub fn diff_removed_text(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(200, 100, 100),
            AppTheme::Light => Color32::from_rgb(150, 0, 0),
            AppTheme::Custom => get_custom_theme_colors().diff_removed_text,
        }
    }

    /// Get info background color for suggestions and notifications
    pub fn info_background(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgba_unmultiplied(70, 130, 180, 30), // Steel blue with low alpha
            AppTheme::Light => Color32::from_rgba_unmultiplied(173, 216, 230, 50), // Light blue with medium alpha
            AppTheme::Custom => Color32::from_rgba_unmultiplied(100, 149, 237, 30), // Cornflower blue with low alpha
        }
    }
    
    /// Get info text/border color for suggestions and notifications
    pub fn info_color(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(135, 206, 235), // Sky blue
            AppTheme::Light => Color32::from_rgb(70, 130, 180), // Steel blue
            AppTheme::Custom => get_custom_theme_colors().accent_color,
        }
    }

    /// Get frame for side panels with consistent inner padding
    pub fn side_panel_frame(&self) -> egui::Frame {
        egui::Frame::none()
            .fill(self.panel_background())
            .inner_margin(egui::Margin::same(8))
    }

    /// Get tool result background color
    pub fn tool_result_background(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(50, 50, 60),
            AppTheme::Light => Color32::from_rgb(245, 245, 250),
            AppTheme::Custom => get_custom_theme_colors().button_background, // Reuse button background for custom
        }
    }

    /// Get separator color
    pub fn separator_color(&self) -> Color32 {
        self.border_color()
    }

    /// Get muted text color
    pub fn muted_text_color(&self) -> Color32 {
        self.hint_text_color()
    }

    /// Get user message background color
    pub fn user_message_background(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(35, 35, 40),
            AppTheme::Light => Color32::from_rgb(240, 240, 245),
            AppTheme::Custom => get_custom_theme_colors().code_background,
        }
    }

    /// Get agent message background color
    pub fn agent_message_background(&self) -> Color32 {
        match self {
            AppTheme::Dark => Color32::from_rgb(30, 40, 30),
            AppTheme::Light => Color32::from_rgb(235, 245, 235),
            AppTheme::Custom => Color32::from_rgba_unmultiplied(
                get_custom_theme_colors().agent_color.r(),
                get_custom_theme_colors().agent_color.g(),
                get_custom_theme_colors().agent_color.b(),
                20
            ),
        }
    }
    
    /// Get base font size
    pub fn base_font_size(&self) -> f32 {
        match self {
            AppTheme::Custom => get_custom_theme_colors().base_font_size,
            _ => 14.0,
        }
    }
    
    /// Get header font size
    pub fn header_font_size(&self) -> f32 {
        match self {
            AppTheme::Custom => get_custom_theme_colors().header_font_size,
            _ => 16.0,
        }
    }
    
    /// Get code font size
    pub fn code_font_size(&self) -> f32 {
        match self {
            AppTheme::Custom => get_custom_theme_colors().code_font_size,
            _ => 13.0,
        }
    }
    
    /// Get small font size
    pub fn small_font_size(&self) -> f32 {
        match self {
            AppTheme::Custom => get_custom_theme_colors().small_font_size,
            _ => 11.0,
        }
    }
}

/// Apply theme to the entire application
pub fn apply_theme(ctx: &egui::Context, theme: AppTheme) {
    theme.apply_to_context(ctx);
}

/// Get theme from string (for config loading)
pub fn theme_from_string(s: &str) -> AppTheme {
    match s.to_lowercase().as_str() {
        "light" => AppTheme::Light,
        "custom" => AppTheme::Custom,
        "dark" | _ => AppTheme::Dark, // Default to dark
    }
}

/// Convert theme to string (for config saving)
pub fn theme_to_string(theme: AppTheme) -> String {
    match theme {
        AppTheme::Dark => "dark".to_string(),
        AppTheme::Light => "light".to_string(),
        AppTheme::Custom => "custom".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_names() {
        assert_eq!(AppTheme::Dark.name(), "Dark");
        assert_eq!(AppTheme::Light.name(), "Light");
        assert_eq!(AppTheme::Custom.name(), "Custom");
    }

    #[test]
    fn test_theme_properties() {
        assert!(AppTheme::Light.is_light());
        assert!(!AppTheme::Light.is_dark());
        
        assert!(AppTheme::Dark.is_dark());
        assert!(!AppTheme::Dark.is_light());
    }

    #[test]
    fn test_theme_all() {
        let all_themes: Vec<AppTheme> = AppTheme::all().collect();
        assert_eq!(all_themes.len(), 3);
        assert!(all_themes.contains(&AppTheme::Dark));
        assert!(all_themes.contains(&AppTheme::Light));
        assert!(all_themes.contains(&AppTheme::Custom));
    }

    #[test]
    fn test_theme_default() {
        assert_eq!(AppTheme::default(), AppTheme::Dark);
    }

    #[test]
    fn test_theme_string_conversion() {
        assert_eq!(theme_to_string(AppTheme::Dark), "dark");
        assert_eq!(theme_to_string(AppTheme::Light), "light");
        assert_eq!(theme_to_string(AppTheme::Custom), "custom");
        
        assert_eq!(theme_from_string("dark"), AppTheme::Dark);
        assert_eq!(theme_from_string("light"), AppTheme::Light);
        assert_eq!(theme_from_string("custom"), AppTheme::Custom);
        assert_eq!(theme_from_string("invalid"), AppTheme::Dark); // Default
    }

    #[test]
    fn test_theme_colors() {
        // Test that colors are different between themes
        assert_ne!(AppTheme::Dark.panel_background(), AppTheme::Light.panel_background());
        assert_ne!(AppTheme::Dark.text_color(), AppTheme::Light.text_color());
        assert_ne!(AppTheme::Dark.input_background(), AppTheme::Light.input_background());
        assert_ne!(AppTheme::Dark.border_color(), AppTheme::Light.border_color());
    }

    #[test]
    fn test_egui_visuals_conversion() {
        let dark_visuals = AppTheme::Dark.to_egui_visuals();
        let light_visuals = AppTheme::Light.to_egui_visuals();
        
        // Verify they're different
        assert_ne!(dark_visuals.window_fill, light_visuals.window_fill);
        assert_ne!(dark_visuals.panel_fill, light_visuals.panel_fill);
    }

    // Comprehensive tests for each color property
    #[test]
    fn test_all_background_colors_work() {
        let themes = [AppTheme::Dark, AppTheme::Light, AppTheme::Custom];
        
        for theme in themes {
            // Test that all background color methods return valid colors
            let panel_bg = theme.panel_background();
            let input_bg = theme.input_background();
            let button_bg = theme.button_background();
            let code_bg = theme.code_background();
            let thinking_bg = theme.thinking_background();
            
            // Verify colors are not transparent (alpha > 0)
            assert!(panel_bg.a() > 0, "Panel background should not be transparent for {:?}", theme);
            assert!(input_bg.a() > 0, "Input background should not be transparent for {:?}", theme);
            assert!(button_bg.a() > 0, "Button background should not be transparent for {:?}", theme);
            assert!(code_bg.a() > 0, "Code background should not be transparent for {:?}", theme);
            assert!(thinking_bg.a() > 0, "Thinking background should not be transparent for {:?}", theme);
            
            // Test that backgrounds are different from each other
            assert_ne!(panel_bg, input_bg, "Panel and input backgrounds should be different for {:?}", theme);
        }
    }

    #[test]
    fn test_all_text_colors_work() {
        let themes = [AppTheme::Dark, AppTheme::Light, AppTheme::Custom];
        
        for theme in themes {
            // Test that all text color methods return valid colors
            let text = theme.text_color();
            let hint = theme.hint_text_color();
            let code = theme.code_text_color();
            let thinking = theme.thinking_text_color();
            let timestamp = theme.timestamp_color();
            
            // Verify colors are not transparent
            assert!(text.a() > 0, "Text color should not be transparent for {:?}", theme);
            assert!(hint.a() > 0, "Hint text color should not be transparent for {:?}", theme);
            assert!(code.a() > 0, "Code text color should not be transparent for {:?}", theme);
            assert!(thinking.a() > 0, "Thinking text color should not be transparent for {:?}", theme);
            assert!(timestamp.a() > 0, "Timestamp color should not be transparent for {:?}", theme);
            
            // Test that main text color is different from hint text
            assert_ne!(text, hint, "Main text and hint text should be different colors for {:?}", theme);
        }
    }

    #[test]
    fn test_all_accent_colors_work() {
        let themes = [AppTheme::Dark, AppTheme::Light, AppTheme::Custom];
        
        for theme in themes {
            let accent = theme.accent_color();
            let success = theme.success_color();
            let warning = theme.warning_color();
            let error = theme.error_color();
            
            // Verify colors are not transparent
            assert!(accent.a() > 0, "Accent color should not be transparent for {:?}", theme);
            assert!(success.a() > 0, "Success color should not be transparent for {:?}", theme);
            assert!(warning.a() > 0, "Warning color should not be transparent for {:?}", theme);
            assert!(error.a() > 0, "Error color should not be transparent for {:?}", theme);
            
            // Test that accent colors are distinct
            assert_ne!(success, error, "Success and error colors should be different for {:?}", theme);
            assert_ne!(warning, error, "Warning and error colors should be different for {:?}", theme);
        }
    }

    #[test]
    fn test_all_border_colors_work() {
        let themes = [AppTheme::Dark, AppTheme::Light, AppTheme::Custom];
        
        for theme in themes {
            let border = theme.border_color();
            let focus_border = theme.focus_border_color();
            
            // Verify colors are not transparent
            assert!(border.a() > 0, "Border color should not be transparent for {:?}", theme);
            assert!(focus_border.a() > 0, "Focus border color should not be transparent for {:?}", theme);
            
            // Focus border should be different from regular border
            assert_ne!(border, focus_border, "Border and focus border should be different for {:?}", theme);
        }
    }

    #[test]
    fn test_all_button_colors_work() {
        let themes = [AppTheme::Dark, AppTheme::Light, AppTheme::Custom];
        
        for theme in themes {
            let button_bg = theme.button_background();
            let button_hover = theme.button_hover_color();
            let button_disabled = theme.button_disabled_color();
            let button_text = theme.button_text_color();
            let button_disabled_text = theme.button_disabled_text_color();
            
            // Verify colors are not transparent
            assert!(button_bg.a() > 0, "Button background should not be transparent for {:?}", theme);
            assert!(button_hover.a() > 0, "Button hover should not be transparent for {:?}", theme);
            assert!(button_disabled.a() > 0, "Button disabled should not be transparent for {:?}", theme);
            assert!(button_text.a() > 0, "Button text should not be transparent for {:?}", theme);
            assert!(button_disabled_text.a() > 0, "Button disabled text should not be transparent for {:?}", theme);
            
            // Test that button states are different
            assert_ne!(button_bg, button_hover, "Button background and hover should be different for {:?}", theme);
            assert_ne!(button_text, button_disabled_text, "Button text and disabled text should be different for {:?}", theme);
        }
    }

    #[test]
    fn test_all_author_colors_work() {
        let themes = [AppTheme::Dark, AppTheme::Light, AppTheme::Custom];
        
        for theme in themes {
            let user = theme.user_color();
            let agent = theme.agent_color();
            let system = theme.system_color();
            let tool = theme.tool_color();
            
            // Verify colors are not transparent
            assert!(user.a() > 0, "User color should not be transparent for {:?}", theme);
            assert!(agent.a() > 0, "Agent color should not be transparent for {:?}", theme);
            assert!(system.a() > 0, "System color should not be transparent for {:?}", theme);
            assert!(tool.a() > 0, "Tool color should not be transparent for {:?}", theme);
            
            // Test that author colors are distinct
            assert_ne!(user, agent, "User and agent colors should be different for {:?}", theme);
            assert_ne!(agent, system, "Agent and system colors should be different for {:?}", theme);
            assert_ne!(system, tool, "System and tool colors should be different for {:?}", theme);
        }
    }

    #[test]
    fn test_all_status_colors_work() {
        let themes = [AppTheme::Dark, AppTheme::Light, AppTheme::Custom];
        
        for theme in themes {
            let streaming = theme.streaming_color();
            let thinking = theme.thinking_indicator_color();
            let complete = theme.complete_color();
            
            // Verify colors are not transparent
            assert!(streaming.a() > 0, "Streaming color should not be transparent for {:?}", theme);
            assert!(thinking.a() > 0, "Thinking indicator should not be transparent for {:?}", theme);
            assert!(complete.a() > 0, "Complete color should not be transparent for {:?}", theme);
            
            // Test that status colors are distinct
            assert_ne!(streaming, thinking, "Streaming and thinking colors should be different for {:?}", theme);
            assert_ne!(thinking, complete, "Thinking and complete colors should be different for {:?}", theme);
        }
    }

    #[test]
    fn test_custom_theme_colors_persistence() {
        // Test that custom colors can be set and retrieved
        let custom_colors = CustomThemeColors {
            panel_background: Color32::from_rgb(100, 100, 100),
            text_color: Color32::from_rgb(200, 200, 200),
            accent_color: Color32::from_rgb(255, 0, 0),
            ..CustomThemeColors::default()
        };
        
        set_custom_theme_colors(custom_colors.clone());
        let retrieved_colors = get_custom_theme_colors();
        
        assert_eq!(custom_colors.panel_background, retrieved_colors.panel_background);
        assert_eq!(custom_colors.text_color, retrieved_colors.text_color);
        assert_eq!(custom_colors.accent_color, retrieved_colors.accent_color);
    }

    #[test]
    fn test_custom_theme_color_application() {
        // Test that custom theme actually uses the custom colors
        let custom_colors = CustomThemeColors {
            panel_background: Color32::from_rgb(123, 45, 67),
            text_color: Color32::from_rgb(89, 123, 45),
            accent_color: Color32::from_rgb(67, 89, 123),
            ..CustomThemeColors::default()
        };
        
        set_custom_theme_colors(custom_colors.clone());
        
        let custom_theme = AppTheme::Custom;
        assert_eq!(custom_theme.panel_background(), custom_colors.panel_background);
        assert_eq!(custom_theme.text_color(), custom_colors.text_color);
        assert_eq!(custom_theme.accent_color(), custom_colors.accent_color);
    }

    #[test]
    fn test_theme_color_contrast() {
        // Test that text colors have sufficient contrast with background colors
        let themes = [AppTheme::Dark, AppTheme::Light];
        
        for theme in themes {
            let bg = theme.panel_background();
            let text = theme.text_color();
            
            // Calculate simple contrast (difference in luminance)
            let bg_luminance = (bg.r() as f32 * 0.299 + bg.g() as f32 * 0.587 + bg.b() as f32 * 0.114) / 255.0;
            let text_luminance = (text.r() as f32 * 0.299 + text.g() as f32 * 0.587 + text.b() as f32 * 0.114) / 255.0;
            let contrast = (bg_luminance - text_luminance).abs();
            
            // Ensure there's reasonable contrast (at least 0.3 difference)
            assert!(contrast > 0.3, "Text should have sufficient contrast with background for {:?}", theme);
        }
    }

    #[test]
    fn test_all_color_methods_exist_and_work() {
        // This test ensures all 25+ color methods are implemented and return valid colors
        let theme = AppTheme::Dark;
        
        // Background colors (5)
        assert!(theme.panel_background().a() > 0);
        assert!(theme.input_background().a() > 0);
        assert!(theme.button_background().a() > 0);
        assert!(theme.code_background().a() > 0);
        assert!(theme.thinking_background().a() > 0);
        
        // Text colors (5)
        assert!(theme.text_color().a() > 0);
        assert!(theme.hint_text_color().a() > 0);
        assert!(theme.code_text_color().a() > 0);
        assert!(theme.thinking_text_color().a() > 0);
        assert!(theme.timestamp_color().a() > 0);
        
        // Accent colors (4)
        assert!(theme.accent_color().a() > 0);
        assert!(theme.success_color().a() > 0);
        assert!(theme.warning_color().a() > 0);
        assert!(theme.error_color().a() > 0);
        
        // Border colors (2)
        assert!(theme.border_color().a() > 0);
        assert!(theme.focus_border_color().a() > 0);
        
        // Button colors (4)
        assert!(theme.button_hover_color().a() > 0);
        assert!(theme.button_disabled_color().a() > 0);
        assert!(theme.button_text_color().a() > 0);
        assert!(theme.button_disabled_text_color().a() > 0);
        
        // Author colors (4)
        assert!(theme.user_color().a() > 0);
        assert!(theme.agent_color().a() > 0);
        assert!(theme.system_color().a() > 0);
        assert!(theme.tool_color().a() > 0);
        
        // Status colors (3)
        assert!(theme.streaming_color().a() > 0);
        assert!(theme.thinking_indicator_color().a() > 0);
        assert!(theme.complete_color().a() > 0);
        
        // Total: 27 color methods tested
    }

    #[test]
    fn test_theme_consistency_across_modes() {
        // Test that each theme mode returns consistent colors
        let themes = [AppTheme::Dark, AppTheme::Light, AppTheme::Custom];
        
        for theme in themes {
            // Call each color method twice and ensure they return the same value
            assert_eq!(theme.text_color(), theme.text_color());
            assert_eq!(theme.panel_background(), theme.panel_background());
            assert_eq!(theme.accent_color(), theme.accent_color());
            assert_eq!(theme.user_color(), theme.user_color());
            assert_eq!(theme.success_color(), theme.success_color());
        }
    }

    #[test]
    fn test_custom_theme_colors_default() {
        // Test that CustomThemeColors::default() provides valid colors
        let default_colors = CustomThemeColors::default();
        
        // Check that all colors have non-zero alpha
        assert!(default_colors.panel_background.a() > 0);
        assert!(default_colors.text_color.a() > 0);
        assert!(default_colors.accent_color.a() > 0);
        assert!(default_colors.user_color.a() > 0);
        assert!(default_colors.success_color.a() > 0);
        // ... and so on for all colors
    }

    #[test]
    fn test_side_panel_frame() {
        let themes = [AppTheme::Dark, AppTheme::Light, AppTheme::Custom];
        
        for theme in themes {
            let frame = theme.side_panel_frame();
            
            // Check that the frame has the correct margins (8px all around)
            assert_eq!(frame.inner_margin.left, 8, "Side panel frame should have 8px left margin for {:?}", theme);
            assert_eq!(frame.inner_margin.right, 8, "Side panel frame should have 8px right margin for {:?}", theme);
            assert_eq!(frame.inner_margin.top, 8, "Side panel frame should have 8px top margin for {:?}", theme);
            assert_eq!(frame.inner_margin.bottom, 8, "Side panel frame should have 8px bottom margin for {:?}", theme);
            
            // Check that the background is set
            assert_eq!(frame.fill, theme.panel_background(), "Side panel frame should use panel background color for {:?}", theme);
        }
    }
} 