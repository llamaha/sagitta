# Project Instructions for Claude Code

This file contains important guidelines for working with this codebase. Please follow these instructions when making changes.

## Code Understanding and Navigation

- **Always use the code search/query MCP tool** to understand the codebase at the beginning.
- **IMPORTANT: Use the `elementType` and `lang` parameters** to get more precise search results:
  - `elementType`: Filter by code constructs (function, class, struct, method, interface, etc.)
  - `lang`: Filter by programming language (rust, python, javascript, go, etc.)
  - These parameters dramatically improve search precision and reduce noise
- Examples of effective queries:
  - Finding auth functions: `query="authentication", elementType="function", lang="rust"`
  - Finding Python classes: `query="database model", elementType="class", lang="python"`
  - Finding Go interfaces: `query="handler interface", elementType="interface", lang="go"`
- Query for similar implementations when adding new features
- Search for existing patterns before creating new ones
- If running "cargo build" ALWAYS use "cargo build --release --all --features cuda".  Otherwise you break the semantic search features and we can't continue.

## Dependencies and Libraries

When adding new dependencies:

1. **Web search for the official Git project URL** to get the correct repository information
2. **Use the repository add tool** to add the dependency's source code for analysis
3. **Use --target-ref or specific branch** to match the exact version you're using
4. **Use repository_add_dependency tool** to link the dependency to the main project
5. **Query the added dependency repository** to understand implementation patterns and APIs
6. **Sync repositories** after adding dependencies to keep indexed data current

### Managing Repository Dependencies

Each repository can have dependencies on other repositories in the system. Use these tools:

- **repository_add_dependency**: Link a repository as a dependency to another
- **repository_remove_dependency**: Remove a dependency link
- **repository_list_dependencies**: List all dependencies for a repository

This approach ensures you understand the actual implementation rather than outdated documentation.

## Research and Documentation

- **Web search for official documentation** when you need to understand APIs or features
- Use multiple sources to verify implementation approaches
- Check for updates and best practices in official documentation

## Version Control Workflow

After making changes:

1. **Create a git commit** with a concise one liner commit message that reflects the changes
2. **Execute the repository sync tool** to update the indexed data for future queries
3. Keep commits focused and atomic for better tracking

## General Guidelines

- Follow existing code patterns and conventions in the project
- Test changes thoroughly before committing
- Document complex implementations for future reference
- Keep the codebase consistent with established patterns

## Tests

- To test everything use `cargo test --release --all --features cuda`.  
- You can use variations of this with `cargo clippy`, `cargo check`, `cargo build` etc.

---

*This file was automatically created by Sagitta Code to provide Claude with project-specific guidance.*
