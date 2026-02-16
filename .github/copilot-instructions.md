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

### Automated Hooks

When using `copilot` CLI from this directory, hooks in `.github/hooks/` provide:
- **PostToolUse:** Reminds you to validate docs after editing MCP server source files
- **Stop:** Warns about uncommitted MCP server changes before session ends
