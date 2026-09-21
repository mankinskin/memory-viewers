# GPU Buffer Infrastructure: Double-Buffered SVO, Splat Buffers, and Bind Groups

## Problem

All GPU storage buffers and bind groups must be created before the render graph can execute. This ticket covers the buffer allocation layer — double-buffered SVO, voxel splatting buffers, bind group layouts, and the pre-built double bind groups that enable zero-allocation buffer swaps.

## Scope

### SvoDoubleBuffer Resource

```rust
#[derive(Resource)]
pub struct SvoDoubleBuffer {
    pub front: wgpu::Buffer,
    pub back: wgpu::Buffer,
    pub current_is_front: bool,
    pub capacity_nodes: usize,
}

impl SvoDoubleBuffer {
    pub fn new(device: &RenderDevice, capacity: usize) -> Self {
        let size = (capacity * std::mem::size_of::<OctreeNode>()) as u64;
        let usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
        Self {
            front: device.create_buffer(&BufferDescriptor { label: Some("svo_front"), size, usage, mapped_at_creation: false }),
            back: device.create_buffer(&BufferDescriptor { label: Some("svo_back"), size, usage, mapped_at_creation: false }),
            current_is_front: true,
            capacity_nodes: capacity,
        }
    }

    pub fn write_target(&self) -> &wgpu::Buffer {
        if self.current_is_front { &self.back } else { &self.front }
    }
    pub fn read_source(&self) -> &wgpu::Buffer {
        if self.current_is_front { &self.front } else { &self.back }
    }
    pub fn swap(&mut self) { self.current_is_front = !self.current_is_front; }
}
```

### SplatBuffers Resource

```rust
#[derive(Resource)]
pub struct SplatBuffers {
    pub splats: wgpu::Buffer,        // VoxelSplat[] from generator
    pub projected: wgpu::Buffer,        // ProjectedSplat[] from AABB projection
    pub sort_keys: wgpu::Buffer,        // u32[] (tile_id | depth)
    pub sort_values: wgpu::Buffer,      // u32[] (splat indices)
    pub sort_scratch: wgpu::Buffer,     // radix sort workspace
    pub histograms: wgpu::Buffer,       // per-workgroup histograms
    pub tile_data: wgpu::Buffer,        // TileData[] (offset, count per tile)
    pub splat_count: wgpu::Buffer,   // atomic counter
    pub max_splats: u32,
}
```

### Bind Group Layouts (WGSL)

```wgsl
// Group 0: SVO (reads FRONT buffer)
@group(0) @binding(0) var<storage, read> octree: array<OctreeNode>;
@group(0) @binding(1) var<uniform> camera: CameraUniforms;
@group(0) @binding(2) var<uniform> globals: GlobalUniforms;

// Group 1: splats
@group(1) @binding(0) var<storage, read_write> splats: array<VoxelSplat>;
@group(1) @binding(1) var<storage, read_write> projected: array<ProjectedSplat>;
@group(1) @binding(2) var<storage, read_write> sort_keys: array<u32>;

// Group 2: Tiles + glass
@group(2) @binding(0) var<storage, read> tile_data: array<TileData>;
@group(2) @binding(1) var<storage, read> sorted_instances: array<SortedInstance>;
@group(2) @binding(2) var<storage, read> glass_panels: array<GlassPanel>;

// Group 3: Mipmap background texture
@group(3) @binding(0) var bg_tex: texture_2d<f32>;
@group(3) @binding(1) var bg_sampler: sampler;
```

### DoubleBindGroups

Both front and back bind groups are pre-built. On swap, the render graph picks the other set — no per-frame allocation.

```rust
pub struct DoubleBindGroups {
    pub front_group: wgpu::BindGroup,
    pub back_group: wgpu::BindGroup,
}
impl DoubleBindGroups {
    pub fn active(&self, current_is_front: bool) -> &wgpu::BindGroup {
        if current_is_front { &self.front_group } else { &self.back_group }
    }
}
```

## Dependencies
- T1 (scaffold): Bevy App skeleton, wgpu device access

## Acceptance Criteria
1. SvoDoubleBuffer created with configurable capacity; swap works correctly
2. SplatBuffers created with configurable max_splats
3. Radix sort buffers (keys, values, scratch, histograms) allocated
4. Tile data buffer sized for screen resolution / 16×16
5. 4 bind group layouts created matching WGSL declarations
6. Pre-built double bind groups swap without per-frame allocation
7. No WebGPU validation errors in browser console
