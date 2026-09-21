# T2: Bug — Node Projection Broken (nodes cluster in top-left corner)

## Problem

3D hypergraph nodes are projected into a small area in the top-left corner instead of spreading across the viewport. Nodes should overflow behind glass sidebar/header panels (treated as overlays). Full 3D hypergraph rendering should only occur when the hypergraph tab is active; other tabs show only ambient GPU effects (smoke, particles).

## Root Cause Analysis

### Projection math uses wrong viewport dimensions

**hypergraph_view.rs L282–283**: The callback receives `cw: u32, ch: u32` from the canvas — but the canvas is currently scoped to `.lv-hypergraph-view` container (small area), not the full viewport. After T1, the canvas becomes full-viewport, so `cw`/`ch` will be physical pixels (viewport × DPR).

**math3d.rs L289–309**: `world_to_screen()` converts 3D → 2D:
```rust
// NDC → screen transform:
let sx = (ndc_x * 0.5 + 0.5) * cw;   // cw = canvas width (physical pixels after T1)
let sy = (1.0 - (ndc_y * 0.5 + 0.5)) * ch;
```

**Problem**: After T1, `cw`/`ch` are physical pixels (e.g., 3840×2160 on 2× HiDPI), but DOM elements are positioned in CSS/logical pixels (1920×1080). The node elements are scaled by 2× in position.

### DOM positioning doesn't account for DPR

**hypergraph_view.rs L187–197**: Node DOM elements positioned with absolute pixel values:
```rust
el.set_attribute("style",
    &format!("position:absolute;top:0;left:0;\
     transform:translate(-50%,-50%) translate({:.1}px,{:.1}px) scale({:.3});\
     z-index:{};visibility:visible;pointer-events:auto;",
     sx, sy, pixel_scale, z_idx))
```

The `sx`/`sy` values come from `world_to_screen()` which uses canvas physical pixels. DOM positioning needs logical pixels.

### Reference: TS nodePositioner.ts

**nodePositioner.ts L51–70**: Uses `vw`/`vh` (viewport logical pixels, not canvas physical pixels):
```typescript
const screen = worldToScreen([n.x, n.y, n.z], viewProj, vw, vh);
```

The TS version passes viewport dimensions (`window.innerWidth/Height`), not canvas buffer dimensions.

## Design

### Step 1: Separate render dimensions from projection dimensions

The GPU callback provides canvas physical dimensions for shader rendering. Node projection needs **logical** viewport dimensions.

Add to the render callback context:
```rust
fn update_node_transforms(
    layout: &[LayoutNode],
    vp: [f32; 16],
    eye: Vec3,
    // Use logical viewport dimensions, not canvas physical dimensions
    viewport_w: f32,  // window.innerWidth (logical)
    viewport_h: f32,  // window.innerHeight (logical)
) {
    for node in layout {
        let (sx, sy, sz, vis) = world_to_screen(
            [node.x, node.y, node.z], vp, viewport_w, viewport_h
        );
        // sx, sy are now in logical CSS pixels — correct for DOM positioning
    }
}
```

### Step 2: Get viewport logical dimensions

```rust
let window = web_sys::window().unwrap();
let vw = window.inner_width().unwrap().as_f64().unwrap() as f32;
let vh = window.inner_height().unwrap().as_f64().unwrap() as f32;
```

Pass `vw`/`vh` to `update_node_transforms()` instead of `canvas.width()`/`canvas.height()`.

### Step 3: Fix view_proj for projection matrix

**hypergraph_view.rs L237–245**: `view_proj()` builds the projection matrix with aspect ratio from canvas dimensions:
```rust
let aspect = w as f32 / h.max(1) as f32;
```

This should use the same logical viewport aspect ratio (which equals the physical pixel aspect ratio since DPR is uniform), so no change needed here — the aspect ratio is the same regardless of DPR.

### Step 4: Fix world_scale_at_depth

**math3d.rs L310–324**: `world_scale_at_depth()` uses `canvas_h`:
```rust
pub fn world_scale_at_depth(eye: Vec3, world_pos: Vec3, canvas_h: f32) -> f32
```

This must receive **logical** viewport height, not physical canvas height, since the result is used for DOM element `scale()` CSS transforms.

### Step 5: Node elements overflow behind glass panels

**hypergraph_view.rs L577–595**: `.hg-node-layer` container currently lives inside `.lv-hypergraph-view`. After T1, the GPU canvas is at App root but the node layer needs to be full-viewport too.

Move `.hg-node-layer` to be a sibling of the GPU canvas at App root:
```css
.hg-node-layer {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100dvh;
    pointer-events: none;  /* Pass clicks through to panels underneath */
    z-index: 0;            /* Behind glass panels but above canvas */
}

.hg-node-layer .hg-node {
    pointer-events: auto;  /* Individual nodes are clickable */
}
```

Glass panels (sidebar, header) have higher z-index but transparent backgrounds — nodes visually appear behind them.

### Step 6: Conditional 3D rendering by active tab

Add a signal `hypergraph_active: RwSignal<bool>` to the Store:
```rust
// store.rs
pub hypergraph_active: RwSignal<bool>,
```

In `app.rs`, update the signal when the active tab changes:
```rust
create_effect(move |_| {
    store.hypergraph_active.set(active_tab.get() == ViewTab::Hypergraph);
});
```

In the GPU render callback, check this signal:
```rust
// overlay.rs render_frame or the registered callback:
if store.hypergraph_active.get_untracked() {
    // Render full 3D scene: nodes, edges, grid, labels
    update_node_transforms(layout, vp, eye, vw, vh);
    // Show .hg-node-layer
} else {
    // Hide .hg-node-layer (display: none)
    // Only ambient effects render (smoke, particles controlled by shader uniforms)
}
```

The ambient shader effects (smoke, particles, CRT, grain) run unconditionally — they don't depend on hypergraph data. Only the node/edge/grid rendering and DOM node positioning are conditional.

### Step 7: Frustum culling

Port the TS culling logic (**nodePositioner.ts L68–76**):
```rust
const CULL_MARGIN: f32 = 200.0;
if !vis || pixel_scale < 0.02
    || sx < -CULL_MARGIN || sx > vw + CULL_MARGIN
    || sy < -CULL_MARGIN || sy > vh + CULL_MARGIN
{
    el.set_attribute("style", "display:none").ok();
    continue;
}
```

## Files to Modify

| File | Change |
|------|--------|
| `src/components/hypergraph_view.rs` | Use logical viewport dimensions for projection; move node layer to App root; add frustum culling |
| `src/gpu/overlay.rs` | Pass logical viewport dims to node callback; check `hypergraph_active` signal |
| `src/gpu/math3d.rs` | No changes needed (just pass correct dimensions) |
| `src/store.rs` | Add `hypergraph_active: RwSignal<bool>` |
| `src/app.rs` | Wire `hypergraph_active` signal to active tab; mount `.hg-node-layer` at root |
| `style.css` | `.hg-node-layer` → position:fixed, full viewport, z-index:0 |

## Acceptance Criteria

1. Node DOM elements project correctly using full viewport logical coordinates
2. Projection accounts for devicePixelRatio (GPU physical pixels vs DOM logical pixels)
3. Nodes visually appear behind transparent glass sidebar/header panels when they overlap
4. Full hypergraph rendering (nodes, edges, grid, labels) only active when hypergraph tab is selected
5. When on other tabs, only ambient GPU effects (smoke, particles, background) render
6. Switching to hypergraph tab re-enables full 3D rendering without flicker or delay
7. Frustum culling hides off-screen nodes (200px margin)
