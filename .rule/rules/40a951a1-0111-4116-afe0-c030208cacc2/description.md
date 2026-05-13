## Operating Principles

- Gather context before coding. Do not guess.
- Read existing tests to infer expected behavior.
- Prefer bash commands over PowerShell/cmd.
- Use Unix-style paths (`/`) in commands and docs.
- Read test logs in `target/test-logs/` for debugging instead of relying on truncated test stdout.
- Keep scope tight: do not add extra features or broad refactors unless requested.