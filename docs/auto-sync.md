# Auto-Sync Functionality

The vectordb-cli tool provides an auto-sync feature that automatically updates your vector database when changes are detected in your repositories. This is particularly useful for keeping your semantic search index up-to-date with your evolving codebase.

## How Auto-Sync Works

When enabled, the auto-sync daemon runs in the background and monitors your repositories for changes. When it detects a change (such as new commits), it automatically indexes the changed files, keeping your vector database in sync with your code.

## Using Auto-Sync

### Prerequisites

Before using auto-sync, you need to have at least one repository added to vectordb-cli. You can add a repository using:

```bash
vectordb-cli repo add /path/to/your/repo
```

### Enabling Auto-Sync for a Repository

To enable auto-sync for a repository:

```bash
vectordb-cli repo auto-sync enable <REPO_NAME_OR_ID>
```

Optional: Specify a custom sync interval in seconds (default is 60 seconds):

```bash
vectordb-cli repo auto-sync enable <REPO_NAME_OR_ID> --interval 300
```

### Starting the Auto-Sync Daemon

After enabling auto-sync for one or more repositories, start the daemon:

```bash
vectordb-cli repo auto-sync start
```

The daemon will run in the background and monitor your repositories for changes.

### Checking Auto-Sync Status

To check the auto-sync status for all repositories:

```bash
vectordb-cli repo auto-sync status
```

To check the status for a specific repository:

```bash
vectordb-cli repo auto-sync status --repo <REPO_NAME_OR_ID>
```

### Disabling Auto-Sync

To disable auto-sync for a repository:

```bash
vectordb-cli repo auto-sync disable <REPO_NAME_OR_ID>
```

### Stopping the Auto-Sync Daemon

To stop the auto-sync daemon:

```bash
vectordb-cli repo auto-sync stop
```

## Best Practices

1. **Set an appropriate interval**: Choose an interval that balances keeping your index up-to-date vs. system resources. For frequently changing repositories, a shorter interval might be appropriate. For less active repositories, a longer interval (300 seconds or more) might be better.

2. **Start auto-sync automatically**: For long-running projects, you might want to start the auto-sync daemon as part of your development environment setup.

3. **Stop when not needed**: If you're not actively using the vector database for searches, consider stopping the auto-sync daemon to save system resources.

## Troubleshooting

- If auto-sync isn't detecting changes, make sure the daemon is running by checking `vectordb-cli repo auto-sync status`
- Verify that git is properly installed and that the repository path is correct
- Check the logs for any errors (you can run with `RUST_LOG=debug` for detailed logs)

## Advanced Uses

The auto-sync feature can be particularly useful in CI/CD pipelines or developer environments where you want to maintain an up-to-date semantic search index without manual intervention. 