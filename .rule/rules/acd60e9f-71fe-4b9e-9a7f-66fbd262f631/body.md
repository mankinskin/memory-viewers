## Feedback Workflow

- Record feedback in the entity feedback store today. Use the canonical entity URN for the target, for example `ce://default/rule/<rule-id>`, `ce://default/spec/<spec-id>`, or `ce://default/ticket/<ticket-id>`.
- When feedback came from a specific generated instruction surface, first locate the canonical rule entry that produced that text so the feedback can target the real rule entity instead of only the generated file path.
- Record or inspect feedback with the feedback-api transports:
  - CLI: `feedback ingest|inbox|summary --store-root <path-to-.feedback> --workspace-slug <slug> --target <ce://...>` with `--source`, optional `--rating`, `--note`, `--note-kind`, `--session-id`, and `--author` on `ingest`.
  - MCP: `feedback_ingest`, `feedback_inbox` or `feedback_query`, `feedback_summary`, and `feedback_mine`.
- Use `feedback_summary` or `feedback summary` when you need the current low-signal state for an entity; use `feedback_inbox` or `feedback inbox` when you need the raw stored entries that explain why follow-up is needed.
- If feedback implies a contract or workflow change, open or update the corresponding spec or ticket and link the exact entity that received the feedback instead of leaving the signal stranded in chat only.