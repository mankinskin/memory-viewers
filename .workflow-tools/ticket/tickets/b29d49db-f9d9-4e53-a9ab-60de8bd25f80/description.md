# Double-Buffered SVO Upload: BACK-Buffer Write + Swap System

## Problem

The VoxelWorld's dirty regions must be uploaded to the GPU without stalling the render loop. WASM writes to the BACK buffer while the GPU reads the FRONT buffer. After upload, swap makes new data available for the next frame's splat generation.

## Scope

### Upload System

```rust
fn svo_upload_system(
    mut voxel_world: ResMut<VoxelWorld>,
    svo_buffer: Res<SvoDoubleBuffer>,
    render_queue: Res<RenderQueue>,
) {
    let ranges = voxel_world.take_dirty_ranges();
    let node_bytes = bytemuck::cast_slice(&voxel_world.nodes);
    let target = svo_buffer.write_target(); // BACK buffer

    for (start, end) in ranges {
        render_queue.write_buffer(target, start as u64, &node_bytes[start..end]);
    }
}
```

### Swap System

```rust
fn double_buffer_swap_system(mut svo_buffer: ResMut<SvoDoubleBuffer>) {
    svo_buffer.swap(); // BACK → FRONT for next frame's rendering
}
```

### Buffer Resize

When the octree grows beyond capacity, both front and back buffers are recreated:

```rust
fn svo_resize_system(
    voxel_world: Res<VoxelWorld>,
    mut svo_buffer: ResMut<SvoDoubleBuffer>,
    device: Res<RenderDevice>,
) {
    if voxel_world.nodes.len() > svo_buffer.capacity_nodes {
        *svo_buffer = SvoDoubleBuffer::new(&device, voxel_world.nodes.len() * 2);
        // Full re-upload on next frame
    }
}
```

### Latency Guarantee

- Frame N: GPU renders from FRONT, WASM uploads to BACK
- Frame N+1: After swap, voxel splat kernel reads new FRONT data
- If upload takes > 1 frame: GPU keeps rendering stale FRONT at full FPS

## Dependencies
- T7a (VoxelWorld API): dirty_ranges from VoxelWorld
- T2a (GPU buffer infra): SvoDoubleBuffer resource

## Acceptance Criteria
1. Upload writes to BACK buffer only (FRONT is untouched during upload)
2. After swap, voxel splat kernel produces updated visuals
3. Complex edit (1000 voxels) does not drop frames
4. Dirty upload < 1ms for a 5-radius sphere brush
5. Buffer resize correctly recreates both front/back buffers
6. Swap is effectively free (< 0.01ms — pointer flip)
