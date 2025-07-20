// Tool rendering functions

use egui::{Ui, Color32, Vec2, Frame, Layout, Align, RichText, ScrollArea};
use std::collections::HashMap;
use crate::gui::theme::AppTheme;
use crate::gui::chat::types::{ToolCall, MessageStatus, CopyButtonState};
use crate::gui::chat::ToolCard;
use crate::gui::app::RunningToolInfo;
use crate::agent::events::ToolRunId;
use crate::gui::chat::tool_mappings::{get_human_friendly_tool_name, get_tool_icon, format_tool_parameters_for_inline};

// Tool rendering functions will be moved here