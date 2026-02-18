# GitHub Copilot Instructions

> **📖 READ `AGENTS.md` FIRST** - This file only contains Copilot-specific additions.
>
> All core guidelines (documentation maintenance, problem-solving, project structure, testing, etc.) are in the root `AGENTS.md` file.

## Copilot-Specific Features

### MCP Docs Server Integration

**To use the MCP docs server with Copilot CLI:**
```bash
copilot --additional-mcp-config @.github/copilot-mcp-config.json
```

**Research documentation before implementation:**
- Use `mcp_docs-server_list_docs` to discover available documentation
- Use `mcp_docs-server_search_docs` or `mcp_docs-server_search_content` to find relevant guides
- Use `mcp_docs-server_read_doc` to read specific documentation files
- Use `mcp_docs-server_list_crates` and `mcp_docs-server_browse_crate` for crate API docs
- Always check documentation for algorithms, patterns, and expected behaviors before implementing or fixing tests

### Automated Hooks

When using `copilot` CLI from this directory, hooks in `.github/hooks/` provide:
- **PostToolUse:** Reminds you to validate docs after editing MCP server source files
- **Stop:** Warns about uncommitted MCP server changes before session ends
