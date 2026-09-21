# Phase 4a: Frustum Culling, Paged SVO Upload and Virtual Address Table

## Problem

After the old pipeline is removed (Phase 3b), the renderer runs entirely on the SVO ray march shader using the existing `SvoDoubleBuffer`, whose GPU buffer is sized for the **entire** octree — all nodes are always resident in VRAM. For large worlds (depth 8 = 16M potential nodes), this wastes VRAM on nodes behind the camera or far outside the frustum.

This ticket introduces a paging system that uploads only frustum-visible subtree blocks to the GPU, together with the **Virtual Address Table (VAT)** that translates SVO node indices to GPU buffer offsets. These two features MUST be built together — without address translation, paging would break all `child_pointer` references in the octree, since the physical buffer positions change when pages are swapped in/out.

## Design

### Root Page: The Top of the Tree

The SVO is split into two tiers:
1. **Root page** (depth 0 to `page_depth - 1`): ALWAYS resident on the GPU. This is a single small block containing the top levels of the octree. Without it, traversal cannot even begin.
2. **Leaf pages** (depth `page_depth` to `max_depth`): Individually pageable subtrees that are uploaded/evicted based on visibility.

For `page_depth = 4`, the root page has at most $\sum_{d=0}^{3} 8^d = 585$ nodes ($1+8+64+512$) — trivially small. Each leaf page is rooted at a depth-4 internal node and contains all its descendants.

### Virtual Address Table (VAT) — Explicit Traversal-Time Indexing

Pages are independent subtrees with variable node counts. Fixed-size arithmetic (`node_index / nodes_per_page`) cannot work here because different pages hold different numbers of nodes. Instead, address resolution uses **explicit page tracking during traversal**: `first_child_idx` encodes different information depending on which tier the node is in.

| Node location | `first_child_idx` meaning |
|---|---|
| Root page (depth < `page_depth − 1`) | Absolute GPU buffer index of first child |
| Page-boundary node (depth == `page_depth − 1`) | Base leaf **page_id** of the first child page |
| Leaf page (depth > `page_depth − 1`) | **Relative offset from current leaf page base** |

Address resolution during traversal (no division or modulo needed):

```wgsl
// Root page traversal — compact child decode (occupied octants only):
if (child_mask & (1u << octant_slot)) == 0u { /* octant empty — miss */ }
let child_rank = countOneBits(child_mask & ((1u << octant_slot) - 1u));
physical_idx = first_child_idx + child_rank   // first_child_idx = GPU index of first packed child

// Crossing a page boundary (depth == page_depth − 1) — compact child pages:
if (child_mask & (1u << octant_slot)) == 0u { /* no child page for this octant */ }
let child_rank = countOneBits(child_mask & ((1u << octant_slot) - 1u));
let page_id  = first_child_idx + child_rank   // first_child_idx = page_table idx of first child page
let new_base = page_table[page_id]            // 0xFFFFFFFF → not resident
if new_base == 0xFFFFFFFFu { /* render nearest resident ancestor as solid; stop */ }
// page_base is per-stack-entry: each pushed frame carries its own page_base (not a global variable)
physical_idx = new_base                       // leaf page root is at per-page relative offset 0

// Leaf page traversal — compact child within current leaf page:
if (child_mask & (1u << octant_slot)) == 0u { /* octant empty within this page */ }
let child_rank = countOneBits(child_mask & ((1u << octant_slot) - 1u));
let page_base = stack_entry.page_base;        // per-stack-entry context (not a global variable)
physical_idx = page_base + first_child_reloff + child_rank
```

Sentinel value `page_table[id] == 0xFFFFFFFF` means the page is not resident → the shader stops descending and renders the nearest resident ancestor as a solid voxel (graceful degradation, no holes).

**CPU packing**: `pack_leaf_page()` serialises each subtree into the GPU buffer, **packing only occupied children in ascending octant order** (compact layout — no empty-slot placeholders). All `first_child_idx` values within the leaf page are rewritten to **relative offsets from the page base** (page root = offset 0). Nodes in the root page whose children cross the page boundary have their `first_child_idx` replaced with the page_table index of the **first (lowest occupied octant) child page**; child-page selection at traversal time uses `countOneBits(child_mask & octant_mask)` to compute rank. No power-of-two padding or alignment is required since pages are variable-size.

### Frustum Culling

Each frame (or when camera moves significantly):
1. Extract 6 frustum planes from the view-projection matrix
2. Traverse the SVO to `page_depth` level
3. For each leaf page root, compute its world-space AABB using the SVO transform
4. Test AABB against the 6 frustum planes (conservative — inside or intersecting = visible)
5. Include a 1-page margin around the frustum for smooth camera panning

### Upload Strategy
- On frustum change: diff the new visible set against the current resident set
- Upload newly visible pages, evict pages that left the frustum
- Use dirty-range system within resident pages (editing visible voxels = partial re-upload only)
- Page eviction is lazy — only evict when GPU buffer space pressure

### GPU Buffer Layout
```
[Root Page (always resident)][Leaf Page 0 nodes...][Leaf Page 1 nodes...]...[Leaf Page N nodes...]
[Page Table: virtual_page_id → physical_base_offset, one u32 per page]
```

## Implementation Plan

0. **Color propagation for fallback rendering**: Add `propagate_colors_up()` to `VoxelWorld` — bottom-up color averaging that fills `color_data` on internal nodes with the average of their children’s colors. This runs once per SVO modification (not per-frame). Without this, non-resident pages would render as black voxels when the shader falls back to the nearest resident ancestor. This must land alongside paging, not after.
1. Add `PageManager` to `kernel/src/svo/mod.rs`:
   - `page_depth: u32` (configurable, default 4)
   - `page_table: Vec<u32>` (virtual → physical offset)
   - `resident_pages: HashSet<u32>` (currently uploaded page indices)
   - `fn frustum_cull(&mut self, frustum: &Frustum, svo: &VoxelWorld) -> PageDiff`
   - `fn pack_pages(&self, svo: &VoxelWorld, visible: &[u32]) -> (Vec<OctreeNode>, Vec<u32>)`
2. Add `Frustum` utility struct with 6 planes extracted from view-projection matrix
3. Modify `svo_upload_system` to use `PageManager` for selective upload
4. Add `page_table` as a GPU storage buffer (read-only for shader)
5. Add `resolve_node()` WGSL helper to `svo_common.wgsl`
6. Update the ray march shader to use `resolve_node()` instead of direct `octree[idx]`
7. Update Phase 2a’s `svo_lookup()` neighbor-query function to use `resolve_node()` for page-aware address translation
8. Update `SvoTransformUniform` with a `page_depth: u32` field (no `nodes_per_page` — the new design does not use fixed-size page arithmetic)
9. Ensure root page is ALWAYS uploaded (never evicted)
10. `propagate_colors_up()` fills internal node `color_data` with average child color — used as fallback for non-resident pages and LOD rendering (Phase 4b)

## Acceptance Criteria

1. Only subtrees intersecting the camera frustum are uploaded to the GPU.
2. The root page (depth 0 to page_depth-1) is always resident — traversal always starts.
3. GPU buffer size is proportional to visible nodes, not total SVO size.
4. Turning the camera 180° evicts back-facing pages and uploads new front-facing ones.
5. Page table is accessible from WGSL as `var<storage, read> page_table: array<u32>`.
6. `resolve_node()` correctly translates virtual indices to physical buffer reads.
7. Non-resident pages render as their nearest resident ancestor using propagated average color (no holes/glitches/black patches).
8. A 256³ world with camera viewing ~25% of it uploads ~25% of the SVO.
9. Dirty-range tracking still works within resident pages (editing visible voxels = partial re-upload).
10. No visual differences from the non-paged version (same rendering output, just less VRAM used).

## Risk

- Page_depth too shallow = too many pages, management overhead
- Page_depth too deep = pages too large, poor culling granularity
- Default of 4 gives 4096 max leaf pages for a depth-8 SVO — reasonable tradeoff

## Dependencies

- Phase 3b (old pipeline removed — GPU buffer space freed up, render graph simplified)

Note: The core ray march shader (Phase 1b) currently uses direct `octree[idx]` lookups with the full SVO resident. This phase modifies it to use `resolve_node()` for all node accesses. The change is transparent to all other shader logic.
