pub mod app;
pub mod chat;
pub mod settings;
pub mod repository;
pub mod theme;
pub mod theme_customizer;
pub mod conversation;
pub mod symbols;
pub mod fonts;
pub mod progress;
pub mod claude_md_modal;

pub use conversation::*;

// Add a placeholder for EditableLabel if it's truly gone and not just un-imported
// This will allow compilation if it's used elsewhere, though it won't function.
// Alternatively, if it's defined in a common components crate that sagitta-code should depend on,
// that dependency should be added.

// Placeholder:
// pub struct EditableLabel<'a> {
//     text: &'a mut String,
// }
// impl<'a> EditableLabel<'a> {
//     pub fn new(text: &'a mut String) -> Self {
//         Self { text }
//     }
// }
// impl<'a> egui::Widget for EditableLabel<'a> {
//     fn ui(self, ui: &mut egui::Ui) -> egui::Response {
//         ui.text_edit_singleline(self.text)
//     }
// }

