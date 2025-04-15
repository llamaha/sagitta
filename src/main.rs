fn main() -> Result<()> {
    // ... setup ...

    // Load config using default path
    let mut config = config::load_config(None).context("Failed to load configuration")?;

    // ... rest of main ...

    // No top-level save needed here as commands save themselves
    // config::save_config(&config, None).context("Failed to save configuration")?

    Ok(())
} 