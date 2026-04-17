//! `BatchPanel` — floating bottom bar for bulk ticket operations.
//!
//! Shown when one or more tickets are selected via the TicketTree checkboxes.
//! Provides buttons for state transitions, priority updates, close, and cancel.
//! On confirm it POSTs a `BatchRequest` to `/api/batch`, shows per-command
//! results, then calls `on_done` to clear the selection and refresh the list.

use dioxus::prelude::*;

use crate::api::{HttpTicketBackend, TicketBackend};
use crate::types::{BatchCommand, BatchRequest};

// ── Operation definitions ─────────────────────────────────────────────────────

/// A bulk operation the user can apply to all selected tickets.
#[derive(Clone, PartialEq)]
enum BulkOp {
    /// Transition every ticket to a specific state.
    SetState(String),
    /// Close every ticket (fast-forward to done).
    Close,
    /// Cancel every ticket.
    Cancel,
    /// Set priority field on every ticket.
    SetPriority(String),
}

// ── Props ─────────────────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct BatchPanelProps {
    /// Active workspace name.
    pub workspace: String,
    /// IDs of tickets currently selected.
    pub selected_ids: Vec<String>,
    /// Called when the batch operation is complete (or cancelled).
    pub on_done: EventHandler<()>,
}

// ── Result per-command ────────────────────────────────────────────────────────

#[derive(Clone)]
struct CommandResult {
    id: String,
    ok: bool,
    message: String,
}

// ── Component ─────────────────────────────────────────────────────────────────

#[component]
pub fn BatchPanel(props: BatchPanelProps) -> Element {
    // ── Local state ───────────────────────────────────────────────────────
    let mut pending_op: Signal<Option<BulkOp>> = use_signal(|| None);
    let mut running: Signal<bool> = use_signal(|| false);
    let mut results: Signal<Vec<CommandResult>> = use_signal(Vec::new);
    let mut global_error: Signal<Option<String>> = use_signal(|| None);

    let count = props.selected_ids.len();
    let has_results = !results.read().is_empty();

    // ── Bulk operation list ───────────────────────────────────────────────

    const STATE_OPS: &[(&str, &str)] = &[
        ("→ new", "new"),
        ("→ ready", "ready"),
        ("→ in-impl", "in-implementation"),
        ("→ in-review", "in-review"),
    ];

    const PRIORITY_OPS: &[(&str, &str)] = &[
        ("↑ critical", "critical"),
        ("↑ high", "high"),
        ("↓ medium", "medium"),
        ("↓ low", "low"),
    ];

    // ── Pre-compute style values (RSX format strings cannot contain quoted  ──
    // string literals inside {} blocks, so compute &str values before rsx!). ──
    let is_running = *running.read();
    let op_ready = pending_op.read().is_some();
    let btn_opacity = if is_running { "0.5" } else { "1" };
    let apply_bg = if op_ready && !is_running { "var(--accent-blue)" } else { "var(--bg-tertiary, #2a2a3a)" };
    let apply_cursor = if op_ready && !is_running { "pointer" } else { "default" };
    let apply_opacity = if op_ready && !is_running { "1" } else { "0.5" };
    let apply_label = if is_running { "Applying…" } else { "Apply" };

    rsx! {
        // ── Fixed bottom panel ────────────────────────────────────────────
        div {
            style: "
                position: fixed;
                left: 0; right: 0; bottom: 0;
                z-index: 200;
                display: flex;
                flex-direction: column;
                align-items: stretch;
                pointer-events: none;
            ",

            // ── Result panel (shown after apply completes) ────────────────
            if has_results {
                div {
                    style: "
                        pointer-events: all;
                        background: var(--bg-primary, #1a1a2e);
                        border-top: 1px solid var(--border-subtle, #333);
                        max-height: 200px;
                        overflow-y: auto;
                        padding: 8px 16px;
                    ",
                    div {
                        style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;",
                        span {
                            style: "font-size: 12px; font-weight: 600; color: var(--text-primary);",
                            "Batch results"
                        }
                        button {
                            style: "border: none; background: transparent; color: var(--text-muted); cursor: pointer; font-size: 11px;",
                            onclick: move |_| props.on_done.call(()),
                            "✕ Close & refresh"
                        }
                    }
                    for r in results.read().clone() {
                        {
                            // Pre-compute per-row values inside the block so rsx! sees
                            // only simple variable references in {} format segments.
                            let row_color = if r.ok { "var(--success, #4ade80)" } else { "var(--error, #f87171)" };
                            let short_id = if r.id.len() >= 8 { r.id[..8].to_string() } else { r.id.clone() };
                            let msg = r.message.clone();
                            rsx! {
                                div {
                                    key: "{r.id}",
                                    style: "
                                        display: flex; gap: 8px; align-items: baseline;
                                        font-size: 11px; padding: 2px 0;
                                        color: {row_color};
                                    ",
                                    span {
                                        style: "font-weight: 600; min-width: 100px; overflow: hidden; text-overflow: ellipsis;",
                                        "{short_id}"
                                    }
                                    span { "{msg}" }
                                }
                            }
                        }
                    }
                }
            }

            // ── Main action bar ───────────────────────────────────────────
            if !has_results {
                div {
                    style: "
                        pointer-events: all;
                        background: var(--bg-secondary, #13131f);
                        border-top: 2px solid var(--accent-blue, #3b82f6);
                        padding: 10px 16px;
                        display: flex;
                        flex-wrap: wrap;
                        gap: 8px;
                        align-items: center;
                    ",

                    // ── Selection count ───────────────────────────────────
                    span {
                        style: "font-size: 13px; font-weight: 600; color: var(--text-primary); margin-right: 4px;",
                        "{count} selected"
                    }

                    // ── Error message ─────────────────────────────────────
                    if let Some(ref err) = *global_error.read() {
                        span {
                            style: "font-size: 11px; color: var(--error, #f87171);",
                            "{err}"
                        }
                    }

                    // Spacer
                    div { style: "flex: 1;" }

                    // ── State transition buttons ──────────────────────────
                    for &(label, state_val) in STATE_OPS.iter() {
                        {
                            let op = BulkOp::SetState(state_val.to_string());
                            let is_pending = *pending_op.read() == Some(op.clone());
                            let bg = if is_pending { "var(--accent-blue)" } else { "var(--bg-primary, #1a1a2e)" };
                            rsx! {
                                button {
                                    key: "{state_val}",
                                    disabled: is_running,
                                    style: "
                                        padding: 5px 10px; border-radius: 5px;
                                        border: 1px solid var(--border-subtle, #333);
                                        background: {bg};
                                        color: var(--text-primary); cursor: pointer;
                                        font-size: 11px; font-weight: 600;
                                        opacity: {btn_opacity};
                                    ",
                                    onclick: move |_| {
                                        let cur = *pending_op.read() == Some(op.clone());
                                        pending_op.set(if cur { None } else { Some(op.clone()) });
                                        global_error.set(None);
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }

                    // Separator
                    span { style: "color: var(--border-subtle, #333); font-size: 14px;", "|" }

                    // ── Priority buttons ──────────────────────────────────
                    for &(label, prio) in PRIORITY_OPS.iter() {
                        {
                            let op = BulkOp::SetPriority(prio.to_string());
                            let is_pending = *pending_op.read() == Some(op.clone());
                            let bg = if is_pending { "var(--accent-purple, #a855f7)" } else { "var(--bg-primary, #1a1a2e)" };
                            rsx! {
                                button {
                                    key: "prio-{prio}",
                                    disabled: is_running,
                                    style: "
                                        padding: 5px 10px; border-radius: 5px;
                                        border: 1px solid var(--border-subtle, #333);
                                        background: {bg};
                                        color: var(--text-primary); cursor: pointer;
                                        font-size: 11px; font-weight: 600;
                                        opacity: {btn_opacity};
                                    ",
                                    onclick: move |_| {
                                        let cur = *pending_op.read() == Some(op.clone());
                                        pending_op.set(if cur { None } else { Some(op.clone()) });
                                        global_error.set(None);
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }

                    // Separator
                    span { style: "color: var(--border-subtle, #333); font-size: 14px;", "|" }

                    // ── Close button ──────────────────────────────────────
                    {
                        let op = BulkOp::Close;
                        let is_pending = *pending_op.read() == Some(op.clone());
                        let bg = if is_pending { "var(--success, #22c55e)" } else { "var(--bg-primary, #1a1a2e)" };
                        rsx! {
                            button {
                                disabled: is_running,
                                style: "
                                    padding: 5px 10px; border-radius: 5px;
                                    border: 1px solid var(--border-subtle, #333);
                                    background: {bg};
                                    color: var(--text-primary); cursor: pointer;
                                    font-size: 11px; font-weight: 600;
                                    opacity: {btn_opacity};
                                ",
                                onclick: move |_| {
                                    let cur = *pending_op.read() == Some(op.clone());
                                    pending_op.set(if cur { None } else { Some(op.clone()) });
                                    global_error.set(None);
                                },
                                "✓ Close"
                            }
                        }
                    }

                    // ── Cancel button ─────────────────────────────────────
                    {
                        let op = BulkOp::Cancel;
                        let is_pending = *pending_op.read() == Some(op.clone());
                        let bg = if is_pending { "var(--error, #ef4444)" } else { "var(--bg-primary, #1a1a2e)" };
                        rsx! {
                            button {
                                disabled: is_running,
                                style: "
                                    padding: 5px 10px; border-radius: 5px;
                                    border: 1px solid var(--border-subtle, #333);
                                    background: {bg};
                                    color: var(--text-primary); cursor: pointer;
                                    font-size: 11px; font-weight: 600;
                                    opacity: {btn_opacity};
                                ",
                                onclick: move |_| {
                                    let cur = *pending_op.read() == Some(op.clone());
                                    pending_op.set(if cur { None } else { Some(op.clone()) });
                                    global_error.set(None);
                                },
                                "✗ Cancel tickets"
                            }
                        }
                    }

                    // Separator
                    span { style: "color: var(--border-subtle, #333); font-size: 14px;", "|" }

                    // ── Apply button ──────────────────────────────────────
                    {
                        let ids = props.selected_ids.clone();
                        let ws = props.workspace.clone();
                        rsx! {
                            button {
                                disabled: !op_ready || is_running,
                                style: "
                                    padding: 6px 16px; border-radius: 6px; border: none;
                                    background: {apply_bg};
                                    color: white; cursor: {apply_cursor};
                                    font-size: 12px; font-weight: 700;
                                    opacity: {apply_opacity};
                                ",
                                onclick: move |_| {
                                    let Some(op) = pending_op.read().clone() else { return };
                                    let ids = ids.clone();
                                    let ws = ws.clone();
                                    running.set(true);
                                    global_error.set(None);

                                    // Build one command per selected ticket.
                                    let commands: Vec<BatchCommand> = ids.iter().map(|id| {
                                        match &op {
                                            BulkOp::SetState(state) => BatchCommand::Update {
                                                id: id.clone(),
                                                state: Some(state.clone()),
                                                from_state: None,
                                                fields: Default::default(),
                                            },
                                            BulkOp::Close => BatchCommand::Close {
                                                id: id.clone(),
                                                target_state: None,
                                            },
                                            BulkOp::Cancel => BatchCommand::Cancel {
                                                id: id.clone(),
                                                reason: None,
                                            },
                                            BulkOp::SetPriority(prio) => {
                                                let mut fields = std::collections::BTreeMap::new();
                                                fields.insert(
                                                    "priority".to_string(),
                                                    serde_json::Value::String(prio.clone()),
                                                );
                                                BatchCommand::Update {
                                                    id: id.clone(),
                                                    state: None,
                                                    from_state: None,
                                                    fields,
                                                }
                                            }
                                        }
                                    }).collect();

                                    let req = BatchRequest { workspace: ws, commands };
                                    spawn(async move {
                                        let backend = HttpTicketBackend::new(None);
                                        match backend.batch_tickets(&req).await {
                                            Ok(resp) => {
                                                let cmd_results: Vec<CommandResult> = req.commands
                                                    .iter()
                                                    .map(|cmd| {
                                                        let id = match cmd {
                                                            BatchCommand::Update { id, .. }
                                                            | BatchCommand::Close { id, .. }
                                                            | BatchCommand::Cancel { id, .. } => id.clone(),
                                                        };
                                                        CommandResult {
                                                            id,
                                                            ok: true,
                                                            message: format!("{} — ok", resp.status),
                                                        }
                                                    })
                                                    .collect();
                                                results.set(cmd_results);
                                                running.set(false);
                                                pending_op.set(None);
                                            }
                                            Err(e) => {
                                                global_error.set(Some(e));
                                                running.set(false);
                                            }
                                        }
                                    });
                                },
                                "{apply_label}"
                            }
                        }
                    }

                    // ── Deselect button ───────────────────────────────────
                    button {
                        disabled: is_running,
                        style: "
                            padding: 6px 12px; border-radius: 6px;
                            border: 1px solid var(--border-subtle, #333);
                            background: transparent;
                            color: var(--text-muted); cursor: pointer;
                            font-size: 12px; font-weight: 600;
                            opacity: {btn_opacity};
                        ",
                        onclick: move |_| props.on_done.call(()),
                        "✕ Deselect"
                    }
                }
            }
        }
    }
}
