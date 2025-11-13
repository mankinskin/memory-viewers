# Agent Development Guide

> **⚠️ READ THIS FILE FIRST** before responding to any request in this workspace.

Workspace-specific reference for the context-engine project.

> **Keep this file concise and scannable.** Remove outdated info. Focus on what's actionable.
>
> **Update this file** when project structure, test locations, or common issues change.

## 📚 Documentation Maintenance

**⚠️ CRITICAL: Always review and update guidance files when making changes!**

### Before Starting Work:
1. ✅ Read `CHEAT_SHEET.md` for current API patterns
2. ✅ Check relevant `<crate>/HIGH_LEVEL_GUIDE.md` for concepts
3. ✅ Review `QUESTIONS_FOR_AUTHOR.md` for known issues related to your task

### After Making Changes:
1. ✅ **Update `CHEAT_SHEET.md`** if you:
   - Changed public API or types
   - Fixed a common gotcha
   - Discovered new patterns or idioms
   - Changed test utilities or macros

2. ✅ **Update `<crate>/HIGH_LEVEL_GUIDE.md`** if you:
   - Modified core concepts or architecture
   - Changed module structure
   - Added/removed key types
   - Changed how operations work

3. ✅ **Update `QUESTIONS_FOR_AUTHOR.md`** if you:
   - Discovered unclear behavior (add question)
   - Found answer to existing question (move to proper docs, remove question)
   - Identified new documentation gaps

4. ✅ **Update `AGENTS.md`** if you:
   - Changed test structure or locations
   - Modified debugging workflows
   - Changed build/test commands
   - Added new crates or major components

### Documentation Checklist:
```
□ Did my changes affect public APIs? → Update CHEAT_SHEET.md
□ Did I change how something conceptually works? → Update HIGH_LEVEL_GUIDE.md
□ Did I find something confusing? → Add to QUESTIONS_FOR_AUTHOR.md
□ Did my fix resolve a known issue? → Update or remove from QUESTIONS
□ Are my code examples still valid? → Verify in all guides
□ Did I add new test patterns? → Document in relevant guides
```

**Why this matters:** Future agents (and humans) rely on these guides. Outdated documentation causes repeated mistakes and wastes time.

## Problem-Solving Approach

### Context-First Strategy

**For complex tasks: Gather context BEFORE coding.**

**Explore the workspace:**
- Use `ls`, `pwd`, `find`, `tree` to understand file structure
- Navigate directories to discover relevant files
- Check for documentation, tests, logs before modifying code
- Verify file locations rather than assuming paths

**Context levels:**
- ❌ Error only → ⚠️ Error + code → ✓ + tests/expectations → ✓✓ + data flow → ✓✓✓ + architecture

**Collect context via:**
1. File system exploration (ls, find, locate relevant directories)
2. Documentation (README, DOCUMENTATION_ANALYSIS.md, bug-reports/)
3. Test files and expectations
4. Logs (`target/test-logs/`)
5. Git history (`git log -p -- <file>`)
6. Backtracking through code/data flow
7. Ask user when: multiple interpretations, contradictory expectations, unclear intent

**Red flags (need more context):**
- Repeated similar fixes failing
- Unclear actual vs expected values
- Uncertain about component ownership or data semantics
- Many unidentified function/type references in your working context

## Project Structure

Multi-crate workspace for context analysis and graph traversal:
- `context-trace` - Foundation: graph structures, paths, bidirectional tracing (see HIGH_LEVEL_GUIDE.md)
- `context-search` - Pattern matching and search with unified Response API (see HIGH_LEVEL_GUIDE.md)
- `context-insert` - Insertion via split-join architecture (see HIGH_LEVEL_GUIDE.md)
- `context-read` - Context reading and expansion

**Architecture:** trace → search → insert → read (each layer builds on previous)

**Quick API Reference:** See CHEAT_SHEET.md for types, common patterns, and gotchas

## Documentation Resources

**Priority order when researching:**
1. **`CHEAT_SHEET.md`** - Quick reference for common operations, types, and patterns (START HERE!)
2. `<crate>/HIGH_LEVEL_GUIDE.md` - Conceptual overview and design explanations
3. `<crate>/README.md` - Purpose and API overview
4. `<crate>/DOCUMENTATION_ANALYSIS.md` - Detailed structural analysis (update when making significant changes)
5. `<crate>/src/tests/` - Usage examples and expected behavior
6. `bug-reports/` - Known issues and fix options
7. **`QUESTIONS_FOR_AUTHOR.md`** - Unclear topics needing clarification
8. `cargo doc --package <crate> --no-deps --document-private-items --open` - Generated API docs

**New Agent Resources:**
- **CHEAT_SHEET.md** - Types, patterns, gotchas, API changes, debugging tips
- **context-trace/HIGH_LEVEL_GUIDE.md** - Graph concepts, paths, tracing explained
- **context-search/HIGH_LEVEL_GUIDE.md** - Search algorithms, Response API, patterns
- **context-insert/HIGH_LEVEL_GUIDE.md** - Split-join architecture, InitInterval, insertion flow
- **QUESTIONS_FOR_AUTHOR.md** - Known gaps and questions to resolve

## Testing & Debugging

### Debug Logging Requirements

**Always use the `tracing` crate for debug output:**
- ✅ Use `tracing::debug!()`, `tracing::info!()`, `tracing::warn!()`, `tracing::error!()`
- ❌ Never use `println!()`, `eprintln!()`, or `dbg!()` for debug output
- Use `context_trace::logging` infrastructure for test setup
- Import: `use tracing::{debug, info, warn, error};`

**Example:**
```rust
use tracing::debug;

debug!("Processing item: {}", item);
debug!("cursor.atom_position: {:?}", cursor.atom_position);
```

### Terminal Commands

**Avoid redundant directory changes:**
- Commands already execute in the workspace root directory
- ❌ Don't prefix with `cd <workspace>` when already in workspace
- ✅ Use absolute paths or relative paths from workspace root
- Only use `cd` when genuinely changing to a different directory

**Example:**
```bash
# ❌ Redundant
cd c:/Users/linus_behrbohm/git/private/context-engine && cargo test

# ✅ Direct (already in workspace)
cargo test -p context-search
```

### Recent API Changes (Important!)

**Response API Unification (context-search):**
- ❌ Old: `CompleteState` / `IncompleteState` (REMOVED)
- ✅ New: Unified `Response` type with accessor methods
- Migration: See CHEAT_SHEET.md "API Changes" section for patterns

**Key changes:**
- `search()` returns `Result<Response, ErrorState>` (not bare Response)
- Check `response.is_complete()` before unwrapping
- Use `response.expect_complete().root_parent()` to get Token
- `Searchable` trait now in `context_search::` (public export)
- Test utility: `context_trace::init_test_tracing!()` not `init_tracing()`

### Test Commands
```bash
cargo test -p context-search -- --nocapture              # Run crate tests with output
RUST_LOG=trace cargo test -p context-search -- --nocapture  # With detailed logging
cargo test --package context-search find_ancestor2 -- --nocapture  # Specific test
```

### Workspace-Specific Test Setup
Add to beginning of test functions:
```rust
let _tracing = init_test_tracing!();  // Enables tracing for this test
```

### Test Organization
- `context-search/src/tests/search/ancestor.rs` - Ancestor finding tests
- `context-search/src/tests/search/mod.rs` - General search tests
- Other crates: `<crate>/src/tests/`

### Debugging
- **Failed test logs**: `target/test-logs/<test_name>.log` (preserved on failure)
- **Log levels**: `RUST_LOG=error|warn|info|debug|trace`
- **Module-specific**: `RUST_LOG=context_search::search=trace`

**Debug workflow:**
1. Track: data flow origin, control flow, component ownership, dependencies
2. Before fix: state understanding, identify gaps, explain why fix should work
3. After failure: re-examine output, check for different underlying issue, consider different layer

## Bug Reports

**Check `bug-reports/` directory** before investigating issues.

### Creating Bug Reports
Filename: `BUG_<component>_<short_description>.md`

Required sections:
- **Summary**: One-line description
- **Root Cause**: What's wrong and why
- **Evidence**: Test commands, error output, logs, code snippets
- **Fix Options**: Proposed solutions with pros/cons
- **Related Files**: Bug location and affected tests

Create reproducing test if one doesn't exist.

**After fixing a bug:**
- Update relevant guidance files (CHEAT_SHEET.md, HIGH_LEVEL_GUIDE.md)
- Remove or update the bug report
- Document the fix pattern if it might help others

## Temporary Work Files

Use `agent-tmp/` for temporary analysis and debugging files. Never commit.

Naming conventions:
- `analysis_<topic>.md`
- `test_output_<test_name>.txt`
- `debug_<component>_<issue>.log`

**Move important findings to proper documentation before cleanup:**
- Patterns → CHEAT_SHEET.md
- Concepts → HIGH_LEVEL_GUIDE.md
- Questions → QUESTIONS_FOR_AUTHOR.md
- Permanent analysis → crate DOCUMENTATION_ANALYSIS.md

## Key Documentation Files

### Context-Search
- `HIGH_LEVEL_GUIDE.md` - Search concepts, Response API, algorithms explained
- `TRACING_GUIDE.md`, `TESTING_PLAN.md`, `SEARCH_API_EXAMPLES.md`
- `agent-tmp/PATTERN_MATCHING_EXAMPLE.md`, `agent-tmp/RESULT_ARCHITECTURE_ANALYSIS.md`

### Context-Trace
- `HIGH_LEVEL_GUIDE.md` - Graph model, paths, tracing, bidirectional cache
- `TRACING_GUIDE.md`, `TRACING_IMPLEMENTATION.md`, `TEST_EXPECTATIONS.md`

### Context-Insert
- `HIGH_LEVEL_GUIDE.md` - Split-join architecture, InitInterval, insertion flow

### Other
- Root: `README.md`, `DOCUMENTATION_SUMMARY.md`, `CHEAT_SHEET.md`, `QUESTIONS_FOR_AUTHOR.md`
- `refactor-tool/` - Multiple feature guides
