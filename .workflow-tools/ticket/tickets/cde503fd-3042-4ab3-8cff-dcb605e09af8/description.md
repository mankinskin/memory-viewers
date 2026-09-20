# Goal
Triage `dead_code` compiler warnings in the `memory-viewers` submodule (split off from parent `9347c9f8` mechanical pass).

# Result
Resolved the full scoped warning set for the `memory-viewers` frontend files.

## Ticket viewer (`ticket-viewer-dioxus`)
Applied focused `#[allow(dead_code)]` to intentional frontend-contract items:
- SSE payload structs in `src/sse.rs` (`SseTicket`, `UpsertPayload`, `DeletePayload`) because fields are deserialized from backend events and only selectively consumed.
- `TicketBackend::get_subgraph` in `src/api/backend.rs` because the trait surface intentionally exposes the graph fetch call even when the current UI path does not invoke it directly.
- API/DTO response containers and optional helper methods in `src/types.rs` where fields exist to mirror backend payloads, support future views, or preserve consistent transport types (`TicketsResponse`, `TicketDetailResponse`, `TicketDescriptionResponse`, `TicketFilesResponse`, `TicketAssetResponse`, `SubgraphStats`, `GraphSubgraphResponse`, `WorkflowCandidateItem::resolved_ticket_ref`, `WorkflowTreeItem::resolved_ticket_ref`, `FieldDef`, `EdgeRuleDef`, `TypeSchema`, `SchemaListResponse`, `SchemaDetailResponse`, `CreateTicketResponse`, `TicketHistoryResponse`, `BatchCommandResult`, `BatchResponse`).

## Spec viewer (`spec-viewer-dioxus`)
Applied focused `#[allow(dead_code)]` to intentional route/API/type scaffolding:
- Uncalled fetch helpers in `src/api.rs` (`search_specs`, `get_spec`, `get_tree`, `list_sections`) preserved for alternate pages and future UI flows.
- `Route::spec_detail` in `src/routes.rs` preserved as canonical route construction helper.
- `ls_key` in `src/store.rs` preserved for persisted UI-state storage.
- DTO/response types and state-color helpers in `src/types.rs` that mirror backend payloads or support dormant views (`SpecListResponse`, `SpecDetailResponse`, `SectionsResponse`, `SectionResponse`, `TreeNode`, `SpecTreeResponse`, `SearchResponse`, `HealthIssue`, `state_colors`, `state_accent`).

# Decision log
- No deletions were needed in this slice; the warned items are intentional frontend DTO / routing / API scaffolding, not obsolete code.
- Per requested policy (a), I used narrow item-level allowances rather than file-level suppression.

# Validation
Commands run:
- `cargo check -p ticket-viewer-dioxus -p spec-viewer-dioxus --message-format=short | grep ...`
- `cargo build -p ticket-viewer-dioxus -p spec-viewer-dioxus`

Results:
- Scoped warning recount for the exact `memory-viewers` file set is **0**.
- Both frontend crates build cleanly.
- Remaining warnings are all outside this ticket, concentrated in `viewer-api-dioxus` (next child `9c329f10`).

# Acceptance
- Scoped dead_code findings resolved. ✓
- No broader build regression introduced. ✓
- Focused rationale preserved on intentional scaffolding. ✓