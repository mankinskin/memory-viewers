# memory-viewers

memory-viewers is the top-level repository for the user-facing viewer tools and the nested toolchain they depend on.

## Tool Surface

| Tool | What it exposes | Use it when |
| --- | --- | --- |
| `ticket-viewer` | Ticket board and graph views | You want to review active work, owners, and dependency flow visually. |
| `spec-viewer` | Spec browsing UI | You want to inspect specs, sections, and linked code references. |
| `memory-api` | CLI, MCP, HTTP, and VS Code tooling | You need automation or authoring workflows behind rules, specs, tickets, and audits. |
| `viewer-api` | Shared viewer-facing APIs | You need reusable viewer integration surfaces across the stack. |
