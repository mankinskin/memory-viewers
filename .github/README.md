# GitHub Copilot Configuration

This directory contains GitHub Copilot custom instructions for the context-engine project.

## Active Files

### Primary Agent Instructions
**`instructions.md`** - Repository-wide custom instructions (copy of `../AGENTS.md`)

This file provides:
- Documentation maintenance checklist
- Problem-solving approach (context-first strategy)
- Project structure overview
- Testing & debugging workflows
- API migration guides (Response API unification)
- Bug reporting conventions
- Temporary work file guidelines

### Supporting Documentation
**`COPILOT_INSTRUCTIONS_GUIDE.md`** - Meta-documentation about the custom instructions system
**`instructions/README.md`** - Guidance for creating path-specific instructions

## GitHub Copilot Instruction Types

GitHub Copilot supports multiple instruction file types (in priority order):

1. **Repository-wide**: `.github/instructions.md` ✅ **We use this**
   - Applied to all requests in the repository
   - Best for project-wide conventions and architecture

2. **Path-specific**: `.github/instructions/*.instructions.md`
   - Target specific files/directories with glob patterns
   - Use for crate-specific or module-specific guidance
   - Currently not implemented (may add later)

3. **Agent instructions**: `AGENTS.md` (anywhere in repo)
   - Nearest file in directory tree takes precedence
   - Alternative to repository-wide approach
   - Not used (we use repository-wide instead)

4. **Model-specific**: `CLAUDE.md` or `GEMINI.md` (root only)
   - Provider-specific optimizations
   - Not used (provider-agnostic approach preferred)

## Documentation Architecture

The custom instructions reference a layered documentation structure:

```
Priority Order (for agents):
1. CHEAT_SHEET.md                    ← Quick reference (types, patterns, gotchas)
2. <crate>/HIGH_LEVEL_GUIDE.md       ← Conceptual understanding
3. <crate>/README.md                 ← API overview
4. <crate>/DOCUMENTATION_ANALYSIS.md ← Detailed structural analysis
5. <crate>/src/tests/                ← Usage examples
6. bug-reports/                      ← Known issues
7. QUESTIONS_FOR_AUTHOR.md           ← Unclear topics
```

## Verification

To verify Copilot is using these instructions:

**In VS Code:**
- Use Copilot Chat
- Check "References" in response
- Look for `.github/instructions.md`

**On GitHub.com:**
- Use Copilot Chat or Agents
- Expand "References" at top of response
- Verify custom instructions file is listed

**For Code Review:**
- Repository Settings → Copilot → Code review
- Verify "Use custom instructions" is enabled

## Updates

When updating `AGENTS.md`, remember to sync changes:
```bash
cp AGENTS.md .github/instructions.md
git add .github/instructions.md
git commit -m "docs: update Copilot instructions"
```

Or consider making `.github/instructions.md` a symlink:
```bash
# Not done yet, but possible future approach:
cd .github
ln -s ../AGENTS.md instructions.md
```

## References

- [GitHub Copilot Custom Instructions Docs](https://docs.github.com/en/copilot/customizing-copilot/adding-custom-instructions-for-github-copilot)
- [Custom Instructions Examples](https://docs.github.com/en/copilot/tutorials/customization-library/custom-instructions)
- [OpenAI agents.md](https://github.com/openai/agents.md)
