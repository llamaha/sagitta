// Content rendering functions

use egui::{Ui, Color32, Vec2, Frame, Layout, Align, RichText, ScrollArea};
use crate::gui::theme::AppTheme;
use crate::gui::chat::types::{StreamingMessage, MessageType};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use std::cell::RefCell;

// Content rendering functions will be moved here