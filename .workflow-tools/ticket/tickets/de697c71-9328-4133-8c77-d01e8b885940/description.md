# Runtime Parameters: Voxel Splatting, Tiling, Sorting, and Double Buffer Config

## Problem

All rendering pipeline parameters must be tweakable at runtime via a Bevy resource. This includes SVO parameters, splat generation, AABB screen projection, GPU radix sort, tiled rasterizer, glass effects, and double buffer behavior.

## Architecture

### Parameter Resources

```rust
#[derive(Resource)]
pub struct SvoParams {
    pub max_depth: u32,           // octree depth (default: 8 → 256³)
    pub chunk_size: u32,          // voxels per chunk for collider gen (default: 16)
}

#[derive(Resource)]
pub struct SplatParams {
    pub max_splats: u32,       // max splats in buffer (default: 2_000_000)
    pub roughness_bits: u32,     // roughness encoding bits (default: 5)
    pub lod_near: f32,            // distance for max detail (default: 5.0)
    pub lod_far: f32,             // distance for min detail (default: 100.0)
    pub lod_min_scale: f32,       // minimum splat scale at far LOD (default: 0.5)
    pub generation_enabled: bool, // can disable splat generation (debug)
}

#[derive(Resource)]
pub struct SortKeyParams {
    pub low_pass_filter: f32,     // anti-aliasing: added to cov2d diagonal (default: 0.3)
    pub cull_screen_threshold: f32, // skip splats smaller than this in pixels (default: 0.1)
}

#[derive(Resource)]
pub struct SortParams {
    pub radix_bits: u32,          // bits per pass (default: 4)
    pub num_passes: u32,          // total passes (default: 8 for 32-bit keys)
    pub workgroup_size: u32,      // threads per workgroup (default: 256)
}

#[derive(Resource)]
pub struct TileParams {
    pub tile_size: u32,           // pixels per tile edge (default: 16)
    pub max_splats_per_tile: u32, // limit per tile (default: 512)
    pub early_out_alpha: f32,     // stop blending when remaining alpha < this (default: 0.01)
}

#[derive(Resource)]
pub struct GlassParams {
    pub ior: f32,                        // index of refraction (default: 1.5)
    pub chromatic_r_scale: f32,          // R channel distortion (default: 1.0)
    pub chromatic_g_scale: f32,          // G channel (default: 1.1)
    pub chromatic_b_scale: f32,          // B channel (default: 1.2)
    pub caustic_strength: f32,           // pseudo-caustic brightness (default: 2.0)
    pub max_frost_mip_level: f32,        // max mipmap level for frosted blur (default: 9.0)
    pub curvature_blur_factor: f32,      // fwidth(normal) multiplier (default: 4.0)
}

#[derive(Resource)]
pub struct DoubleBufferParams {
    pub enabled: bool,                   // can disable for debugging (default: true)
    pub upload_budget_bytes: u32,        // max bytes per frame upload (default: 4MB)
}

#[derive(Resource)]
pub struct RenderParams {
    pub svo: SvoParams,
    pub splat: SplatParams,
    pub sort_key: SortKeyParams,
    pub sort: SortParams,
    pub tile: TileParams,
    pub glass: GlassParams,
    pub double_buffer: DoubleBufferParams,
}
```

### GPU Uniform Upload

Parameters flow to GPU via a uniform buffer updated each frame:

```rust
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct GpuRenderUniforms {
    // Camera
    pub view_matrix: [f32; 16],
    pub proj_matrix: [f32; 16],
    pub camera_pos: [f32; 4],
    pub viewport_size: [f32; 2],

    // Splat params
    pub max_splats: u32,
    pub roughness_bits: u32,     // roughness encoding bits (default: 5)
    pub lod_near: f32,
    pub lod_far: f32,
    pub lod_min_scale: f32,

    // AABB projection
    pub aabb_padding: f32,
    pub cull_screen_threshold: f32,

    // Tiling
    pub tile_size: u32,
    pub grid_width: u32,
    pub grid_height: u32,
    pub early_out_alpha: f32,

    // Glass
    pub glass_ior: f32,
    pub chromatic_scales: [f32; 3],
    pub caustic_strength: f32,
    pub max_frost_mip: f32,
    pub curvature_blur_factor: f32,

    // Frame
    pub frame_index: u32,
    pub _padding: [f32; 1],
}
```

### Upload System

```rust
fn upload_render_uniforms(
    params: Res<RenderParams>,
    camera: Query<(&Transform, &Projection), With<Camera3d>>,
    mut uniform_buffer: ResMut<GpuRenderUniformBuffer>,
    queue: Res<wgpu::Queue>,
) {
    let uniforms = GpuRenderUniforms::from_params(&params, &camera);
    queue.write_buffer(&uniform_buffer.buffer, 0, bytemuck::bytes_of(&uniforms));
}
```

### Runtime Tweaking

Parameters modified at runtime (e.g., via debug UI or world editor) take effect next frame — the uniform buffer is re-uploaded every frame. No pipeline recreation needed.

## Dependencies
- T1 (scaffold): Resource definitions
- T2 (render init): Uniform buffer creation and bind group
- T6 (3D scene): All compute/render passes read these uniforms

## Acceptance Criteria
1. All SVO, voxel splat, sort key, sort, tile, glass, double buffer params exposed as Bevy resources
2. GpuRenderUniforms uploaded every frame
3. Changing params at runtime affects rendering next frame
4. Default values produce correct rendering out of the box
5. `generation_enabled = false` stops splat generation (shows empty scene, useful for debug)
6. `double_buffer.enabled = false` falls back to single-buffer mode (may stutter but simpler to debug)
7. roughness change (e.g., 3→0) visibly reduces material fidelity
