## Tool Use Examples

```bash
viewer-ctl start ticket-viewer
viewer-ctl start spec-viewer
cargo run -p rule-cli -- sync-targets --config memory-viewers/rule-targets.yaml
```

- Start `ticket-viewer` to inspect work in progress and board relationships.
- Start `spec-viewer` to browse spec structure and linked code references.
- Regenerate the parent README when rule content changes.
