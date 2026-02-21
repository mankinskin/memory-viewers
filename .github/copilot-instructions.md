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

**Consolidated CRUD-based API (6 tools):**

| Tool | Purpose | Targets |
|------|---------|---------|
| `list` | List/browse/read | agent_docs, crate_docs, crates |
| `search` | Search content | agent_docs, crate_docs, all |
| `validate` | Maintenance ops | agent_docs, crate_docs, all |
| `create` | Create docs | agent_doc, crate_module |
| `update` | Update docs | agent_doc, crate_doc, crate_index |
| `delete` | Delete docs | agent_doc, crate_module |

**Research documentation before implementation:**
- Use `mcp_doc-viewer-mcp_list` with `target: crates` to see available crates
- Use `mcp_doc-viewer-mcp_list` with `target: agent_docs` to browse guides/plans
- Use `mcp_doc-viewer-mcp_search` with `target: all` to find relevant documentation
- Use `mcp_doc-viewer-mcp_validate` to check documentation health
- Always check documentation for algorithms, patterns, and expected behaviors

### Automated Hooks

When using `copilot` CLI from this directory, hooks in `.github/hooks/` provide:
- **PostToolUse:** Reminds you to validate docs after editing MCP server source files
- **Stop:** Warns about uncommitted MCP server changes before session ends
