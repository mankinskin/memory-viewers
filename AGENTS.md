# Agent Rules & Code Requirements

> **⚠️ MANDATORY: READ THIS FILE FIRST** before any code changes in this workspace.

Code requirements and development rules for the context-engine project.

> **AGENT RESPONSIBILITY:** Keep this file current. Update immediately when requirements change.
>
> **Update triggers:** Project structure changes, new test patterns, modified debugging workflows, requirement changes.

## 📚 Documentation Maintenance

**⚠️ CRITICAL: Update docs when changing code!**

### Before Work:
1. `CHEAT_SHEET.md` - API patterns
2. `<crate>/HIGH_LEVEL_GUIDE.md` - concepts
3. `QUESTIONS_FOR_AUTHOR.md` - known issues

### After Changes:
| Change Type | Update File |
|------------|-------------|
| API/types/patterns/macros | `CHEAT_SHEET.md` |
| Concepts/architecture/modules | `<crate>/HIGH_LEVEL_GUIDE.md` |
| Unclear behavior/gaps | `QUESTIONS_FOR_AUTHOR.md` |
| Test structure/workflows/commands | `AGENTS.md` (this file) |

## Problem-Solving Approach

### Complex Tasks: Plan First, Execute Later

**For multi-file refactors, large features, or architectural changes:**

1. **Planning session (research-focused):**
   - Create `agent-tmp/PLAN_<task_name>.md` with:
     - **Objective:** Clear goal statement
     - **Context:** All relevant files, dependencies, constraints
     - **Analysis:** What needs to change and why
     - **Execution steps:** Numbered, atomic, testable actions
     - **Risks:** Known issues, breaking changes, edge cases
     - **Validation:** How to verify success
   - Gather ALL context before planning
   - Ask user to clarify unknowns
   - Don't implement yet - just plan

2. **Execution session (fresh context):**
   - Load plan file
   - Execute steps sequentially
   - Verify each step before proceeding
   - Update plan if deviations needed
   - Mark completed items

**Benefits:**
- Fresh context = more tokens for code
- Complete picture before starting
- Parallel execution possible (multiple agents)
- Recoverable from failures
- User can review plan before execution

**When to use:** >5 files affected, >100 lines changed, or unclear scope

**When to skip:** Simple fixes, single-file changes, clear implementation path

### When Confused: Research → Document (or Ask)

1. **Don't guess!** Check `guides/GUIDES_INDEX.md` by tags first
2. **Quick research:** Read source, check docs, scan tests (10-15 min max)
3. **Still unclear? → Ask user** rather than deep rabbit holes
4. **After user clarifies:** Document in `guides/<TOPIC>_GUIDE.md`:
   - Problem description + root cause
   - Correct/incorrect examples
   - Common mistakes + fixes
   - Related files + migration checklist
5. **Update index:** Add to `guides/GUIDES_INDEX.md` (or note "TODO" if urgent)

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
- `context-trace` - Foundation: graph structures, paths, bidirectional tracing (see HIGH_LEVEL_GUIDE.md)
- `context-search` - Pattern matching and search with unified Response API (see HIGH_LEVEL_GUIDE.md)
- `context-insert` - Insertion via split-join architecture (see HIGH_LEVEL_GUIDE.md)
- `context-read` - Context reading and expansion

**Architecture:** trace → search → insert → read (each layer builds on previous)

**Quick API Reference:** See CHEAT_SHEET.md for types, common patterns, and gotchas

## Documentation Resources

**Priority order:**
1. **`CHEAT_SHEET.md`** - Types, patterns, gotchas (START HERE)
2. `<crate>/HIGH_LEVEL_GUIDE.md` - Concepts, design
3. `<crate>/README.md` - Purpose, API overview
4. `<crate>/src/tests/` - Usage examples
5. `bug-reports/` - Known issues
6. `QUESTIONS_FOR_AUTHOR.md` - Unclear topics
7. `cargo doc --package <crate>` - Generated docs

## Testing & Debugging

### API Changes (Important!)
- ❌ `CompleteState`/`IncompleteState` → ✅ `Response` (unified)
- `search()` returns `Result<Response, ErrorState>`
- Check `response.is_complete()` before unwrap
- `Searchable` trait in `context_search::`
- Use `init_test_tracing!()` not `init_tracing()`

### Test Commands
```bash
cargo test -p <crate> [test_name] -- --nocapture
LOG_STDOUT=1 LOG_FILTER=trace cargo test -p <crate> -- --nocapture
```

**Always append `; focus_chat`** to return focus (except background/interactive commands)

### Tracing Setup (REQUIRED)
```rust
let _tracing = init_test_tracing!(&graph);  // Pass graph for readable tokens!
```

### Debug Workflow
1. Run: `LOG_STDOUT=1 LOG_FILTER=trace cargo test <name> -- --nocapture`
2. Check: `target/test-logs/<name>.log`
3. Track: data flow, control flow, ownership, dependencies

## Bug Reports

Check `bug-reports/` before investigating.

**Format:** `BUG_<component>_<desc>.md` with: Summary, Root Cause, Evidence, Fix Options, Related Files

**After fixing:** Update guides, remove/update bug report, document pattern

## Temporary Files

Use `agent-tmp/` for work files (never commit).

**Move findings to:** CHEAT_SHEET.md (patterns) | HIGH_LEVEL_GUIDE.md (concepts) | QUESTIONS_FOR_AUTHOR.md (questions)

## Key Docs by Crate

- **context-trace:** HIGH_LEVEL_GUIDE.md (graph, paths, tracing, cache)
- **context-search:** HIGH_LEVEL_GUIDE.md (search, Response API, patterns)
- **context-insert:** HIGH_LEVEL_GUIDE.md (split-join, InitInterval, insertion)
- **Root:** README.md, CHEAT_SHEET.md, QUESTIONS_FOR_AUTHOR.md