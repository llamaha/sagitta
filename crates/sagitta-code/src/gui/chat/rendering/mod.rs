// Module for organizing chat rendering functions

pub mod messages;
pub mod tools; 
pub mod content;
pub mod tool_outputs;

// Re-export commonly used functions (will be uncommented as functions are moved)
pub use messages::{group_consecutive_messages, render_message_group, render_single_message_content, render_thinking_content};
// pub use tools::{render_single_tool_call, render_tool_calls_compact, render_tool_card};
// pub use content::{
//     render_message_content_compact, render_text_content_compact, render_text_with_tool_links,
//     render_code_block_compact, render_mixed_content_compact, render_welcome_message
// };
// pub use tool_outputs::{
//     render_terminal_output, render_diff_output, render_file_read_output,
//     render_file_write_output, render_search_output, render_repository_output,
//     render_todo_output, render_ping_output
// };