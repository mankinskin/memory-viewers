## Tool Use Examples

### Install the shared tools first

From the parent `context-engine` checkout, install the viewer lifecycle tooling and authoring CLIs in one step:

```bash
bash ./install-tools.sh
```

That installs `viewer-ctl`, `trunk`, `spec-viewer`, `ticket-viewer`, and the `rule`, `spec`, `ticket`, and `audit` CLIs onto Cargo's bin path. If you are working from a standalone `memory-viewers` checkout, follow the direct install commands in [memory-api/README.md](memory-api/README.md) and [viewer-api/README.md](viewer-api/README.md).

### Start the viewers and regenerate docs

```bash
viewer-ctl start ticket-viewer
viewer-ctl start spec-viewer
rule sync-targets --config memory-viewers/rule-targets.yaml
```

- `viewer-ctl start ...` follows the lifecycle workflow documented in [viewer-api/viewer-ctl/README.md](viewer-api/viewer-ctl/README.md).
- `rule sync-targets --config memory-viewers/rule-targets.yaml` is documented in [memory-api/tools/cli/rule-cli/README.md](memory-api/tools/cli/rule-cli/README.md).
- Continue from [memory-api/README.md](memory-api/README.md) for rule, spec, ticket, and audit command surfaces and from [viewer-api/README.md](viewer-api/README.md) for shared viewer runtime details.
