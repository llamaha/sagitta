use anyhow::{Context, Result};
use clap::Args;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use std::io::Write;
use serde_json::json;
use log;

use sagitta_search::config::AppConfig;
use crate::cli::utils::get_active_repo_config;
use sagitta_search::fs_utils::find_files_matching_pattern;

#[derive(Args, Debug, Clone)]
pub struct SearchFileArgs {
    /// Glob pattern to search for files (e.g., "*.rs", "src/**/*.toml").
    #[arg(required = true)]
    pub pattern: String,

    /// Perform case-sensitive matching.
    #[arg(long)]
    pub case_sensitive: bool,

    /// Output results in JSON format.
    #[arg(long)]
    pub json: bool,

    /// Optional: Specify the repository name to search in (overrides active repo).
    #[arg(long)]
    pub name: Option<String>,
}

pub async fn handle_repo_search_file(
    args: SearchFileArgs,
    config: &AppConfig, // Use immutable ref for reading
) -> Result<()> {
    log::debug!("Handling repo search-file with args: {:?}", args);

    let repo_config = get_active_repo_config(config, args.name.as_deref())?;
    let search_path = &repo_config.local_path;

    let matches = find_files_matching_pattern(search_path, &args.pattern, args.case_sensitive)
        .with_context(|| format!("Failed to search for files in {}", search_path.display()))?;

    if args.json {
        let output = json!(matches);
        println!("{}", output);
    } else {
        if matches.is_empty() {
            println!("No files found matching the pattern '{}' in repository '{}'.", args.pattern, repo_config.name);
        } else {
            println!("Found {} file(s) matching '{}' in repository '{}':", matches.len(), args.pattern, repo_config.name);
            let mut stdout = StandardStream::stdout(ColorChoice::Auto);
            for path in matches {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
                writeln!(&mut stdout, "  {}", path.display())?;
            }
            stdout.reset()?;
        }
    }

    Ok(())
} 