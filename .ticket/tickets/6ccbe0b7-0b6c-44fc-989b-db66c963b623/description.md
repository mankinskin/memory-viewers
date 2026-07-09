W3. The left file-tree panel is badly formatted and expensive to render; widening the panel is sluggish and laggy.

Requirements:
- Fix formatting to match the viewer-api tree-view spec (a20a0395).
- Eliminate resize lag: decouple width changes from expensive relayout/measurement.
- Avoid per-frame DOM thrash during drag-resize.
- Bound render cost for long trees (virtualize or window the list).

Shared component lives in viewer-api and benefits spec-viewer/log-viewer too. Relates to prior FileTree work (d7971816 sortable header, c10cc92e sidebar parity).