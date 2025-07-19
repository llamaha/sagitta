// Theme customization panel with color pickers for all UI components

use egui::{Context, Ui, Color32, Grid, RichText, ScrollArea, CollapsingHeader, Button, Frame, Vec2, Stroke};
use crate::gui::theme::{AppTheme, CustomThemeColors, get_custom_theme_colors, set_custom_theme_colors};
use rand::Rng;

/// Theme customization panel for fine-tuning all UI colors
#[derive(Clone)]
pub struct ThemeCustomizer {
    pub is_open: bool,
    pub colors: CustomThemeColors,
    pub preview_enabled: bool,
    pub reset_confirmation: bool,
    pub show_test_section: bool,
    pub show_individual_tests: bool,
}

impl Default for ThemeCustomizer {
    fn default() -> Self {
        Self {
            is_open: false,
            colors: get_custom_theme_colors(),
            preview_enabled: true,
            reset_confirmation: false,
            show_test_section: false,
            show_individual_tests: false,
        }
    }
}

impl ThemeCustomizer {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Toggle the panel visibility
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
        if self.is_open {
            // Load current custom colors when opening
            self.colors = get_custom_theme_colors();
        }
    }
    
    /// Check if the panel is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }
    
    /// Apply the current colors to the custom theme
    pub fn apply_colors(&self) {
        set_custom_theme_colors(self.colors.clone());
    }
    
    /// Reset to default colors
    pub fn reset_to_defaults(&mut self) {
        self.colors = CustomThemeColors::default();
        if self.preview_enabled {
            self.apply_colors();
        }
    }
    
    /// Load preset colors (dark or light)
    pub fn load_preset(&mut self, preset: AppTheme) {
        match preset {
            AppTheme::Dark => {
                self.colors = CustomThemeColors::default(); // Default is dark
            },
            AppTheme::Light => {
                self.colors = CustomThemeColors {
                    // Background colors
                    panel_background: Color32::from_rgb(248, 248, 248),
                    input_background: Color32::from_rgb(255, 255, 255),
                    button_background: Color32::from_rgb(240, 240, 240),
                    code_background: Color32::from_rgb(250, 250, 250),
                    thinking_background: Color32::from_rgb(245, 245, 245),
                    tool_card_background: Color32::from_rgb(242, 242, 248),
                    
                    // Text colors
                    text_color: Color32::from_rgb(60, 60, 60),
                    hint_text_color: Color32::from_rgb(128, 128, 128),
                    code_text_color: Color32::from_rgb(40, 40, 40),
                    thinking_text_color: Color32::from_rgb(80, 80, 80),
                    timestamp_color: Color32::from_rgb(128, 128, 128),
                    
                    // Accent and highlight colors
                    accent_color: Color32::from_rgb(70, 130, 180),
                    success_color: Color32::from_rgb(34, 139, 34),
                    warning_color: Color32::from_rgb(255, 140, 0),
                    error_color: Color32::from_rgb(220, 20, 60),
                    
                    // Border and stroke colors
                    border_color: Color32::from_rgb(200, 200, 200),
                    focus_border_color: Color32::from_rgb(70, 130, 180),
                    tool_card_border_color: Color32::from_rgb(180, 180, 190),
                    
                    // Button states
                    button_hover_color: Color32::from_rgb(230, 230, 230),
                    button_disabled_color: Color32::from_rgb(200, 200, 200),
                    button_text_color: Color32::from_rgb(60, 60, 60),
                    button_disabled_text_color: Color32::from_rgb(120, 120, 120),
                    
                    // Author colors
                    user_color: Color32::from_rgb(60, 60, 60),
                    agent_color: Color32::from_rgb(34, 139, 34),
                    system_color: Color32::from_rgb(220, 20, 60),
                    tool_color: Color32::from_rgb(255, 140, 0),
                    
                    // Status indicators
                    streaming_color: Color32::from_rgb(34, 139, 34),
                    thinking_indicator_color: Color32::from_rgb(70, 130, 180),
                    complete_color: Color32::from_rgb(34, 139, 34),
                    
                    // Diff colors
                    diff_added_bg: Color32::from_rgb(200, 255, 200),   // Light green background
                    diff_removed_bg: Color32::from_rgb(255, 200, 200), // Light red background
                    diff_added_text: Color32::from_rgb(0, 100, 0),     // Dark green text
                    diff_removed_text: Color32::from_rgb(100, 0, 0),   // Dark red text
                    
                    // Font sizes
                    base_font_size: 14.0,
                    header_font_size: 16.0,
                    code_font_size: 13.0,
                    small_font_size: 11.0,
                };
            },
            AppTheme::Custom => {
                // Keep current colors
            }
        }
        
        if self.preview_enabled {
            self.apply_colors();
        }
    }
    
    /// Render the theme customization panel
    pub fn render(&mut self, ctx: &Context) -> bool {
        let mut theme_changed = false;
        
        if !self.is_open {
            return theme_changed;
        }
        
        egui::SidePanel::right("theme_customizer_panel")
            .resizable(true)
            .default_width(450.0)
            .min_width(400.0)
            .max_width(700.0)
            .frame(Frame::NONE.fill(self.colors.panel_background))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Header
                    ui.horizontal(|ui| {
                        ui.heading("🎨 Theme Customizer");
                        ui.add_space(8.0);
                        if ui.button("×").clicked() {
                            self.is_open = false;
                        }
                    });
                    ui.separator();
                    
                    // Controls
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.preview_enabled, "Live Preview");
                        ui.add_space(8.0);
                        
                        if ui.button("Apply").clicked() {
                            self.apply_colors();
                            theme_changed = true;
                        }
                        
                        if ui.button("Reset").clicked() {
                            if self.reset_confirmation {
                                self.reset_to_defaults();
                                self.reset_confirmation = false;
                                theme_changed = true;
                            } else {
                                self.reset_confirmation = true;
                            }
                        }
                        
                        if self.reset_confirmation {
                            ui.label(RichText::new("Click Reset again to confirm").color(Color32::from_rgb(255, 100, 100)));
                        }
                    });
                    
                    ui.add_space(8.0);
                    
                    // Preset buttons
                    ui.horizontal(|ui| {
                        ui.label("Presets:");
                        if ui.button("Dark").clicked() {
                            self.load_preset(AppTheme::Dark);
                            theme_changed = true;
                        }
                        if ui.button("Light").clicked() {
                            self.load_preset(AppTheme::Light);
                            theme_changed = true;
                        }
                        if ui.button("🎲 Random").on_hover_text("Generate a random theme using color theory").clicked() {
                            self.generate_random_theme();
                            theme_changed = true;
                        }
                    });
                    
                    ui.add_space(8.0);
                    
                    // Export/Import buttons
                    ui.horizontal(|ui| {
                        ui.label("Share:");
                        if ui.button("📤 Export").on_hover_text("Export theme to .sagitta-theme.json file").clicked() {
                            self.export_theme();
                        }
                        if ui.button("📥 Import").on_hover_text("Import theme from .sagitta-theme.json file").clicked() {
                            self.import_theme();
                            theme_changed = true;
                        }
                    });
                    
                    ui.add_space(8.0);
                    
                    // Test section toggles
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.show_test_section, "Show Test Section");
                        ui.checkbox(&mut self.show_individual_tests, "Individual Tests");
                    });
                    
                    ui.add_space(8.0);
                    ui.separator();
                    
                    // Color customization sections
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // Test section first if enabled
                            if self.show_test_section {
                                if self.render_test_section(ui) {
                                    theme_changed = true;
                                }
                                ui.separator();
                                ui.add_space(8.0);
                            }
                            
                            // Individual tests if enabled
                            if self.show_individual_tests {
                                if self.render_individual_tests(ui) {
                                    theme_changed = true;
                                }
                                ui.separator();
                                ui.add_space(8.0);
                            }
                            
                            if self.render_font_sizes(ui) {
                                theme_changed = true;
                            }
                            if self.render_background_colors(ui) {
                                theme_changed = true;
                            }
                            
                            if self.render_text_colors(ui) {
                                theme_changed = true;
                            }
                            
                            if self.render_accent_colors(ui) {
                                theme_changed = true;
                            }
                            
                            if self.render_border_colors(ui) {
                                theme_changed = true;
                            }
                            
                            if self.render_button_colors(ui) {
                                theme_changed = true;
                            }
                            
                            if self.render_author_colors(ui) {
                                theme_changed = true;
                            }
                            
                            if self.render_status_colors(ui) {
                                theme_changed = true;
                            }
                            
                            if self.render_diff_colors(ui) {
                                theme_changed = true;
                            }
                        });
                });
            });
        
        // Apply live preview if enabled
        if theme_changed && self.preview_enabled {
            self.apply_colors();
        }
        
        theme_changed
    }
    
    /// Render comprehensive test section showing all UI elements
    fn render_test_section(&mut self, ui: &mut Ui) -> bool {
        let changed = false;
        
        CollapsingHeader::new("🧪 Test Section - Preview All Colors")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("This section shows samples of all UI elements to test color changes:");
                ui.add_space(8.0);
                
                // Panel background test
                Frame::NONE
                    .fill(self.colors.panel_background)
                    .stroke(Stroke::new(1.0, self.colors.border_color))
                    .inner_margin(Vec2::splat(8.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new("Panel Background").color(self.colors.text_color));
                        
                        // Input background test
                        Frame::NONE
                            .fill(self.colors.input_background)
                            .stroke(Stroke::new(1.0, self.colors.focus_border_color))
                            .inner_margin(Vec2::splat(4.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new("Input Background").color(self.colors.text_color));
                            });
                        
                        ui.add_space(4.0);
                        
                        // Button tests
                        ui.horizontal(|ui| {
                            let button = Button::new(RichText::new("Normal Button").color(self.colors.button_text_color))
                                .fill(self.colors.button_background);
                            ui.add(button);
                            
                            let hover_button = Button::new(RichText::new("Hover Button").color(self.colors.button_text_color))
                                .fill(self.colors.button_hover_color);
                            ui.add(hover_button);
                            
                            let disabled_button = Button::new(RichText::new("Disabled").color(self.colors.button_disabled_text_color))
                                .fill(self.colors.button_disabled_color);
                            ui.add_enabled(false, disabled_button);
                        });
                        
                        ui.add_space(4.0);
                        
                        // Text color tests
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Main Text").color(self.colors.text_color));
                            ui.label(RichText::new("Hint Text").color(self.colors.hint_text_color));
                            ui.label(RichText::new("Timestamp").color(self.colors.timestamp_color).small());
                        });
                        
                        ui.add_space(4.0);
                        
                        // Code background test
                        Frame::NONE
                            .fill(self.colors.code_background)
                            .stroke(Stroke::new(1.0, self.colors.border_color))
                            .inner_margin(Vec2::splat(4.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new("Code Background").color(self.colors.code_text_color).monospace());
                            });
                        
                        ui.add_space(4.0);
                        
                        // Thinking background test
                        Frame::NONE
                            .fill(self.colors.thinking_background)
                            .stroke(Stroke::new(1.0, self.colors.border_color))
                            .inner_margin(Vec2::splat(4.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new("Thinking Background").color(self.colors.thinking_text_color).italics());
                            });
                        
                        ui.add_space(4.0);
                        
                        // Tool card background test
                        Frame::NONE
                            .fill(self.colors.tool_card_background)
                            .stroke(Stroke::new(1.0, self.colors.border_color))
                            .inner_margin(Vec2::splat(4.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new("🔧 Tool Card Background").color(self.colors.tool_color).strong());
                            });
                        
                        ui.add_space(4.0);
                        
                        // Accent colors test
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Accent").color(self.colors.accent_color));
                            ui.label(RichText::new("Success").color(self.colors.success_color));
                            ui.label(RichText::new("Warning").color(self.colors.warning_color));
                            ui.label(RichText::new("Error").color(self.colors.error_color));
                        });
                        
                        ui.add_space(4.0);
                        
                        // Author colors test
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("You").color(self.colors.user_color));
                            ui.label(RichText::new("Sagitta Code").color(self.colors.agent_color));
                            ui.label(RichText::new("System").color(self.colors.system_color));
                            ui.label(RichText::new("Tool").color(self.colors.tool_color));
                        });
                        
                        ui.add_space(4.0);
                        
                        // Status indicators test
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("● Streaming").color(self.colors.streaming_color));
                            ui.label(RichText::new("● Thinking").color(self.colors.thinking_indicator_color));
                            ui.label(RichText::new("● Complete").color(self.colors.complete_color));
                        });
                        
                        ui.add_space(4.0);
                        
                        // Diff colors test
                        ui.horizontal(|ui| {
                            // Added diff example
                            Frame::NONE
                                .fill(self.colors.diff_added_bg)
                                .inner_margin(Vec2::splat(2.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new("+ Added").color(self.colors.diff_added_text).monospace());
                                });
                            
                            // Removed diff example
                            Frame::NONE
                                .fill(self.colors.diff_removed_bg)
                                .inner_margin(Vec2::splat(2.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new("- Removed").color(self.colors.diff_removed_text).monospace());
                                });
                        });
                    });
            });
        
        changed
    }
    
    /// Render individual tests for each color property
    fn render_individual_tests(&mut self, ui: &mut Ui) -> bool {
        let changed = false;
        
        CollapsingHeader::new("🔬 Individual Color Tests - Isolated Examples")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Each test shows exactly which UI elements should change for each color:");
                ui.add_space(8.0);
                
                // Background color tests
                CollapsingHeader::new("🏠 Background Color Tests")
                    .default_open(false)
                    .show(ui, |ui| {
                        self.test_panel_background(ui);
                        ui.add_space(4.0);
                        self.test_input_background(ui);
                        ui.add_space(4.0);
                        self.test_button_background(ui);
                        ui.add_space(4.0);
                        self.test_code_background(ui);
                        ui.add_space(4.0);
                        self.test_thinking_background(ui);
                        ui.add_space(4.0);
                        self.test_tool_card_background(ui);
                    });
                
                ui.add_space(8.0);
                
                // Text color tests
                CollapsingHeader::new("📝 Text Color Tests")
                    .default_open(false)
                    .show(ui, |ui| {
                        self.test_text_color(ui);
                        ui.add_space(4.0);
                        self.test_hint_text_color(ui);
                        ui.add_space(4.0);
                        self.test_code_text_color(ui);
                        ui.add_space(4.0);
                        self.test_thinking_text_color(ui);
                        ui.add_space(4.0);
                        self.test_timestamp_color(ui);
                    });
                
                ui.add_space(8.0);
                
                // Accent color tests
                CollapsingHeader::new("✨ Accent Color Tests")
                    .default_open(false)
                    .show(ui, |ui| {
                        self.test_accent_color(ui);
                        ui.add_space(4.0);
                        self.test_success_color(ui);
                        ui.add_space(4.0);
                        self.test_warning_color(ui);
                        ui.add_space(4.0);
                        self.test_error_color(ui);
                    });
                
                ui.add_space(8.0);
                
                // Border color tests
                CollapsingHeader::new("🔲 Border Color Tests")
                    .default_open(false)
                    .show(ui, |ui| {
                        self.test_border_color(ui);
                        ui.add_space(4.0);
                        self.test_focus_border_color(ui);
                        ui.add_space(4.0);
                        self.test_tool_card_border_color(ui);
                    });
                
                ui.add_space(8.0);
                
                // Button color tests
                CollapsingHeader::new("🔘 Button Color Tests")
                    .default_open(false)
                    .show(ui, |ui| {
                        self.test_button_hover_color(ui);
                        ui.add_space(4.0);
                        self.test_button_disabled_color(ui);
                        ui.add_space(4.0);
                        self.test_button_text_color(ui);
                        ui.add_space(4.0);
                        self.test_button_disabled_text_color(ui);
                    });
                
                ui.add_space(8.0);
                
                // Author color tests
                CollapsingHeader::new("👤 Author Color Tests")
                    .default_open(false)
                    .show(ui, |ui| {
                        self.test_user_color(ui);
                        ui.add_space(4.0);
                        self.test_agent_color(ui);
                        ui.add_space(4.0);
                        self.test_system_color(ui);
                        ui.add_space(4.0);
                        self.test_tool_color(ui);
                    });
                
                ui.add_space(8.0);
                
                // Status color tests
                CollapsingHeader::new("📊 Status Color Tests")
                    .default_open(false)
                    .show(ui, |ui| {
                        self.test_streaming_color(ui);
                        ui.add_space(4.0);
                        self.test_thinking_indicator_color(ui);
                        ui.add_space(4.0);
                        self.test_complete_color(ui);
                    });
                
                ui.add_space(8.0);
                
                // Diff color tests
                CollapsingHeader::new("🔄 Diff Color Tests")
                    .default_open(false)
                    .show(ui, |ui| {
                        self.test_diff_added_bg(ui);
                        ui.add_space(4.0);
                        self.test_diff_removed_bg(ui);
                        ui.add_space(4.0);
                        self.test_diff_added_text(ui);
                        ui.add_space(4.0);
                        self.test_diff_removed_text(ui);
                    });
            });
        
        changed
    }

    // Individual test methods for each color property
    fn test_panel_background(&self, ui: &mut Ui) {
        ui.label(RichText::new("Panel Background Test:").strong());
        Frame::NONE
            .fill(self.colors.panel_background)
            .stroke(Stroke::new(2.0, Color32::RED)) // Red border to highlight the test area
            .inner_margin(Vec2::splat(8.0))
            .show(ui, |ui| {
                ui.label("This entire frame should use the panel background color");
                ui.label("Used in: Main panels, side panels, dialog backgrounds");
            });
    }

    fn test_input_background(&self, ui: &mut Ui) {
        ui.label(RichText::new("Input Background Test:").strong());
        Frame::NONE
            .fill(self.colors.input_background)
            .stroke(Stroke::new(2.0, Color32::RED))
            .inner_margin(Vec2::splat(8.0))
            .show(ui, |ui| {
                ui.label("This frame simulates input field background");
                ui.label("Used in: Text inputs, text areas, search boxes");
            });
    }

    fn test_button_background(&self, ui: &mut Ui) {
        ui.label(RichText::new("Button Background Test:").strong());
        ui.horizontal(|ui| {
            let button = Button::new("Normal Button")
                .fill(self.colors.button_background);
            ui.add(button);
            ui.label("← This button uses the button background color");
        });
        ui.label("Used in: All buttons in normal state");
    }

    fn test_code_background(&self, ui: &mut Ui) {
        ui.label(RichText::new("Code Background Test:").strong());
        Frame::NONE
            .fill(self.colors.code_background)
            .stroke(Stroke::new(2.0, Color32::RED))
            .inner_margin(Vec2::splat(8.0))
            .show(ui, |ui| {
                ui.label(RichText::new("fn main() { println!(\"Hello\"); }").monospace());
                ui.label("Used in: Code blocks, syntax highlighting backgrounds");
            });
    }

    fn test_thinking_background(&self, ui: &mut Ui) {
        ui.label(RichText::new("Thinking Background Test:").strong());
        Frame::NONE
            .fill(self.colors.thinking_background)
            .stroke(Stroke::new(2.0, Color32::RED))
            .inner_margin(Vec2::splat(8.0))
            .show(ui, |ui| {
                ui.label(RichText::new("Sagitta Code is thinking about your request...").italics());
                ui.label("Used in: Thinking bubbles, reasoning displays");
            });
    }

    fn test_tool_card_background(&self, ui: &mut Ui) {
        ui.label(RichText::new("Tool Card Background Test:").strong());
        Frame::NONE
            .fill(self.colors.tool_card_background)
            .stroke(Stroke::new(2.0, Color32::RED))
            .inner_margin(Vec2::splat(8.0))
            .show(ui, |ui| {
                ui.label(RichText::new("🔧 Tool: Read File - path: /example.txt").color(self.colors.tool_color).strong());
                ui.label("This frame simulates a tool card background");
                ui.label("Used in: Tool execution cards, function call results");
            });
    }

    fn test_text_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Main Text Color Test:").strong());
        ui.label(RichText::new("This is the main text color used throughout the application").color(self.colors.text_color));
        ui.label("Used in: Primary text, labels, most UI text content");
        ui.label(RichText::new("⚠️ This is the most important color - it should be visible everywhere!").color(self.colors.text_color).strong());
    }

    fn test_hint_text_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Hint Text Color Test:").strong());
        ui.label(RichText::new("This is hint text - subdued and secondary").color(self.colors.hint_text_color));
        ui.label("Used in: Placeholder text, help text, secondary information");
    }

    fn test_code_text_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Code Text Color Test:").strong());
        ui.label(RichText::new("let code_text = \"This is code text\";").color(self.colors.code_text_color).monospace());
        ui.label("Used in: Code content, monospace text in code blocks");
    }

    fn test_thinking_text_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Thinking Text Color Test:").strong());
        ui.label(RichText::new("This is thinking text - used in reasoning displays").color(self.colors.thinking_text_color).italics());
        ui.label("Used in: Thinking content, reasoning text, internal monologue");
    }

    fn test_timestamp_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Timestamp Color Test:").strong());
        ui.label(RichText::new("2024-01-15 14:30:25").color(self.colors.timestamp_color).small());
        ui.label("Used in: Message timestamps, time displays, date information");
    }

    fn test_accent_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Accent Color Test:").strong());
        ui.label(RichText::new("This is the accent color for highlights and focus").color(self.colors.accent_color));
        ui.label("Used in: Focus indicators, highlights, important UI elements");
    }

    fn test_success_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Success Color Test:").strong());
        ui.label(RichText::new("✓ Success message or positive status").color(self.colors.success_color));
        ui.label("Used in: Success messages, positive status indicators, completion states");
    }

    fn test_warning_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Warning Color Test:").strong());
        ui.label(RichText::new("⚠ Warning message or caution status").color(self.colors.warning_color));
        ui.label("Used in: Warning messages, caution indicators, attention-needed states");
    }

    fn test_error_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Error Color Test:").strong());
        ui.label(RichText::new("✗ Error message or failure status").color(self.colors.error_color));
        ui.label("Used in: Error messages, failure indicators, problem states");
    }

    fn test_border_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Border Color Test:").strong());
        Frame::NONE
            .stroke(Stroke::new(3.0, self.colors.border_color))
            .inner_margin(Vec2::splat(8.0))
            .show(ui, |ui| {
                ui.label("This frame has a border using the border color");
            });
        ui.label("Used in: Frame borders, panel separators, UI element outlines");
    }

    fn test_focus_border_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Focus Border Color Test:").strong());
        Frame::NONE
            .stroke(Stroke::new(3.0, self.colors.focus_border_color))
            .inner_margin(Vec2::splat(8.0))
            .show(ui, |ui| {
                ui.label("This frame simulates a focused element border");
            });
        ui.label("Used in: Focused input fields, selected elements, active UI components");
    }

    fn test_tool_card_border_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Tool Card Border Color Test:").strong());
        Frame::NONE
            .fill(self.colors.tool_card_background)
            .stroke(Stroke::new(0.5, self.colors.tool_card_border_color.linear_multiply(0.3)))
            .inner_margin(Vec2::splat(12.0))
            .corner_radius(egui::CornerRadius::same(6))
            .shadow(egui::Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color: Color32::from_black_alpha(25),
            })
            .show(ui, |ui| {
                ui.label("This simulates a tool card appearance");
            });
        ui.label("Used in: Tool cards in chat messages");
    }

    fn test_button_hover_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Button Hover Color Test:").strong());
        ui.horizontal(|ui| {
            let button = Button::new("Hover State Button")
                .fill(self.colors.button_hover_color);
            ui.add(button);
            ui.label("← This simulates a hovered button");
        });
        ui.label("Used in: Buttons when mouse hovers over them");
    }

    fn test_button_disabled_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Button Disabled Color Test:").strong());
        ui.horizontal(|ui| {
            let button = Button::new("Disabled Button")
                .fill(self.colors.button_disabled_color);
            ui.add_enabled(false, button);
            ui.label("← This is a disabled button");
        });
        ui.label("Used in: Buttons that are disabled/inactive");
    }

    fn test_button_text_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Button Text Color Test:").strong());
        ui.horizontal(|ui| {
            let button = Button::new(RichText::new("Button Text").color(self.colors.button_text_color))
                .fill(Color32::GRAY);
            ui.add(button);
            ui.label("← The text color in this button");
        });
        ui.label("Used in: Text inside buttons");
    }

    fn test_button_disabled_text_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Button Disabled Text Color Test:").strong());
        ui.horizontal(|ui| {
            let button = Button::new(RichText::new("Disabled Text").color(self.colors.button_disabled_text_color))
                .fill(Color32::GRAY);
            ui.add_enabled(false, button);
            ui.label("← The text color in disabled buttons");
        });
        ui.label("Used in: Text inside disabled buttons");
    }

    fn test_user_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("User Color Test:").strong());
        ui.label(RichText::new("👤 You: This is a user message").color(self.colors.user_color));
        ui.label("Used in: User messages, user indicators, 'You' labels");
    }

    fn test_agent_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Agent Color Test:").strong());
        ui.label(RichText::new("🤖 Sagitta Code: This is an agent message").color(self.colors.agent_color));
        ui.label("Used in: Agent messages, Sagitta Code's responses, AI indicators");
    }

    fn test_system_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("System Color Test:").strong());
        ui.label(RichText::new("⚙️ System: This is a system message").color(self.colors.system_color));
        ui.label("Used in: System messages, internal notifications, status updates");
    }

    fn test_tool_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Tool Color Test:").strong());
        ui.label(RichText::new("🔧 Tool: This is a tool message").color(self.colors.tool_color));
        ui.label("Used in: Tool outputs, function results, external tool responses");
    }

    fn test_streaming_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Streaming Color Test:").strong());
        ui.label(RichText::new("⟳ Streaming content...").color(self.colors.streaming_color));
        ui.label("Used in: Streaming indicators, live content updates, real-time status");
    }

    fn test_thinking_indicator_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Thinking Indicator Color Test:").strong());
        ui.label(RichText::new("💭 Thinking...").color(self.colors.thinking_indicator_color));
        ui.label("Used in: Thinking indicators, reasoning status, processing states");
    }

    fn test_complete_color(&self, ui: &mut Ui) {
        ui.label(RichText::new("Complete Color Test:").strong());
        ui.label(RichText::new("✓ Complete").color(self.colors.complete_color));
        ui.label("Used in: Completion indicators, finished states, done status");
    }

    fn test_diff_added_bg(&self, ui: &mut Ui) {
        ui.label(RichText::new("Added Background Test:").strong());
        Frame::NONE
            .fill(self.colors.diff_added_bg)
            .stroke(Stroke::new(2.0, Color32::RED))
            .inner_margin(Vec2::splat(8.0))
            .show(ui, |ui| {
                ui.label("This entire frame should use the added background color");
                ui.label("Used in: Added background areas, new content additions");
            });
    }

    fn test_diff_removed_bg(&self, ui: &mut Ui) {
        ui.label(RichText::new("Removed Background Test:").strong());
        Frame::NONE
            .fill(self.colors.diff_removed_bg)
            .stroke(Stroke::new(2.0, Color32::RED))
            .inner_margin(Vec2::splat(8.0))
            .show(ui, |ui| {
                ui.label("This entire frame should use the removed background color");
                ui.label("Used in: Removed background areas, content deletions");
            });
    }

    fn test_diff_added_text(&self, ui: &mut Ui) {
        ui.label(RichText::new("Added Text Test:").strong());
        ui.label(RichText::new("This text should use the added text color").color(self.colors.diff_added_text));
        ui.label("Used in: Added text content, new information introductions");
    }

    fn test_diff_removed_text(&self, ui: &mut Ui) {
        ui.label(RichText::new("Removed Text Test:").strong());
        ui.label(RichText::new("This text should use the removed text color").color(self.colors.diff_removed_text));
        ui.label("Used in: Removed text content, information removals");
    }
    
    /// Render background color controls
    fn render_font_sizes(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        
        ui.heading("📏 Font Sizes");
        ui.add_space(8.0);
        
        egui::Grid::new("font_sizes_grid")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Base Font Size:");
                if ui.add(egui::Slider::new(&mut self.colors.base_font_size, 10.0..=20.0).suffix(" px")).changed() {
                    changed = true;
                }
                ui.end_row();
                
                ui.label("Header Font Size:");
                if ui.add(egui::Slider::new(&mut self.colors.header_font_size, 12.0..=24.0).suffix(" px")).changed() {
                    changed = true;
                }
                ui.end_row();
                
                ui.label("Code Font Size:");
                if ui.add(egui::Slider::new(&mut self.colors.code_font_size, 10.0..=18.0).suffix(" px")).changed() {
                    changed = true;
                }
                ui.end_row();
                
                ui.label("Small Font Size:");
                if ui.add(egui::Slider::new(&mut self.colors.small_font_size, 9.0..=14.0).suffix(" px")).changed() {
                    changed = true;
                }
                ui.end_row();
            });
        
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
        
        if self.preview_enabled && changed {
            self.apply_colors();
        }
        
        changed
    }
    
    fn render_background_colors(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        
        CollapsingHeader::new("🏠 Background Colors")
            .default_open(true)
            .show(ui, |ui| {
                Grid::new("background_colors_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        changed |= color_picker_standalone(ui, "Panel Background", &mut self.colors.panel_background);
                        changed |= color_picker_standalone(ui, "Input Background", &mut self.colors.input_background);
                        changed |= color_picker_standalone(ui, "Button Background", &mut self.colors.button_background);
                        changed |= color_picker_standalone(ui, "Code Background", &mut self.colors.code_background);
                        changed |= color_picker_standalone(ui, "Thinking Background", &mut self.colors.thinking_background);
                        changed |= color_picker_standalone(ui, "Tool Card Background", &mut self.colors.tool_card_background);
                    });
            });
        
        changed
    }
    
    /// Render text color controls
    fn render_text_colors(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        
        CollapsingHeader::new("📝 Text Colors")
            .default_open(true)
            .show(ui, |ui| {
                Grid::new("text_colors_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        changed |= color_picker_standalone(ui, "Main Text", &mut self.colors.text_color);
                        changed |= color_picker_standalone(ui, "Hint Text", &mut self.colors.hint_text_color);
                        changed |= color_picker_standalone(ui, "Code Text", &mut self.colors.code_text_color);
                        changed |= color_picker_standalone(ui, "Thinking Text", &mut self.colors.thinking_text_color);
                        changed |= color_picker_standalone(ui, "Timestamp", &mut self.colors.timestamp_color);
                    });
            });
        
        changed
    }
    
    /// Render accent color controls
    fn render_accent_colors(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        
        CollapsingHeader::new("✨ Accent & Highlight Colors")
            .default_open(true)
            .show(ui, |ui| {
                Grid::new("accent_colors_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        changed |= color_picker_standalone(ui, "Accent Color", &mut self.colors.accent_color);
                        changed |= color_picker_standalone(ui, "Success Color", &mut self.colors.success_color);
                        changed |= color_picker_standalone(ui, "Warning Color", &mut self.colors.warning_color);
                        changed |= color_picker_standalone(ui, "Error Color", &mut self.colors.error_color);
                    });
            });
        
        changed
    }
    
    /// Render border color controls
    fn render_border_colors(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        
        CollapsingHeader::new("🔲 Border Colors")
            .default_open(false)
            .show(ui, |ui| {
                Grid::new("border_colors_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        changed |= color_picker_standalone(ui, "Border Color", &mut self.colors.border_color);
                        changed |= color_picker_standalone(ui, "Focus Border", &mut self.colors.focus_border_color);
                        changed |= color_picker_standalone(ui, "Tool Card Border", &mut self.colors.tool_card_border_color);
                    });
            });
        
        changed
    }
    
    /// Render button color controls
    fn render_button_colors(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        
        CollapsingHeader::new("🔘 Button Colors")
            .default_open(false)
            .show(ui, |ui| {
                Grid::new("button_colors_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        changed |= color_picker_standalone(ui, "Button Hover", &mut self.colors.button_hover_color);
                        changed |= color_picker_standalone(ui, "Button Disabled", &mut self.colors.button_disabled_color);
                        changed |= color_picker_standalone(ui, "Button Text", &mut self.colors.button_text_color);
                        changed |= color_picker_standalone(ui, "Disabled Text", &mut self.colors.button_disabled_text_color);
                    });
            });
        
        changed
    }
    
    /// Render author color controls
    fn render_author_colors(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        
        CollapsingHeader::new("👤 Author Colors")
            .default_open(false)
            .show(ui, |ui| {
                Grid::new("author_colors_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        changed |= color_picker_standalone(ui, "User (You)", &mut self.colors.user_color);
                        changed |= color_picker_standalone(ui, "Agent (Sagitta Code)", &mut self.colors.agent_color);
                        changed |= color_picker_standalone(ui, "System", &mut self.colors.system_color);
                        changed |= color_picker_standalone(ui, "Tool", &mut self.colors.tool_color);
                    });
            });
        
        changed
    }
    
    /// Render status color controls
    fn render_status_colors(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        
        CollapsingHeader::new("📊 Status Indicators")
            .default_open(false)
            .show(ui, |ui| {
                Grid::new("status_colors_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        changed |= color_picker_standalone(ui, "Streaming", &mut self.colors.streaming_color);
                        changed |= color_picker_standalone(ui, "Thinking", &mut self.colors.thinking_indicator_color);
                        changed |= color_picker_standalone(ui, "Complete", &mut self.colors.complete_color);
                    });
            });
        
        changed
    }
    
    /// Render diff color controls
    fn render_diff_colors(&mut self, ui: &mut Ui) -> bool {
        let mut changed = false;
        
        CollapsingHeader::new("🔄 Diff Colors")
            .default_open(false)
            .show(ui, |ui| {
                Grid::new("diff_colors_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        changed |= color_picker_standalone(ui, "Added Background", &mut self.colors.diff_added_bg);
                        changed |= color_picker_standalone(ui, "Removed Background", &mut self.colors.diff_removed_bg);
                        changed |= color_picker_standalone(ui, "Added Text", &mut self.colors.diff_added_text);
                        changed |= color_picker_standalone(ui, "Removed Text", &mut self.colors.diff_removed_text);
                    });
            });
        
        changed
    }
    
    /// Render a color picker with preview
    fn color_picker(&mut self, ui: &mut Ui, label: &str, color: &mut Color32) -> bool {
        color_picker_standalone(ui, label, color)
    }

    /// Generate a random theme using smart color theory algorithms
    pub fn generate_random_theme(&mut self) {
        let mut rng = rand::thread_rng();
        
        // Choose a random theme generation algorithm
        let algorithm = rng.gen_range(0..4);
        
        match algorithm {
            0 => self.generate_golden_ratio_theme(),
            1 => self.generate_analogous_theme(),
            2 => self.generate_complementary_theme(),
            3 => self.generate_triadic_theme(),
            _ => self.generate_golden_ratio_theme(),
        }
        
        if self.preview_enabled {
            self.apply_colors();
        }
    }
    
    /// Export current theme to a JSON file
    pub fn export_theme(&self) {
        // Use native file dialog to save theme
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Sagitta Theme", &["sagitta-theme.json"])
            .set_file_name("my_theme.sagitta-theme.json")
            .save_file()
        {
            match self.export_theme_to_file(&path) {
                Ok(_) => {
                    log::info!("Theme exported successfully to: {}", path.display());
                }
                Err(e) => {
                    log::error!("Failed to export theme to {}: {}", path.display(), e);
                }
            }
        }
    }
    
    /// Import theme from a JSON file
    pub fn import_theme(&mut self) {
        // Use native file dialog to open theme
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Sagitta Theme", &["sagitta-theme.json"])
            .pick_file()
        {
            match self.import_theme_from_file(&path) {
                Ok(_) => {
                    log::info!("Theme imported successfully from: {}", path.display());
                    if self.preview_enabled {
                        self.apply_colors();
                    }
                }
                Err(e) => {
                    log::error!("Failed to import theme from {}: {}", path.display(), e);
                }
            }
        }
    }
    
    /// Export theme to a specific file path
    fn export_theme_to_file(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(&self.colors)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    
    /// Import theme from a specific file path
    fn import_theme_from_file(&mut self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let colors: CustomThemeColors = serde_json::from_str(&content)?;
        self.colors = colors;
        Ok(())
    }
    
    /// Generate a theme using the golden ratio for hue distribution
    fn generate_golden_ratio_theme(&mut self) {
        let mut rng = rand::thread_rng();
        
        // Start with a random base hue
        let base_hue = rng.gen_range(0.0..360.0);
        
        // Golden angle for optimal color distribution
        let golden_angle = 137.508;
        
        // Choose theme brightness (dark or light)
        let is_dark = rng.gen_bool(0.6); // 60% chance for dark themes
        
        // Base saturation and lightness values
        let base_saturation = if is_dark { 
            rng.gen_range(0.4..0.8) 
        } else { 
            rng.gen_range(0.3..0.7) 
        };
        
        let background_lightness = if is_dark { 
            rng.gen_range(0.05..0.15) 
        } else { 
            rng.gen_range(0.85..0.95) 
        };
        
        let text_lightness = if is_dark { 
            rng.gen_range(0.8..0.95) 
        } else { 
            rng.gen_range(0.1..0.3) 
        };
        
        // Generate colors using golden ratio distribution
        let mut hue_index = 0;
        let mut get_next_hue = || {
            let hue = (base_hue + (hue_index as f32 * golden_angle)) % 360.0;
            hue_index += 1;
            hue
        };
        
        // Background colors (low saturation, consistent lightness)
        let bg_saturation = base_saturation * 0.3;
        self.colors.panel_background = hsl_to_color32(get_next_hue(), bg_saturation, background_lightness);
        self.colors.input_background = hsl_to_color32(get_next_hue(), bg_saturation, background_lightness + if is_dark { 0.05 } else { -0.05 });
        self.colors.button_background = hsl_to_color32(get_next_hue(), bg_saturation, background_lightness + if is_dark { 0.1 } else { -0.1 });
        self.colors.code_background = hsl_to_color32(get_next_hue(), bg_saturation, background_lightness + if is_dark { 0.03 } else { -0.03 });
        self.colors.thinking_background = hsl_to_color32(get_next_hue(), bg_saturation, background_lightness + if is_dark { 0.07 } else { -0.07 });
        self.colors.tool_card_background = hsl_to_color32(get_next_hue(), bg_saturation, background_lightness + if is_dark { 0.02 } else { -0.02 });
        
        // Text colors (low saturation, high contrast)
        let text_saturation = base_saturation * 0.2;
        self.colors.text_color = hsl_to_color32(get_next_hue(), text_saturation, text_lightness);
        self.colors.hint_text_color = hsl_to_color32(get_next_hue(), text_saturation, text_lightness - if is_dark { 0.3 } else { -0.3 });
        self.colors.code_text_color = hsl_to_color32(get_next_hue(), text_saturation, text_lightness - if is_dark { 0.1 } else { -0.1 });
        self.colors.thinking_text_color = hsl_to_color32(get_next_hue(), text_saturation, text_lightness - if is_dark { 0.2 } else { -0.2 });
        self.colors.timestamp_color = hsl_to_color32(get_next_hue(), text_saturation, text_lightness - if is_dark { 0.4 } else { -0.4 });
        
        // Accent colors (higher saturation, medium lightness)
        let accent_saturation = base_saturation;
        let accent_lightness = if is_dark { 0.6 } else { 0.4 };
        self.colors.accent_color = hsl_to_color32(get_next_hue(), accent_saturation, accent_lightness);
        self.colors.success_color = hsl_to_color32(120.0, accent_saturation, accent_lightness); // Green
        self.colors.warning_color = hsl_to_color32(45.0, accent_saturation, accent_lightness); // Orange
        self.colors.error_color = hsl_to_color32(0.0, accent_saturation, accent_lightness); // Red
        
        // Border colors
        self.colors.border_color = hsl_to_color32(get_next_hue(), bg_saturation, background_lightness + if is_dark { 0.15 } else { -0.15 });
        self.colors.focus_border_color = self.colors.accent_color;
        
        // Button states
        self.colors.button_hover_color = hsl_to_color32(get_next_hue(), bg_saturation, background_lightness + if is_dark { 0.15 } else { -0.15 });
        self.colors.button_disabled_color = hsl_to_color32(get_next_hue(), bg_saturation * 0.5, background_lightness + if is_dark { 0.05 } else { -0.05 });
        self.colors.button_text_color = self.colors.text_color;
        self.colors.button_disabled_text_color = self.colors.hint_text_color;
        
        // Author colors (distinct hues with good contrast)
        self.colors.user_color = hsl_to_color32(get_next_hue(), accent_saturation * 0.8, accent_lightness);
        self.colors.agent_color = hsl_to_color32(get_next_hue(), accent_saturation * 0.8, accent_lightness);
        self.colors.system_color = hsl_to_color32(get_next_hue(), accent_saturation * 0.8, accent_lightness);
        self.colors.tool_color = hsl_to_color32(get_next_hue(), accent_saturation * 0.8, accent_lightness);
        
        // Status indicators
        self.colors.streaming_color = hsl_to_color32(get_next_hue(), accent_saturation, accent_lightness);
        self.colors.thinking_indicator_color = hsl_to_color32(get_next_hue(), accent_saturation, accent_lightness);
        self.colors.complete_color = hsl_to_color32(get_next_hue(), accent_saturation, accent_lightness);
        
        // Diff colors (use fixed hues for consistency: green for added, red for removed)
        let diff_saturation = accent_saturation * 0.8;
        let diff_bg_lightness = if is_dark { 0.2 } else { 0.8 };
        let diff_text_lightness = if is_dark { 0.7 } else { 0.3 };
        
        self.colors.diff_added_bg = hsl_to_color32(120.0, diff_saturation, diff_bg_lightness);     // Green background
        self.colors.diff_removed_bg = hsl_to_color32(0.0, diff_saturation, diff_bg_lightness);    // Red background
        self.colors.diff_added_text = hsl_to_color32(120.0, diff_saturation, diff_text_lightness);  // Green text
        self.colors.diff_removed_text = hsl_to_color32(0.0, diff_saturation, diff_text_lightness); // Red text
    }
    
    /// Generate an analogous color scheme (adjacent colors on color wheel)
    fn generate_analogous_theme(&mut self) {
        let mut rng = rand::thread_rng();
        
        // Choose a base hue and create analogous colors within 60 degrees
        let base_hue = rng.gen_range(0.0..360.0);
        let hue_range = 60.0;
        let is_dark = rng.gen_bool(0.6);
        
        let base_saturation = if is_dark { rng.gen_range(0.5..0.8) } else { rng.gen_range(0.4..0.7) };
        let background_lightness = if is_dark { rng.gen_range(0.05..0.15) } else { rng.gen_range(0.85..0.95) };
        let text_lightness = if is_dark { rng.gen_range(0.8..0.95) } else { rng.gen_range(0.1..0.3) };
        
        let get_analogous_hue = || {
            (base_hue + rng.gen_range(-hue_range/2.0..hue_range/2.0)) % 360.0
        };
        
        // Apply analogous color scheme
        self.apply_color_scheme(get_analogous_hue, base_saturation, background_lightness, text_lightness, is_dark);
    }
    
    /// Generate a complementary color scheme (opposite colors on color wheel)
    fn generate_complementary_theme(&mut self) {
        let mut rng = rand::thread_rng();
        
        let base_hue = rng.gen_range(0.0..360.0);
        let complement_hue = (base_hue + 180.0) % 360.0;
        let is_dark = rng.gen_bool(0.6);
        
        let base_saturation = if is_dark { rng.gen_range(0.5..0.8) } else { rng.gen_range(0.4..0.7) };
        let background_lightness = if is_dark { rng.gen_range(0.05..0.15) } else { rng.gen_range(0.85..0.95) };
        let text_lightness = if is_dark { rng.gen_range(0.8..0.95) } else { rng.gen_range(0.1..0.3) };
        
        let mut use_complement = false;
        let get_complementary_hue = || {
            use_complement = !use_complement;
            if use_complement { complement_hue } else { base_hue }
        };
        
        self.apply_color_scheme(get_complementary_hue, base_saturation, background_lightness, text_lightness, is_dark);
    }
    
    /// Generate a triadic color scheme (three colors equally spaced on color wheel)
    fn generate_triadic_theme(&mut self) {
        let mut rng = rand::thread_rng();
        
        let base_hue = rng.gen_range(0.0..360.0);
        let hues = [base_hue, (base_hue + 120.0) % 360.0, (base_hue + 240.0) % 360.0];
        let is_dark = rng.gen_bool(0.6);
        
        let base_saturation = if is_dark { rng.gen_range(0.5..0.8) } else { rng.gen_range(0.4..0.7) };
        let background_lightness = if is_dark { rng.gen_range(0.05..0.15) } else { rng.gen_range(0.85..0.95) };
        let text_lightness = if is_dark { rng.gen_range(0.8..0.95) } else { rng.gen_range(0.1..0.3) };
        
        let mut hue_index = 0;
        let get_triadic_hue = || {
            let hue = hues[hue_index % 3];
            hue_index += 1;
            hue
        };
        
        self.apply_color_scheme(get_triadic_hue, base_saturation, background_lightness, text_lightness, is_dark);
    }
    
    /// Apply a color scheme using the provided hue generator
    fn apply_color_scheme<F>(&mut self, mut get_hue: F, base_saturation: f32, background_lightness: f32, text_lightness: f32, is_dark: bool)
    where
        F: FnMut() -> f32,
    {
        // Background colors (low saturation)
        let bg_saturation = base_saturation * 0.3;
        self.colors.panel_background = hsl_to_color32(get_hue(), bg_saturation, background_lightness);
        self.colors.input_background = hsl_to_color32(get_hue(), bg_saturation, background_lightness + if is_dark { 0.05 } else { -0.05 });
        self.colors.button_background = hsl_to_color32(get_hue(), bg_saturation, background_lightness + if is_dark { 0.1 } else { -0.1 });
        self.colors.code_background = hsl_to_color32(get_hue(), bg_saturation, background_lightness + if is_dark { 0.03 } else { -0.03 });
        self.colors.thinking_background = hsl_to_color32(get_hue(), bg_saturation, background_lightness + if is_dark { 0.07 } else { -0.07 });
        self.colors.tool_card_background = hsl_to_color32(get_hue(), bg_saturation, background_lightness + if is_dark { 0.02 } else { -0.02 });
        
        // Text colors (low saturation, high contrast)
        let text_saturation = base_saturation * 0.2;
        self.colors.text_color = hsl_to_color32(get_hue(), text_saturation, text_lightness);
        self.colors.hint_text_color = hsl_to_color32(get_hue(), text_saturation, text_lightness - if is_dark { 0.3 } else { -0.3 });
        self.colors.code_text_color = hsl_to_color32(get_hue(), text_saturation, text_lightness - if is_dark { 0.1 } else { -0.1 });
        self.colors.thinking_text_color = hsl_to_color32(get_hue(), text_saturation, text_lightness - if is_dark { 0.2 } else { -0.2 });
        self.colors.timestamp_color = hsl_to_color32(get_hue(), text_saturation, text_lightness - if is_dark { 0.4 } else { -0.4 });
        
        // Accent colors (higher saturation)
        let accent_saturation = base_saturation;
        let accent_lightness = if is_dark { 0.6 } else { 0.4 };
        self.colors.accent_color = hsl_to_color32(get_hue(), accent_saturation, accent_lightness);
        self.colors.success_color = hsl_to_color32(120.0, accent_saturation, accent_lightness); // Keep green for success
        self.colors.warning_color = hsl_to_color32(45.0, accent_saturation, accent_lightness); // Keep orange for warning
        self.colors.error_color = hsl_to_color32(0.0, accent_saturation, accent_lightness); // Keep red for error
        
        // Border colors
        self.colors.border_color = hsl_to_color32(get_hue(), bg_saturation, background_lightness + if is_dark { 0.15 } else { -0.15 });
        self.colors.focus_border_color = self.colors.accent_color;
        
        // Button states
        self.colors.button_hover_color = hsl_to_color32(get_hue(), bg_saturation, background_lightness + if is_dark { 0.15 } else { -0.15 });
        self.colors.button_disabled_color = hsl_to_color32(get_hue(), bg_saturation * 0.5, background_lightness + if is_dark { 0.05 } else { -0.05 });
        self.colors.button_text_color = self.colors.text_color;
        self.colors.button_disabled_text_color = self.colors.hint_text_color;
        
        // Author colors (distinct hues)
        self.colors.user_color = hsl_to_color32(get_hue(), accent_saturation * 0.8, accent_lightness);
        self.colors.agent_color = hsl_to_color32(get_hue(), accent_saturation * 0.8, accent_lightness);
        self.colors.system_color = hsl_to_color32(get_hue(), accent_saturation * 0.8, accent_lightness);
        self.colors.tool_color = hsl_to_color32(get_hue(), accent_saturation * 0.8, accent_lightness);
        
        // Status indicators
        self.colors.streaming_color = hsl_to_color32(get_hue(), accent_saturation, accent_lightness);
        self.colors.thinking_indicator_color = hsl_to_color32(get_hue(), accent_saturation, accent_lightness);
        self.colors.complete_color = hsl_to_color32(get_hue(), accent_saturation, accent_lightness);
        
        // Diff colors (use fixed hues for consistency: green for added, red for removed)
        let diff_saturation = accent_saturation * 0.8;
        let diff_bg_lightness = if is_dark { 0.2 } else { 0.8 };
        let diff_text_lightness = if is_dark { 0.7 } else { 0.3 };
        
        self.colors.diff_added_bg = hsl_to_color32(120.0, diff_saturation, diff_bg_lightness);     // Green background
        self.colors.diff_removed_bg = hsl_to_color32(0.0, diff_saturation, diff_bg_lightness);    // Red background
        self.colors.diff_added_text = hsl_to_color32(120.0, diff_saturation, diff_text_lightness);  // Green text
        self.colors.diff_removed_text = hsl_to_color32(0.0, diff_saturation, diff_text_lightness); // Red text
    }
}

/// Render a color picker with preview (standalone function to avoid borrowing issues)
fn color_picker_standalone(ui: &mut Ui, label: &str, color: &mut Color32) -> bool {
    let mut changed = false;
    
    ui.label(label);
    
    ui.horizontal(|ui| {
        // Color picker button - this will show a full color picker when clicked
        if ui.color_edit_button_srgba(color).changed() {
            changed = true;
        }
    });
    
    ui.end_row();
    changed
}

/// Convert HSL color values to Color32
/// H: Hue (0-360 degrees)
/// S: Saturation (0.0-1.0)
/// L: Lightness (0.0-1.0)
fn hsl_to_color32(h: f32, s: f32, l: f32) -> Color32 {
    let h = h % 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    
    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsl_to_color32_conversion() {
        // Test pure red
        let red = hsl_to_color32(0.0, 1.0, 0.5);
        assert_eq!(red, Color32::from_rgb(255, 0, 0));
        
        // Test pure green
        let green = hsl_to_color32(120.0, 1.0, 0.5);
        assert_eq!(green, Color32::from_rgb(0, 255, 0));
        
        // Test pure blue
        let blue = hsl_to_color32(240.0, 1.0, 0.5);
        assert_eq!(blue, Color32::from_rgb(0, 0, 255));
        
        // Test white
        let white = hsl_to_color32(0.0, 0.0, 1.0);
        assert_eq!(white, Color32::from_rgb(255, 255, 255));
        
        // Test black
        let black = hsl_to_color32(0.0, 0.0, 0.0);
        assert_eq!(black, Color32::from_rgb(0, 0, 0));
    }

    #[test]
    fn test_random_theme_generation() {
        let mut customizer = ThemeCustomizer::new();
        
        // Store original colors
        let original_colors = customizer.colors.clone();
        
        // Generate a random theme
        customizer.generate_random_theme();
        
        // Verify that colors have changed (at least some of them should be different)
        let new_colors = &customizer.colors;
        let mut changes_detected = 0;
        
        // Check if any colors have changed
        if new_colors.panel_background != original_colors.panel_background { changes_detected += 1; }
        if new_colors.text_color != original_colors.text_color { changes_detected += 1; }
        if new_colors.accent_color != original_colors.accent_color { changes_detected += 1; }
        if new_colors.button_background != original_colors.button_background { changes_detected += 1; }
        
        // At least some colors should have changed
        assert!(changes_detected > 0, "Random theme generation should change at least some colors");
        
        // Verify all colors are valid (not transparent black which would indicate an error)
        assert_ne!(new_colors.panel_background, Color32::TRANSPARENT);
        assert_ne!(new_colors.text_color, Color32::TRANSPARENT);
        assert_ne!(new_colors.accent_color, Color32::TRANSPARENT);
        assert_ne!(new_colors.button_background, Color32::TRANSPARENT);
    }

    #[test]
    fn test_multiple_random_themes_are_different() {
        let mut customizer = ThemeCustomizer::new();
        
        // Generate first random theme
        customizer.generate_random_theme();
        let first_theme = customizer.colors.clone();
        
        // Generate second random theme
        customizer.generate_random_theme();
        let second_theme = customizer.colors.clone();
        
        // The themes should be different (at least some colors should differ)
        let mut differences = 0;
        if first_theme.panel_background != second_theme.panel_background { differences += 1; }
        if first_theme.text_color != second_theme.text_color { differences += 1; }
        if first_theme.accent_color != second_theme.accent_color { differences += 1; }
        if first_theme.button_background != second_theme.button_background { differences += 1; }
        
        // Note: There's a small chance themes could be identical, but it's very unlikely
        // with proper random generation across the color space
        assert!(differences > 0, "Multiple random theme generations should produce different results");
    }

    #[test]
    fn test_theme_customizer_creation() {
        let customizer = ThemeCustomizer::new();
        
        // Verify initial state
        assert!(customizer.preview_enabled); // Default is true
        assert!(!customizer.show_individual_tests);
        
        // Verify colors are initialized to custom theme defaults
        let default_custom = get_custom_theme_colors();
        assert_eq!(customizer.colors.panel_background, default_custom.panel_background);
        assert_eq!(customizer.colors.text_color, default_custom.text_color);
    }
} 