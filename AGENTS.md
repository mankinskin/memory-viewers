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

**For multi-file refactors, large features, or architectural changes:**

1. **Planning session (research-focused):**
   - Create `agents/plans/PLAN_<task_name>.md` (use template in `agents/plans/`)
   - Include: Objective, Context, Analysis, Execution steps, Risks, Validation
   - Gather ALL context before planning
   - Ask user to clarify unknowns
   - Don't implement yet - just plan

2. **Execution session (fresh context):**
   - Load plan from `agents/plans/`
   - Execute steps sequentially
   - Verify each step before proceeding
   - Update plan if deviations needed
   - Mark completed items
   - When done: Create summary in `agents/implemented/`, update INDEX.md, keep or archive plan

**Benefits:**
- Fresh context = more tokens for code
- Complete picture before starting
- Parallel execution possible (multiple agents)
- Recoverable from failures
- User can review plan before execution

**When to use:** >5 files affected, >100 lines changed, or unclear scope

**When to skip:** Simple fixes, single-file changes, clear implementation path

### When Confused: Research → Document (or Ask)

1. **Don't guess!** Check `agents/guides/INDEX.md` by tags first
2. **Quick research:** Read source, check docs, scan tests (10-15 min max)
3. **Still unclear? → Ask user** rather than deep rabbit holes
4. **After user clarifies:** Document in `agents/guides/<TOPIC>_GUIDE.md`:
   - Problem description + root cause
   - Correct/incorrect examples
   - Common mistakes + fixes
   - Related files + migration checklist
5. **Update index:** Add to `agents/guides/INDEX.md` (or note "TODO" if urgent)

**When to defer to user:**
- Research taking >15 minutes without clarity
- Multiple plausible interpretations
- Contradictory expectations or evidence
- Domain-specific knowledge needed
- Architecture decisions required

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

**Red flags:** Repeated failures, unclear values, uncertain ownership, many unknown refs

## Project Structure

Multi-crate workspace for context analysis and graph traversal:

**Core crates** (in `crates/` directory):
- `crates/context-trace/` - Foundation: graph structures, paths, bidirectional tracing (see HIGH_LEVEL_GUIDE.md)
- `crates/context-search/` - Pattern matching and search with unified Response API (see HIGH_LEVEL_GUIDE.md)
- `crates/context-insert/` - Insertion via split-join architecture (see HIGH_LEVEL_GUIDE.md)
- `crates/context-read/` - Context reading and expansion

**Tools** (in `tools/` directory):
- `tools/doc-viewer/` - Documentation viewer with HTTP API and MCP support (has its own `agents/docs/`)
- `tools/log-viewer/` - Log viewer for tracing logs with JQ query support
- `tools/viewer-api/` - Shared server infrastructure for viewer tools

**Architecture:** trace → search → insert → read (each layer builds on previous)

**Quick API Reference:** See CHEAT_SHEET.md for types, common patterns, and gotchas

## Documentation Resources

**Priority order:**
1. **`CHEAT_SHEET.md`** - Types, patterns, gotchas (START HERE)
2. **`agents/guides/INDEX.md`** - How-to guides by topic
3. **`crates/<crate>/agents/docs/`** - API documentation (via MCP tools)
4. `crates/<crate>/HIGH_LEVEL_GUIDE.md` - Concepts, design
5. `crates/<crate>/README.md` - Purpose, API overview
6. `crates/<crate>/src/tests/` - Usage examples
7. `agents/bug-reports/INDEX.md` - Known issues
8. `QUESTIONS_FOR_AUTHOR.md` - Unclear topics
9. `cargo doc --package <crate>` - Generated docs

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

Check `agents/bug-reports/INDEX.md` before investigating.

**Format:** `agents/bug-reports/YYYYMMDD_BUG_<component>_<desc>.md` with: Summary, Root Cause, Evidence, Fix Options, Related Files

**After creating:** Add entry to `agents/bug-reports/INDEX.md` with tags and summary

**After fixing:** Update guides in `agents/guides/`, archive/update bug report in INDEX.md, document pattern

## Agentic Workflow Organization

**Use `agents/` directory to keep repository organized. See `agents/README.md` for complete guide.**

**Directory structure:**
- `agents/guides/` - How-to guides and troubleshooting (commit these)
- `agents/plans/` - Task plans before execution (commit active plans)
- `agents/implemented/` - Completed feature documentation (commit these)
- `agents/bug-reports/` - Known issues and analyses (commit these)
- `agents/analysis/` - Algorithm analysis and comparisons (commit these)
- `agents/tmp/` - Temporary scratch files (never commit)

**File naming convention (CRITICAL):**
All agent-generated files MUST include a timestamp prefix for chronological ordering:
- Format: `YYYYMMDD_<FILENAME>.md` (e.g., `20260216_FEATURE_NAME.md`)
- Lists newest files first when sorted alphabetically
- Makes file age immediately visible
- Enables easy tracking of document history

**Quick decision tree:**
- Confused? → Check `agents/guides/INDEX.md` → Research 10-15min → Still unclear? Ask user → Document in `agents/guides/` + **update INDEX.md**
- Large task (>5 files)? → Create plan in `agents/plans/` → Execute later → Summary in `agents/implemented/` + **update INDEX.md**
- Found bug? → Document in `agents/bug-reports/` + **update INDEX.md** → After fix, update `agents/guides/`
- Feature done? → Write summary in `agents/implemented/` + **update INDEX.md**

**Move findings from tmp/ to:** CHEAT_SHEET.md (patterns) | HIGH_LEVEL_GUIDE.md (concepts) | `agents/guides/` (how-tos) | QUESTIONS_FOR_AUTHOR.md (questions)

**Full documentation:** `agents/README.md` - Master index with when-to-use guide for each directory

## Key Docs by Crate

- **crates/context-trace/:** HIGH_LEVEL_GUIDE.md (graph, paths, tracing, cache)
- **crates/context-search/:** HIGH_LEVEL_GUIDE.md (search, Response API, patterns)
- **crates/context-insert/:** HIGH_LEVEL_GUIDE.md (split-join, InitInterval, insertion)
- **Root:** README.md, CHEAT_SHEET.md, QUESTIONS_FOR_AUTHOR.md, `agents/README.md`