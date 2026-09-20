Hand-author the Dioxus skill — the one true ecosystem gap (best skills.sh result ~71 installs).

Anchor spec: agents/skill-infrastructure (a9b7ef39) — AC4. Depends on the contract ticket.

Required coverage (all 5 areas):
- Signals/state + component patterns.
- Server functions + fullstack data flow.
- WASM build/bundle + trunk/dx toolchain.
- Integration with this repo's viewer-api managed viewers.
- Styling/asset handling.

Acceptance criteria (verifiable):
- AC-1: `.agents/skills/dioxus/SKILL.md` exists in the contract shape with a description trigger.
- AC-2: Each of the 5 coverage areas has a dedicated section.
- AC-3: At least one worked, compiling example (or a reference to a real example under memory-viewers/*/frontend/dioxus).
- AC-4: Listed in the master skills index.