use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::*;
use git2::Repository;
use qdrant_client::Qdrant;
use std::{fs, path::PathBuf, sync::Arc, collections::HashMap};

use crate::config::{self, AppConfig};
use crate::cli::repo_commands::helpers;

#[derive(Args, Debug)]
#[derive(Clone)]
pub struct AddRepoArgs {
    #[arg(short = 'p', long, value_parser)]
    /// Local path where the repository should be cloned or already exists.
    pub local_path: Option<PathBuf>,

    #[clap(long, value_parser)]
    /// URL of the repository to clone (e.g., https://gitlab.com/user/repo.git)
    pub url: String,

    /// Optional custom name for the repository (defaults to deriving from URL).
    #[arg(short, long)]
    pub name: Option<String>,

    /// Optional specific branch to track initially (defaults to the repo's default).
    #[arg(short, long)]
    pub branch: Option<String>,

    /// Optional name for the Git remote (defaults to "origin").
    #[arg(short = 'r', long)]
    pub remote: Option<String>,

    /// Optional path to the SSH private key for authentication.
    #[arg(long)]
    pub ssh_key: Option<PathBuf>,

    /// Optional passphrase for the SSH private key.
    #[arg(long, requires = "ssh_key")] // Passphrase only makes sense if key is provided
    pub ssh_passphrase: Option<String>,
}

pub async fn handle_repo_add(
    args: AddRepoArgs,
    config: &mut AppConfig,
    client: Arc<Qdrant>,
    override_path: Option<&PathBuf>,
) -> Result<()> {
    let repo_name = match args.name {
        Some(name) => name,
        None => PathBuf::from(&args.url)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim_end_matches(".git").to_string())
            .ok_or_else(|| anyhow!("Could not derive repository name from URL"))?,
    };

    if config.repositories.iter().any(|r| r.name == repo_name) {
        bail!("Repository '{}' already exists.", repo_name);
    }

    let repo_base_path = config::get_repo_base_path()?;
    fs::create_dir_all(&repo_base_path)
        .with_context(|| format!("Failed to create repository base directory at {}", repo_base_path.display()))?;
    let local_path = args.local_path.unwrap_or(repo_base_path.join(&repo_name));

    let repo = if local_path.exists() {
         println!(
            "{}",
            format!(
                "Local directory '{}' already exists. Assuming it's the intended clone. Skipping clone.",
                local_path.display()
            ).yellow()
        );
        Repository::open(&local_path)
            .with_context(|| format!("Failed to open existing repository at {}", local_path.display()))?
    } else {
        println!("Cloning repository '{}' from {}...", repo_name.cyan(), args.url.cyan());
        let repo_configs_clone = config.repositories.clone();
        let fetch_opts = helpers::create_fetch_options(repo_configs_clone, &args.url)?;

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        let repo = builder.clone(&args.url, &local_path)
            .with_context(|| format!("Failed to clone repository from {}", args.url))?;
        println!("Repository cloned successfully to {}", local_path.display());
        repo
    };

    let initial_branch_name = match args.branch {
        Some(branch_name) => branch_name,
        None => {
            let head_ref = repo.find_reference("HEAD")?;
            let head_ref_resolved = head_ref.resolve()?;
            head_ref_resolved.shorthand()
                .ok_or_else(|| anyhow!("Could not determine default branch name from HEAD"))?
                .to_string()
        }
    };
    println!("Default/Initial branch detected: {}", initial_branch_name.cyan());


    let collection_name = helpers::get_collection_name(&repo_name);
    println!("Ensuring Qdrant collection '{}' exists...", collection_name.cyan());
    helpers::ensure_repository_collection_exists(&client, &collection_name).await?;
    println!("Qdrant collection ensured.");

    let new_repo_config = config::RepositoryConfig {
        name: repo_name.clone(),
        url: args.url.clone(),
        local_path: local_path.clone(),
        default_branch: initial_branch_name.clone(),
        tracked_branches: vec![initial_branch_name.clone()],
        active_branch: Some(initial_branch_name.clone()),
        remote_name: Some(args.remote.unwrap_or_else(|| "origin".to_string())),
        ssh_key_path: args.ssh_key.clone(),
        ssh_key_passphrase: args.ssh_passphrase.clone(),
        last_synced_commits: HashMap::new(),
        indexed_languages: None,
    };

    config.repositories.push(new_repo_config);
    config.active_repository = Some(repo_name.clone());
    config::save_config(config, override_path)?;

    println!("Set '{}' as the active repository.", repo_name.cyan());
    println!("{}", "Successfully added repository configuration.".green());
    println!("Run '{}' to fetch and index the '{}' branch.",
             format!("vectordb-cli repo sync {}", repo_name).cyan(),
             initial_branch_name.cyan());


    Ok(())
} 