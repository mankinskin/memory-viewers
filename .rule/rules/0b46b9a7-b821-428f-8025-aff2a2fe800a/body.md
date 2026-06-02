# memory-viewers

memory-viewers is the top-level repository for the user-facing viewer tools and the nested toolchain they depend on.

Direct child READMEs:

- [memory-api/README.md](memory-api/README.md) for the rule, spec, ticket, and audit automation surfaces.
- [viewer-api/README.md](viewer-api/README.md) for `viewer-ctl`, the shared viewer runtime, and the frontend scaffold.

Installable content in or directly behind this repository includes the `spec-viewer` and `ticket-viewer` binaries plus the `viewer-ctl`, `rule`, `spec`, `ticket`, and `audit` command surfaces documented in those child READMEs.

## Tool Surface

| Tool | What it exposes | Direct docs |
| --- | --- | --- |
| <code>ticket&#8209;viewer</code> | Ticket board and graph views | [memory-api/README.md](memory-api/README.md) for the ticket backends and [viewer-api/README.md](viewer-api/README.md) for the shared viewer runtime. |
| <code>spec&#8209;viewer</code> | Spec browsing UI | [memory-api/README.md](memory-api/README.md) for the spec backends and [viewer-api/README.md](viewer-api/README.md) for the shared viewer runtime. |
| [<code>memory&#8209;api</code>](memory-api/README.md) | CLI, MCP, HTTP, and VS Code tooling | [memory-api/README.md](memory-api/README.md) |
| [<code>viewer&#8209;api</code>](viewer-api/README.md) | Shared viewer-facing APIs | [viewer-api/README.md](viewer-api/README.md) |
