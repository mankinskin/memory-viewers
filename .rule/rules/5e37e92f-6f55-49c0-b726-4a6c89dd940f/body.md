## Tool Use Examples

### Install the shared tools first

From the parent `context-engine` checkout, install the viewer lifecycle tooling and authoring CLIs in one step:

```bash
bash ./install-tools.sh
```

That installs `viewer-ctl`, `trunk`, and the `rule`, `spec`, `ticket`, and `audit` CLIs onto Cargo's bin path. If you are working from a standalone `memory-viewers` checkout, follow the direct install commands in [memory-api/README.md](memory-api/README.md) and [viewer-api/README.md](viewer-api/README.md).

### Start the viewers and regenerate docs

```bash
viewer-ctl start ticket-viewer
viewer-ctl start spec-viewer
rule sync-targets --config memory-viewers/rule-targets.yaml
```

- Start `ticket-viewer` to inspect work in progress and board relationships.
- Start `spec-viewer` to browse spec structure and linked code references.
- Regenerate the parent README when rule content changes.
