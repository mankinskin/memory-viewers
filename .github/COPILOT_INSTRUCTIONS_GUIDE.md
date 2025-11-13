# GitHub Copilot Custom Instructions Guide

This repository uses GitHub Copilot's custom instructions feature to provide context to AI coding agents.

## Available Instruction Files

### 1. Repository-Wide Instructions
**File:** `.github/copilot-instructions.md`
**Scope:** All requests in this repository
**Usage:** Primary onboarding document for Copilot agents
**Status:** ✅ Active (copy of `AGENTS.md`)

### 2. Path-Specific Instructions (Optional)
**Location:** `.github/instructions/*.instructions.md`
**Scope:** Files matching glob patterns specified in frontmatter
**Usage:** Crate-specific or feature-specific guidance
**Status:** Not yet implemented

Example structure:
```
.github/instructions/
  ├── trace.instructions.md       # context-trace crate specific
  ├── search.instructions.md      # context-search crate specific
  ├── insert.instructions.md      # context-insert crate specific
  └── tests.instructions.md       # Testing conventions
```

### 3. Agent Instructions (Alternative)
**File:** `AGENTS.md` (can be placed in subdirectories)
**Scope:** Nearest file in directory tree takes precedence
**Usage:** Per-directory agent guidance
**Status:** Not used (we use repository-wide instead)

### 4. Model-Specific Instructions (Alternative)
**Files:** `CLAUDE.md`, `GEMINI.md` (repository root only)
**Scope:** Specific AI model providers
**Usage:** Provider-specific optimizations
**Status:** Not used (provider-agnostic approach preferred)

## Current Setup

We use a **unified repository-wide approach**:
- Primary file: `.github/copilot-instructions.md` (copy of `AGENTS.md`)
- Layered documentation: CHEAT_SHEET.md → HIGH_LEVEL_GUIDE.md → detailed docs
- Questions tracked in: `QUESTIONS_FOR_AUTHOR.md`

## Future Enhancements

Consider adding path-specific instructions if:
- Crates have significantly different conventions
- Test files need specialized guidance
- Specific modules require detailed context

## References

- [GitHub Copilot Custom Instructions Docs](https://docs.github.com/en/copilot/customizing-copilot/adding-custom-instructions-for-github-copilot)
- [Custom Instructions Tutorial](https://docs.github.com/en/copilot/tutorials/customization-library/custom-instructions)
- [OpenAI agents.md repository](https://github.com/openai/agents.md)

## Validation

To verify Copilot is using your instructions:
1. Use Copilot Chat in VS Code or GitHub.com
2. Check the "References" section in responses
3. Look for `.github/copilot-instructions.md` in the reference list
4. Click to view the active instructions

## Enable/Disable

**In VS Code:** Settings → GitHub Copilot → Custom Instructions
**On GitHub.com:** Chat panel → Options (⋮) → "Enable/Disable custom instructions"
**For code review:** Repository Settings → Copilot → Code review → Toggle custom instructions
