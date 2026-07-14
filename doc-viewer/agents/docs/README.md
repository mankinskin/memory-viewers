# Doc Viewer

Documentation viewer server for the context-engine project.

## Overview

This server provides tools for two types of documentation:

1. **Agent Documentation** (`agents/` directory)
   - Guides, plans, bug reports, implemented features, analysis documents
   - Uses YAML frontmatter for metadata
   - Auto-generates INDEX.md files

2. **Crate API Documentation** (`crates/*/agents/docs/`)
   - Documents crate modules, types, traits, and macros
   - Uses index.yaml files for structured metadata
   - Supports staleness detection via git history

## Architecture

```
doc-viewer/
├── src/
│   ├── main.rs       # HTTP/MCP server entry point
│   ├── tools/        # DocsManager, CrateDocsManager
│   ├── schema.rs     # Data types, metadata structures
│   ├── parser.rs     # Document parsing utilities
│   ├── templates.rs  # Document generation templates
│   └── git.rs        # Git integration for staleness
├── frontend/         # Preact web application
└── agents/docs/      # This documentation
```

## Key Components

### DocsManager

Handles agent documentation in `agents/` directories:

- `create_document()` - Create new docs from templates
- `list_documents()` - List documents by type
- `search_docs()` - Search document metadata and content
- `validate()` - Check documentation health
- `update_index()` - Regenerate INDEX.md files

### CrateDocsManager

Handles crate API documentation in `crates/*/agents/docs/`:

- `discover_crates()` - Find documented crates
- `browse_crate()` - Browse module tree
- `read_crate_doc()` / `update_crate_doc()` - Read/write documentation
- `check_stale_docs()` - Detect outdated documentation
- `sync_crate_docs()` - Analyze source for sync suggestions

## MCP Tools

### Agent Documentation Tools
- `create_doc` - Create new document from template
- `list_docs` - List documents by type/tag/status
- `read_doc` - Read document with configurable detail
- `update_doc_meta` - Update document metadata
- `search_docs` - Search document metadata
- `search_content` - Search within document content
- `browse_docs` - Browse documentation structure
- `validate_docs` - Check for issues
- `regenerate_index` - Rebuild INDEX.md
- `get_docs_needing_review` - Find old documents
- `add_frontmatter` - Add missing frontmatter
- `health_dashboard` - Overall health metrics

### Crate Documentation Tools
- `list_crates` - List all documented crates
- `browse_crate` - Browse crate module tree
- `read_crate_doc` - Read crate/module docs
- `update_crate_doc` - Update crate/module docs
- `create_module_doc` - Create new module docs
- `search_crate_docs` - Search crate documentation
- `validate_crate_docs` - Validate structure
- `check_stale_docs` - Check for stale documentation
- `sync_crate_docs` - Analyze source for sync

## Usage

The server is started automatically via MCP configuration. Tools are invoked through MCP protocol messages.

## Development

```bash
# Check compilation
cargo check

# Run tests (currently none)
cargo test

# Build release
cargo build --release
```
