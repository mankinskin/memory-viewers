## Rationale

The ticket-viewer is fully GPU-accelerated by default — the 3D graph view, glass
panels, animated background and particle effects are part of the visual identity of
the app. The master GPU toggle is provided so users can opt out of the overlay if
they prefer a lightweight, static presentation. The graph3d view continues to use
WebGPU regardless of the master toggle (it owns the canvas via `GPU_CANVAS_OWNER`).