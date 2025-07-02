# Project Instructions for Claude Code

This file contains important guidelines for working with this codebase. Please follow these instructions when making changes.

## Code Understanding and Navigation

- **Always use the code search/query MCP tool** to understand the codebase before making changes
- Use semantic search to find relevant functions, components, and patterns
- Query for similar implementations when adding new features
- Search for existing patterns before creating new ones

## Dependencies and Libraries

When adding new dependencies:

1. **Web search for the official Git project URL** to get the correct repository information
2. **Use the repository add tool** to add the dependency's source code for analysis
3. **Use --target-ref or specific branch** to match the exact version you're using
4. **Query the added dependency repository** to understand implementation patterns and APIs
5. **Sync repositories** after adding dependencies to keep indexed data current

This approach ensures you understand the actual implementation rather than outdated documentation.

## Research and Documentation

- **Web search for official documentation** when you need to understand APIs or features
- Use multiple sources to verify implementation approaches
- Check for updates and best practices in official documentation

## Version Control Workflow

Before making changes:

1. **Check out a feature branch** if you haven't already done so (avoid working directly on main/master)
2. Use descriptive branch names that reflect the work being done

After making changes:

1. **Create a git commit** with a concise one liner commit message that reflects the changes
2. **Execute the repository sync tool** to update the indexed data for future queries
3. Keep commits focused and atomic for better tracking

## General Guidelines

- Follow existing code patterns and conventions in the project
- Test changes thoroughly before committing
- Document complex implementations for future reference
- Keep the codebase consistent with established patterns

---

*This file was automatically created by Sagitta Code to provide Claude with project-specific guidance.*