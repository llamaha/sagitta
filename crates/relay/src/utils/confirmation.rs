use crate::utils::error::{RelayError, Result};
use std::io::{self, Write};
use tracing::warn;

/// Prompts the user with a message and waits for confirmation (y/n).
///
/// Returns:
/// - `Ok(true)` if the user enters 'y' or 'Y'.
/// - `Ok(false)` if the user enters anything else.
/// - `Err(RelayError::IoError)` if there's an error reading from stdin or flushing stdout.
pub fn prompt_user_confirmation(prompt_message: &str) -> Result<bool> {
    print!("{}", prompt_message);
    io::stdout().flush().map_err(RelayError::IoError)?; 

    let mut user_input = String::new();
    io::stdin().read_line(&mut user_input).map_err(RelayError::IoError)?;

    if user_input.trim().to_lowercase() == "y" {
        Ok(true)
    } else {
        warn!(user_input = %user_input.trim(), "User denied confirmation or provided invalid input.");
        Ok(false) 
    }
} 