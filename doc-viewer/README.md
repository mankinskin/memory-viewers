# Doc Viewer

A web-based documentation viewer for the context-engine project, supporting both HTTP API and MCP protocols.

## Structure

```
doc-viewer/
├── src/              # Rust server
│   ├── main.rs       # HTTP/MCP server entry point
│   ├── http.rs       # HTTP handlers
│   ├── mcp.rs        # MCP server implementation
│   └── query.rs      # JQ query engine
├── frontend/         # Preact web application
└── static/           # Built frontend assets
```

## Running

### Server Modes

```bash
# HTTP server only (default)
cargo run -- --http

# MCP server only (for AI assistants)
cargo run -- --mcp

# Both servers simultaneously
cargo run -- --http --mcp
```

### Development Mode

```bash
# Terminal 1: Start the server
cargo run

# Terminal 2: Start the frontend dev server
cd frontend
npm install
npm run dev
```

The frontend dev server runs on `http://localhost:5173` and proxies API requests to the server on port 3001.

### Production Mode

```bash
# Build the frontend
cd frontend
npm install
npm run build

# Run the server (serves built frontend from static/)
cd ..
cargo run
```

Access the app at `http://localhost:3001`.

## Installation

### Build

```bash
cd context-engine/tools/doc-viewer
cargo build --release
```

### VS Code Integration (MCP)

Add to `.vscode/mcp.json`:

```json
{
  "servers": {
    "docs": {
      "type": "stdio",
      "command": "${workspaceFolder}/context-engine/tools/doc-viewer/target/release/doc-viewer",
      "args": ["--mcp"],
      "env": {
        "AGENTS_DIR": "${workspaceFolder}/context-engine/agents"
      }
    }
  }
}
```

## Tools

### `create_doc`
Create a new document from template.

**Parameters:**
- `doc_type`: "guide", "plan", "implemented", "bug-report", or "analysis"
- `name`: Short name (becomes UPPER_SNAKE_CASE in filename)
- `title`: Human-readable title
- `summary`: One-line summary for INDEX
- `tags`: Array of tags (without #)
- `status`: (plans only) "design", "in-progress", "completed", "blocked", "superseded"

**Example:**
```json
{
  "doc_type": "guide",
  "name": "error handling",
  "title": "Error Handling Patterns",
  "summary": "Common error handling patterns in context-search",
  "tags": ["error-handling", "patterns"]
}
```

Creates: `guides/20260131_ERROR_HANDLING.md`

### `list_docs`
List all documents of a specific type.

**Parameters:**
- `doc_type`: Document category

### `update_doc_meta`
Update metadata for an existing document.

**Parameters:**
- `filename`: Document filename
- `tags`: (optional) New tags (replaces existing)
- `summary`: (optional) New summary
- `status`: (optional) New status

### `search_docs`
Search documents by tag across all categories.

**Parameters:**
- `tag`: Tag to search for

### `regenerate_index`
Rebuild INDEX.md from document frontmatter.

**Parameters:**
- `doc_type`: Document category

### `validate_docs`
Check all documents for convention compliance.

## Crate Documentation Tools

Tools for managing API documentation in `crates/<crate>/agents/docs/`:

### `list_crates`
List all crates with documentation.

**Parameters:**
- `pattern`: (optional) Filter to crates matching pattern (e.g., "context-*")

### `browse_crate`
Browse a crate's documentation structure.

**Parameters:**
- `crate_name`: Crate name (e.g., "context-trace")
- `module_path`: (optional) Specific module path (e.g., "graph/vertex")
- `detail`: "overview" (modules only) or "full" (with types)

### `read_crate_doc`
Read crate or module documentation.

**Parameters:**
- `crate_name`: Crate name
- `module_path`: (optional) Module path (empty for crate root)
- `content`: "index" (yaml), "readme" (markdown), or "both"

### `update_crate_doc`
Update crate documentation.

**Parameters:**
- `crate_name`: Crate name
- `module_path`: (optional) Module path
- `field`: Field to update (e.g., "summary", "description")
- `value`: New value

### `create_module_doc`
Create documentation for a new module.

**Parameters:**
- `crate_name`: Crate name
- `module_path`: Module path (e.g., "graph/vertex")
- `summary`: One-line description

### `search_crate_docs`
Search crate documentation by tag or text.

**Parameters:**
- `crate_name`: (optional) Search in specific crate
- `query`: Search text
- `search_in`: "names", "descriptions", or "both"

### `validate_crate_docs`
Check crate documentation for completeness.

**Parameters:**
- `crate_name`: (optional) Validate specific crate

### `check_stale_docs`
Check if documentation is stale by comparing git modification times of source files against the `last_synced` timestamp in index.yaml.

**Parameters:**
- `crate_filter`: (optional) Check only specific crate
- `stale_threshold_days`: Days after which docs are "stale" (default: 7)
- `very_stale_threshold_days`: Days after which docs are "very stale" (default: 30)

**Returns:** Report showing:
- Fresh, stale, and very stale documentation
- Which source files were modified since last sync
- Days since documentation was last synced

### `sync_crate_docs`
Analyze source files and suggest documentation updates. Parses Rust source files to find public types, traits, and macros, then compares with documentation.

**Parameters:**
- `crate_name`: Name of the crate to analyze
- `module_path`: (optional) Specific module to analyze
- `update_timestamp`: Update `last_synced` timestamp (default: false)

**Returns:** Report showing:
- Public items found in source files
- Suggested additions (items in source but not documented)
- Suggested removals (documented items not found in source)

## Stale Detection Schema

To enable stale detection, add these fields to `index.yaml`:

```yaml
# crates/context-stack/context-trace/agents/docs/index.yaml
name: context-trace
description: Graph structures and traversal
source_files:
  - src/lib.rs
  - src/graph/mod.rs
last_synced: "2026-02-15T10:30:00+00:00"
modules:
  - name: graph
    path: graph
```

| Field | Description |
|-------|-------------|
| `source_files` | List of source file paths (relative to crate root) to track |
| `last_synced` | ISO 8601 timestamp of when docs were last verified/updated |

The `check_stale_docs` tool compares `last_synced` against git modification times of `source_files` to determine staleness.

## Document Structure

All documents use YAML frontmatter:

```markdown
---
tags: `#tag1` `#tag2`
summary: One-line summary
status: 📋  <!-- plans only -->
---

# Title

## Section
...
```

## Templates

Each document type has a tailored template:

- **Guide**: Problem → Solution → Example → Common Mistakes → Related
- **Plan**: Objective → Context → Analysis → Execution Steps → Validation → Risks
- **Implemented**: Summary → Changes → API → Migration → Testing
- **Bug Report**: Symptoms → Reproduction → Root Cause → Location → Fix → Verification
- **Analysis**: Overview → Findings → Comparison → Conclusions → References
