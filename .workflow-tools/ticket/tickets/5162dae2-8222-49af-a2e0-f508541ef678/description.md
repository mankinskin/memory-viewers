# Feature: Interaction Bridge: 2D Unprojection & Dioxus-to-WASM Pipeline

## Problem

Clicks and hovers originating inside the generic Dioxus UI layer (`kernel-root`) must translate to 3D physical world events without stalling the main browser thread. The translation from 2D DOM coordinates into 3D voxel rays requires a dedicated, async "Interaction Bridge."

## Architecture

### The Unprojector (Screen to World Ray)
The Kernel translates `MouseEvent` coordinates on the `canvas` via the current inverse view-projection matrix:
```rust
// kernel/src/interaction.rs
pub fn screen_to_world_ray(screen_pos: Vec2, viewport: Vec2, view_proj: Mat4) -> Ray {
    let ndc = Vec2::new(
        (screen_pos.x / viewport.x) * 2.0 - 1.0,
        1.0 - (screen_pos.y / viewport.y) * 2.0
    );
    let inv = view_proj.inverse();
    let near = inv.project_point3(Vec3::new(ndc.x, ndc.y, 0.0));
    let far = inv.project_point3(Vec3::new(ndc.x, ndc.y, 1.0));
    
    Ray { origin: near, direction: (far - near).normalize() }
}
```

### Async Event Pipeline
Dioxus pushes `KernelEvent::Interact` down an asynchronous channel linking the DOM thread immediately to the Bevy/WASM main loop context loop:
```rust
kernel_tx.send(KernelEvent::Interact {
    ray,
    interaction_type: Interaction::PrimaryAction,
});
```
This guarantees Dioxus never blocks while waiting for the GPU tree query.

### The World-Crate Brain
Inside the Render loop, the Kernel performs a hard `svo.ray_cast(ray)`. However, the Kernel remains agnostic towards rules. It queries the Domain Trait:
```rust
if let Some(hit) = svo.ray_cast(ray) {
    world_logic.on_voxel_hit(hit, &mut world_state); 
}
```

### Liquid Hover Caustics
When the ray identifies a hit directly underneath a UI panel bounding box (hover event), it calculates local divergence. The shader uses this to bundle light (caustics effect) behind the Dioxus element before clicking, creating heavy physical presence in the DOM overlay.

## Dependencies
- T10b (Panel interaction) which handles the 3D-bound panels vs screen-bound UI separation.
- T1 (Scaffold / Kernel split) for the generic `WorldEvent` struct routing.
- T7a (VoxelWorld API) for CPU-side / WASM `svo.ray_cast(ray)`.

## Acceptance Criteria
1. Clicking arbitrary screen space unprojects accurate origin and direction rays bounding exactly matching the SVO renderer's camera model.
2. Clicking fires an asynchronous `KernelEvent` and resolves an `svo.ray_cast()` in the next pipeline tick.
3. The trait method `world_logic.on_voxel_hit()` receives accurate world coordinates (e.g. hit position, normal vector).
4. Hovering updates a shader buffer resulting in real-time visible caustics tracking the mouse location through the Liquid Glass overlay.
