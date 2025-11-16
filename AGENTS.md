# Agent Rules & Code Requirements

> **⚠️ MANDATORY: READ THIS FILE FIRST** before any code changes in this workspace.

Code requirements and development rules for the context-engine project.

> **AGENT RESPONSIBILITY:** Keep this file current. Update immediately when requirements change.
>
> **Update triggers:** Project structure changes, new test patterns, modified debugging workflows, requirement changes.

## 📚 Documentation Maintenance

**⚠️ CRITICAL: Always review and update guidance files when making changes!**

### Before Starting Work (REQUIRED):
1. ✅ Read `CHEAT_SHEET.md` for current API patterns
2. ✅ Check relevant `<crate>/HIGH_LEVEL_GUIDE.md` for concepts
3. ✅ Review `QUESTIONS_FOR_AUTHOR.md` for known issues related to your task

### After Making Changes (MANDATORY):
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

4. ✅ **Update this AGENTS.md** if you:
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
LOG_STDOUT=1 LOG_FILTER=trace cargo test -p context-search -- --nocapture  # With detailed logging
cargo test --package context-search find_ancestor2 -- --nocapture  # Specific test
```

**IMPORTANT: Return Focus to Chat After Commands**

After running terminal commands, **always** append `&& focus_chat` to return focus to the chat window:

```bash
# Examples:
cargo test -p context-search -- --nocapture && focus_chat
LOG_STDOUT=1 cargo test -p context-search -- --nocapture && focus_chat
cargo build && focus_chat
```

The `focus_chat` command is a bash alias that triggers the custom VS Code extension to focus the Copilot Chat window. This provides a better workflow by automatically returning to the chat after command completion.

**Exception:** Do NOT use `focus_chat` for:
- Background processes (`isBackground: true`)
- Commands that are expected to keep terminal focus
- Interactive commands (grep, less, etc.)

**Important:** To see debug/tracing output in tests, you MUST set `LOG_STDOUT=1`:
```bash
# Enable tracing output to terminal
LOG_STDOUT=1 cargo test -p context-search test_name -- --nocapture

# With specific log level
LOG_STDOUT=1 LOG_FILTER=debug cargo test -p context-search -- --nocapture

# Module-specific logging
LOG_STDOUT=1 LOG_FILTER=context_search::match=debug cargo test -- --nocapture
```

Without `LOG_STDOUT=1`, tracing output only goes to `target/test-logs/<test_name>.log` files.

### Workspace-Specific Test Setup (REQUIRED)
Add to beginning of test functions:
```rust
let _tracing = init_test_tracing!();  // Enables tracing for this test
```

### Test Organization
- `context-search/src/tests/search/ancestor.rs` - Ancestor finding tests
- `context-search/src/tests/search/mod.rs` - General search tests
- Other crates: `<crate>/src/tests/`

### Debugging Requirements

**When tests fail, ALWAYS check test log files:**
- **Location**: `target/test-logs/<test_name>.log`
- **Preserved**: Automatically saved on test failure
- **Contains**: Full tracing output including debug, info, warn, error messages
- **Usage**: Essential for debugging - often contains details not visible in terminal output

**Log level configuration:**
- **Environment variable**: `LOG_FILTER=error|warn|info|debug|trace`
- **Module-specific**: `LOG_FILTER=context_search::search=trace`
- **Multiple modules**: `LOG_FILTER=context_search::search=trace,context_trace=debug`

**Required debug workflow:**
1. Run failing test with `LOG_STDOUT=1 LOG_FILTER=trace`
2. **Check log file**: `target/test-logs/<test_name>.log` for complete details
3. Track: data flow origin, control flow, component ownership, dependencies
4. Before fix: state understanding, identify gaps, explain why fix should work
5. After failure: re-examine log output, check for different underlying issue, consider different layer

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
