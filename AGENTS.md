# Agent Rules & Code Requirements

Code requirements and development rules for the context-engine project.

## Environment Guidelines

- **Prefer bash commands** over PowerShell or cmd when running terminal commands
- **Always use Unix-style paths** (forward slashes `/`) in commands, documentation, and code comments
- **Read test logs instead of test command output for debugging** Test logs are located in `target/test-logs/` and contain the full trace output, while test command output may be truncated.

### Documentation Validation (MCP Server)
**Search Docs:** `mcp_docs-server_search_docs <query>`- Search agent docs by keyword or tag
**After code changes**, run validation:
- `mcp_docs-server_validate_docs` - Check agent docs
- `mcp_docs-server_check_stale_docs` - Detect stale crate docs

## Problem-Solving Approach

### Complex Tasks: Plan First, Execute Later

**For multi-file refactors, large features, or architectural changes, use the ticket system.**
See `.github/prompts/ticket-system.prompt.md` for the full CLI reference.

1. **Planning session (research-focused):**
   - Create a ticket: `ticket create --title "<task>" --state open --field component=<crate> --field risk_level=<level> --field "acceptance_criteria=<done condition>" [--body-file <plan.md>]`
   - Include objective, context, analysis, execution steps, risks, and validation in the body
   - Gather ALL context before planning
   - Ask user to clarify unknowns
   - Don't implement yet — just plan
   - Wire dependencies: `ticket link --from <this> --to <blocker> --kind depends_on`

2. **Execution session (fresh context):**
   - Load plan: `ticket get --id <uuid>` (body contains full plan doc)
   - Claim it: `ticket claim --id <uuid> --worker-id <agent>`
   - Transition: `ticket update --id <uuid> --to-state in-progress`
   - Execute steps sequentially, verify each before proceeding
   - When done: `ticket update --id <uuid> --to-state done`; record completion notes in the ticket body

**Benefits:**
- Fresh context = more tokens for code
- Complete picture before starting
- Parallel execution possible (multiple agents with leases)
- Dependency graph enforced (acyclic, queryable)
- Recoverable from failures
- User can review plan: `ticket list --state open`

**When to use:** >5 files affected, >100 lines changed, or unclear scope

**When to skip:** Simple fixes, single-file changes, clear implementation path

### When Confused: Research → Document (or Ask)

1. **Don't guess!** Search the ticket database first: `ticket search "<keyword>"`
2. **Quick research:** Read source, check docs, scan tests (10-15 min max)
3. **Still unclear? → Ask user** rather than deep rabbit holes
4. **After user clarifies:** Create a ticket capturing the finding:
   - `ticket create --title "Guide: <topic>" --state done --field component=<crate> --field workflow_stage=design --body-file <temp-doc.md>`
   - This makes it searchable by future agents
5. Key patterns and gotchas also belong in `CHEAT_SHEET.md`

### Context-First Strategy

**Gather context before coding.**

| Context Level | Includes |
|--------------|----------|
| ❌ Minimal | Error only |
| ⚠️ Basic | Error + code |
| ✓ Good | + tests/expectations |
| ✓✓ Better | + data flow |
| ✓✓✓ Best | + architecture |

**Context sources:** `ls`/`find` → docs → tests → `target/test-logs/` → `git log -p` → ask user

**When to defer to user:**
- Research taking >15 minutes without clarity
- Multiple plausible interpretations
- Contradictory expectations or evidence
- Domain-specific knowledge needed
- Architecture decisions required

## Project Structure

Multi-crate workspace for context analysis and graph traversal:

**Core crates** (in `crates/` directory):
- `crates/context-trace/` - Foundation: graph structures, paths, bidirectional tracing (see HIGH_LEVEL_GUIDE.md)
- `crates/context-search/` - Pattern matching and search with unified Response API (see HIGH_LEVEL_GUIDE.md)
- `crates/context-insert/` - Insertion via split-join architecture (see HIGH_LEVEL_GUIDE.md)
- `crates/context-read/` - Context reading and expansion

**Tools** (in `tools/` directory):
- `tools/doc-viewer/` - Documentation viewer with HTTP API and MCP support
- `tools/log-viewer/` - Log viewer for tracing logs with JQ query support
- `tools/viewer-api/` - Shared server infrastructure for viewer tools

**Architecture:** trace → search → insert → read (each layer builds on previous)

**Quick API Reference:** See CHEAT_SHEET.md for types, common patterns, and gotchas

## Documentation Resources

**Priority order:**
1. **`CHEAT_SHEET.md`** - Types, patterns, gotchas (START HERE)
2. **`crates/<crate>/agents/docs/`** - API documentation (via MCP tools)
3. `crates/<crate>/HIGH_LEVEL_GUIDE.md` - Concepts, design
4. `crates/<crate>/README.md` - Purpose, API overview
5. `crates/<crate>/src/tests/` - Usage examples
6. `QUESTIONS_FOR_AUTHOR.md` - Unclear topics
7. `cargo doc --package <crate>` - Generated docs
8. `ticket search "<keywords>"` - Search all tracked issues, plans, and bugs

## Testing & Debugging

### Tracing Setup (IMPORTANT) Enable test log files in target/test-logs/:
```rust
let _tracing = init_test_tracing!(&graph);  // Pass graph for readable tokens in output!
```
### Test Commands
```bash
cargo test -p <crate> [test_name] -- --nocapture
LOG_STDOUT=1 LOG_FILTER=trace cargo test -p <crate> -- --nocapture
```

### Debug Workflow
1. Run: `LOG_STDOUT=1 LOG_FILTER=trace cargo test <name> -- --nocapture`
2. Check: `target/test-logs/<name>.log`
3. Track: data flow, control flow, ownership, dependencies

## Bug Reports

File bugs as tickets:
```bash
ticket create --title "Bug: <component> — <symptom>" --state open \
  --field component=<crate> --field risk_level=<high|medium|low> \
  --field "acceptance_criteria=Root cause identified; fix verified; regression test added" \
  [--body-file <analysis.md>]
```

Search known issues: `ticket search "<symptom>"` or `ticket list --state open`

See `.github/prompts/ticket-system.prompt.md` for the full bug workflow.

## Agentic Workflow

All task tracking, planning, bug reports, and design decisions live in the ticket database.
See `.github/prompts/ticket-system.prompt.md` for the complete workflow.

**Quick decision tree:**
- Confused? → `ticket search "<keyword>"` → Read source/tests 10-15 min → Still unclear? Ask user → capture finding as a `done` ticket with body
- Large task (>5 files)? → `ticket create --title "Plan: ..."` with body doc → wire deps → execute → `ticket update --to-state done`
- Found bug? → `ticket create --title "Bug: ..."` → fix → validate → `done`
- Feature done? → `ticket update --id <uuid> --to-state done`

**Scratch work** goes in a local temp file; never commit ephemeral notes.
**Key patterns/gotchas** go in `CHEAT_SHEET.md`.

## Key Docs by Crate

- **crates/context-trace/:** HIGH_LEVEL_GUIDE.md (graph, paths, tracing, cache)
- **crates/context-search/:** HIGH_LEVEL_GUIDE.md (search, Response API, patterns)
- **crates/context-insert/:** HIGH_LEVEL_GUIDE.md (split-join, InitInterval, insertion)
- **Root:** README.md, CHEAT_SHEET.md, QUESTIONS_FOR_AUTHOR.md