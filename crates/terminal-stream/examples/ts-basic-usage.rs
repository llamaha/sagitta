use terminal_stream::{TerminalConfig, TerminalWidget, StreamEvent, events::LineType};
use crossbeam_channel;
use eframe::egui;
use std::time::Duration;
use uuid::Uuid;
use chrono;

/// Basic terminal streaming example
pub struct TerminalApp {
    terminal_widget: TerminalWidget,
    event_sender: crossbeam_channel::Sender<StreamEvent>,
}

impl TerminalApp {
    pub fn new() -> Self {
        // Create a channel for streaming events
        let (event_sender, event_receiver) = crossbeam_channel::unbounded();
        
        // Create terminal configuration
        let config = TerminalConfig::new()
            .with_max_lines(1000)
            .unwrap()
            .with_auto_scroll(true)
            .with_timestamps(true)
            .with_font_size(12.0)
            .unwrap();

        let terminal_widget = TerminalWidget::builder()
            .id("terminal_widget")
            .config(config.clone())
            .event_receiver(event_receiver)
            .build()
            .expect("Failed to build terminal widget");
        
        Self {
            terminal_widget,
            event_sender,
        }
    }
    
    /// Simulate adding some terminal output
    pub fn simulate_output(&self) {
        let command_id = Uuid::new_v4();
        
        // Send some example events
        let events = vec![
            StreamEvent::command("ls -la".to_string()),
            StreamEvent::stdout(Some(command_id), "total 24".to_string()),
            StreamEvent::stdout(Some(command_id), "drwxr-xr-x  4 user user 4096 Dec  1 10:00 .".to_string()),
            StreamEvent::stdout(Some(command_id), "drwxr-xr-x 12 user user 4096 Dec  1 09:30 ..".to_string()),
            StreamEvent::stdout(Some(command_id), "-rw-r--r--  1 user user  123 Dec  1 10:00 file.txt".to_string()),
            StreamEvent::system("Command completed successfully".to_string()),
        ];
        
        for event in events {
            let _ = self.event_sender.send(event);
        }
    }
    
    /// Simulate an error
    pub fn simulate_error(&self) {
        let events = vec![
            StreamEvent::command("cat non_existent_file.txt".to_string()),
            StreamEvent::stderr(None, "cat: non_existent_file.txt: No such file or directory".to_string()),
            StreamEvent::StreamError {
                message: "Command failed with exit code 1".to_string(),
                timestamp: chrono::Utc::now(),
            },
        ];
        
        for event in events {
            let _ = self.event_sender.send(event);
        }
    }
}

impl eframe::App for TerminalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Terminal Stream Example");
            
            ui.horizontal(|ui| {
                if ui.button("Simulate Output").clicked() {
                    self.simulate_output();
                }
                if ui.button("Simulate Error").clicked() {
                    self.simulate_error();
                }
                if ui.button("Clear Terminal").clicked() {
                    self.terminal_widget.clear();
                }
            });
            
            ui.separator();
            
            // Display the terminal widget
            self.terminal_widget.show(ui);
        });
        
        // Request repaint to keep the UI responsive
        ctx.request_repaint_after(Duration::from_millis(16)); // ~60 FPS
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Terminal Stream Example"),
        ..Default::default()
    };
    
    eframe::run_native(
        "Terminal Stream",
        options,
        Box::new(|_cc| Ok(Box::new(TerminalApp::new()))),
    )
} 