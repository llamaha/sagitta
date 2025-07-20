// Tool output rendering functions

use egui::{Ui, Color32, Vec2, Frame, Layout, Align, RichText, ScrollArea};
use serde_json;
use crate::gui::theme::AppTheme;
use crate::gui::chat::syntax_highlighting::{render_syntax_highlighted_code, render_code_diff};

// Tool output rendering functions will be moved here