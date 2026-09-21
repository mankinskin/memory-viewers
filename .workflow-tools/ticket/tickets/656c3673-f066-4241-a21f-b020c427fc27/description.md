# Dioxus–Taffy Bridge: 2D UI Panels Composited Over Voxel-Splatted Scene

## Problem

The 2D HUD/panel layer (Dioxus virtual DOM → Taffy layout) must composite over the 3D Voxel-splatted scene. The bridge renders 2D UI to a texture that is alpha-blended on top of the final tiled rasterizer output.

## Architecture

### Bridge Pipeline

```
[Dioxus VDOM] → [Taffy layout] → [UiElement list]
  → [UI render pass] → ui_texture (RGBA8)
  → [Composite pass]: sample scene_color (from tiled rasterizer) + ui_texture → swapchain
```

The scene_color texture is the OUTPUT of the voxel splat tiled rasterizer (after all splats + glass refraction are composited). The UI layer is rendered independently and blended on top.

### Glass Effect on UI Panels

Certain UI panels can opt into a "frosted glass" look by sampling the scene_color texture with mipmap blur:

```wgsl
// In ui_panel.wgsl
@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

fn frosted_panel_bg(uv: vec2<f32>, blur_level: f32) -> vec4<f32> {
    return textureSampleLevel(scene_tex, scene_sampler, uv, blur_level);
}
```

This reuses the same mipmap chain generated for in-world frosted glass (T3), so no extra GPU work.

### UiElement → Draw Calls

```rust
pub struct UiElement {
    pub rect: taffy::Layout,    // x, y, width, height from Taffy
    pub bg_color: [f32; 4],
    pub border_color: [f32; 4],
    pub border_width: f32,
    pub text_runs: Vec<TextRun>,
    pub frosted_glass: bool,    // sample scene mipmap for background
    pub frost_blur: f32,        // mipmap level for blur (0.0 = clear, 4.0 = heavy)
}
```

### Composite Pass

```rust
// Final pass: blend ui_texture over scene_color
fn create_composite_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    // fullscreen triangle, samples scene_color + ui_color, alpha blends
}
```

### Dioxus Integration

```rust
pub fn dioxus_bridge_system(
    vdom: Res<DioxusVDom>,
    mut layout: ResMut<TaffyLayout>,
    mut ui_elements: ResMut<Vec<UiElement>>,
) {
    let diff = vdom.diff();
    layout.apply_diff(&diff);
    layout.compute();
    *ui_elements = layout.to_elements();
}
```

## Dependencies
- T2 (render init): Scene color texture from tiled rasterizer output
- T3 (liquid glass): Mipmap chain for frosted panel backgrounds
- T1 (scaffold): Dioxus + Taffy crate deps

## Acceptance Criteria
1. Dioxus VDOM diffs applied to Taffy layout each frame
2. UI elements rendered to ui_texture
3. Composite pass blends UI over voxel-splatted scene
4. Frosted glass panels sample scene mipmap with configurable blur
5. UI click/hover detection from Taffy hit testing
6. Text rendered with glyph atlas (wgpu_text or custom)
