# GPU Radix Sort: 8-Pass Parallel Sort for Voxel Splat Depth+Tile Ordering

## Problem

After sort key construction (T6b), ~1M voxel splats must be sorted by composite key `(tile_id | depth)` for correct front-to-back compositing in the tiled rasterizer (T6d). This ticket implements an 8-pass, 4-bit GPU radix sort entirely in compute shaders — data never leaves VRAM.

## Scope

### Algorithm

For each 4-bit digit (8 passes for 32-bit key):
1. **Histogram**: count occurrences of each digit (0–15) per workgroup
2. **Prefix Sum** (Blelloch scan): compute global offsets
3. **Scatter**: write each element to its sorted position

### Histogram Pass

```wgsl
var<workgroup> local_histogram: array<atomic<u32>, 16>;

@compute @workgroup_size(256)
fn radix_histogram(@builtin(global_invocation_id) id: vec3u) {
    let entry = sort_keys[id.x];
    let digit = (entry >> current_bit_shift) & 0xFu;
    atomicAdd(&local_histogram[digit], 1u);
    workgroupBarrier();
    if id.x % 256u == 0u {
        for (var d = 0u; d < 16u; d++) {
            atomicAdd(&global_histograms[d + workgroup_id * 16u], atomicLoad(&local_histogram[d]));
        }
    }
}
```

### Prefix Sum Pass (Blelloch Scan)

```wgsl
@compute @workgroup_size(256)
fn prefix_sum(@builtin(global_invocation_id) id: vec3u) {
    // Up-sweep (reduce) then down-sweep over global_histograms
    // Produces cumulative offsets for each digit bucket
}
```

### Scatter Pass

```wgsl
@compute @workgroup_size(256)
fn radix_scatter(@builtin(global_invocation_id) id: vec3u) {
    let key = sort_keys[id.x];
    let digit = (key >> current_bit_shift) & 0xFu;
    let dest = offsets[digit] + local_offset;
    sorted_keys[dest] = key;
    sorted_values[dest] = sort_values[id.x];
}
```

### 8-Pass Dispatch

The RadixSortNode dispatches 3 compute passes × 8 digits = 24 dispatches, ping-ponging between sort_keys/sort_scratch buffers.

### Bevy Render Node

```rust
pub struct RadixSortNode;
impl Node for RadixSortNode {
    fn run(&self, ...) {
        for bit_shift in (0..32).step_by(4) {
            // 1. Dispatch histogram
            // 2. Dispatch prefix_sum
            // 3. Dispatch scatter (swap src/dst buffers)
        }
    }
}
```

## Dependencies
- T6b (sort key construction): sort_keys[] and sort_values[] input
- T2a (GPU buffer infra): sort_scratch, histograms buffers

## Acceptance Criteria
1. Sort correctly orders 1M elements by composite key
2. All 8 passes execute without data corruption (ping-pong correct)
3. Sort completes in < 1ms for 1M voxel splats
4. Data stays in VRAM — no CPU round-trip between passes
5. Workgroup atomics for histogram are correct (no race conditions)
6. Prefix sum produces valid cumulative offsets
