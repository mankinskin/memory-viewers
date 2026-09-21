# T1: Bug — GPU Canvas Full Page + HiDPI

## Problem

The GPU canvas is currently scoped to the `.lv-hypergraph-view` container instead of covering the full viewport. It uses `position: absolute; inset: 0` within its parent, so it only renders in the hypergraph tab area. Additionally, there is no `devicePixelRatio` scaling, causing blurry rendering on HiDPI displays.

## Root Cause

### Canvas mounting (current Leptos)
- **overlay.rs L746–751**: `start_overlay()` receives the canvas from `HypergraphView` component — canvas is a child of `.lv-hypergraph-view`
- **style.css L390–402**: `.hg-gpu-canvas` uses `position: absolute; inset: 0` — scoped to parent container
- **app.rs L27**: GPU overlay only initializes when `HypergraphView` component mounts (Settings tab → no GPU)

### Canvas sizing (current Leptos)
- **overlay.rs L275–276**: `let w = self.canvas.width(); let h = self.canvas.height();` — reads raw canvas attrs
- No `devicePixelRatio` multiplication anywhere — canvas buffer matches CSS pixels, not physical pixels

### Reference: TS implementation
- **WgpuOverlay.tsx L92–106**: Canvas has `position: fixed; top: 0; left: 0; width: 100vw; height: 100dvh; pointer-events: none; zIndex: -1`
- **WgpuOverlay.tsx L70–75**: Canvas sizing: `canvas.width = window.innerWidth; canvas.height = window.innerHeight`
- **WgpuOverlay.tsx L77–80**: Resize listeners: `window.addEventListener('resize', sync)` + `window.visualViewport?.addEventListener('resize', sync)`

## Design

### Step 1: Move canvas to App level

Currently the canvas element is created inside `HypergraphView`. It needs to move to the `App` component root so it persists across all tabs.

**app.rs** — Add a `<canvas>` element at the root of the component tree (before `<Header />`):
```rust
// In App component view:
<canvas
    node_ref=canvas_ref
    class="gpu-canvas"
    style="position:fixed;top:0;left:0;width:100vw;height:100dvh;pointer-events:none;z-index:-1;"
/>
<div class="lv-app">
    <Header />
    ...
</div>
```

**overlay.rs** — `start_overlay()` signature stays the same (takes `HtmlCanvasElement`), but is now called from `App::on_mount` instead of `HypergraphView`.

### Step 2: HiDPI canvas buffer sizing

Add DPI-aware sizing in `start_overlay()` initialization and resize handler:

```rust
fn sync_canvas_size(canvas: &HtmlCanvasElement) {
    let window = web_sys::window().unwrap();
    let dpr = window.device_pixel_ratio();
    let w = window.inner_width().unwrap().as_f64().unwrap();
    let h = window.inner_height().unwrap().as_f64().unwrap();
    canvas.set_width((w * dpr) as u32);
    canvas.set_height((h * dpr) as u32);
    // CSS size stays at viewport dimensions (set via style attribute)
}
```

### Step 3: Resize listeners

Register both `resize` and `visualviewport.resize` via `web_sys`:

```rust
// In start_overlay(), after init_gpu:
let canvas_clone = canvas.clone();
let resize_cb = Closure::wrap(Box::new(move || {
    sync_canvas_size(&canvas_clone);
}) as Box<dyn Fn()>);

window.add_event_listener_with_callback("resize", resize_cb.as_ref().unchecked_ref())?;
if let Some(vp) = window.visual_viewport() {
    vp.add_event_listener_with_callback("resize", resize_cb.as_ref().unchecked_ref())?;
}
resize_cb.forget(); // Leak intentionally — lives for app lifetime
```

### Step 4: Update build_uniforms for physical pixels

**overlay.rs L381–451**: `build_uniforms(time, dt, w, h, elem_count)` receives canvas physical dimensions. The uniform buffer already uses these for shader resolution — no change needed in the shader, but ensure the `w`/`h` passed are the physical pixel values from `canvas.width()` / `canvas.height()` (which are now DPI-scaled).

### Step 5: GPU-active always on

- Move `gpu-active` class addition to App mount, not conditional on tab
- **overlay.rs L746–751**: Keep this in `start_overlay()` — it runs once at App mount now
- GPU always renders ambient effects (smoke, particles) regardless of active tab
- Tab-specific rendering (hypergraph 3D) controlled by a signal (see T2)

### Step 6: Update CSS

**style.css** — Replace `.hg-gpu-canvas` with `.gpu-canvas`:
```css
.gpu-canvas {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100dvh;
    pointer-events: none;
    z-index: -1;
}
```

Remove the old `.hg-gpu-canvas` block (L390–402).

## Files to Modify

| File | Change |
|------|--------|
| `src/app.rs` | Move canvas element to App root, call `start_overlay()` from App `on_mount` |
| `src/gpu/overlay.rs` | Add `sync_canvas_size()`, add resize listeners, remove canvas creation from HypergraphView dependency |
| `src/components/hypergraph_view.rs` | Remove canvas creation — use shared canvas ref from context |
| `style.css` | Replace `.hg-gpu-canvas` with `.gpu-canvas` (position:fixed, full viewport) |

## Acceptance Criteria

1. Canvas covers full viewport (`position: fixed`, `100vw × 100dvh`)
2. Canvas buffer size = viewport × devicePixelRatio (crisp on HiDPI)
3. Canvas resizes on `window.resize` and `visualViewport.resize`
4. GPU renders on all tabs (ambient effects always visible)
5. `gpu-active` class applied at App mount
6. Glass panel effects (backdrop-filter blur) work across all UI panels
