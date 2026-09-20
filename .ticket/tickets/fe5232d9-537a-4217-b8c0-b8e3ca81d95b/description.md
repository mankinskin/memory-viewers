# Problem

Browser-hosted frontend guidance requires Playwright coverage, but it does not explicitly tell agents to try the MCP Playwright/browser tools first before falling back to repo-local wrappers or manual browser steps.

## Goal

Update the testing instructions so browser-hosted frontend work consistently prefers MCP Playwright/browser tooling when it is available.

## Acceptance criteria

- The frontend-specific testing instructions say to try MCP Playwright/browser tools first for browser-hosted frontend validation.
- The shared AGENTS quality-gate guidance says the same for Playwright-based browser validation.
- The wording still preserves the requirement to use an external Chromium-family browser for manual visual checks when manual verification is needed.

## Implementation status

- Updated the canonical rule sources for the shared AGENTS quality-gate guidance and frontend validation guidance.
- Regenerated the AGENTS and frontend instruction outputs from those updated rule entries.

## Validation status

- `get_errors` on the touched instruction files returned no errors.
- Regenerated targets with `./target/debug/rule.exe sync-targets --config rule-targets.yaml --json` and `./target/debug/rule.exe sync-targets --config memory-viewers/rule-targets.yaml --json`.
- Reviewed the focused diff for the instruction-only changes.