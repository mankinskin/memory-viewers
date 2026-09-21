# Voxel Inventory: Mini-SVO Items, Glass UI Rendering & Drag-to-World

## Problem

Items in this RPG are physical voxel objects — each item is a small SVO (e.g., 8³) that can be inspected in 3D inside the inventory UI, rotated, and dragged into the world where it materializes as actual voxels. The inventory UI uses Liquid Glass panels to display items, with real-time refraction on the mini-SVO contents.

## Architecture: Items as Mini-SVOs in Glass Containers

### Inventory Data Flow

```
SpacetimeDB InventorySlot + ItemBlueprint tables
    ↓ (subscription)
Local InventoryCache resource
    ↓
Inventory UI system renders glass slots with mini-SVO viewports
    ↓
Ray-marching shader traverses mini-SVO in each slot's bounding box
    ↓
Drag & Drop: client ray-casts → SpacetimeDB reducer → voxels placed in world
```

### Client-Side Data

```rust
#[derive(Resource)]
pub struct InventoryCache {
    pub slots: Vec<CachedSlot>,
    pub active_slot: Option<usize>,   // hotbar selection
    pub drag_state: DragState,
}

pub struct CachedSlot {
    pub slot_id: u64,
    pub blueprint_id: u32,
    pub quantity: u32,
    pub mini_svo: Vec<OctreeNode>,    // deserialized from ItemBlueprint.voxel_data
    pub rotation: Quat,               // player can rotate item for inspection
}

pub enum DragState {
    None,
    Dragging { slot_index: usize, screen_pos: Vec2 },
    Previewing { slot_index: usize, world_pos: Vec3, valid: bool },
}
```

### Mini-SVO Rendering in Inventory Slots

Each inventory slot has a bounding box in the Liquid Glass UI. The tiled rasterizer's glass pre-loop already handles glass SDF panels (T3a). We extend this to render mini-SVOs *behind* the glass:

```wgsl
// In the glass pre-loop, after refraction ray direction is computed:
for (var slot = 0u; slot < inventory_slot_count; slot++) {
    let slot_bounds = inventory_slots[slot];
    // Transform refracted ray into mini-SVO local space
    let local_ray = transform_ray(refracted_ray, slot_bounds.world_matrix);

    // Traverse mini-SVO (same octree traversal as world SVO, different buffer)
    let hit = trace_mini_svo(local_ray, mini_svo_buffers[slot]);
    if hit.t < MAX_DIST {
        // Shade with SH lighting, apply glass tint
        color = shade_voxel(hit, slot_bounds.tint) * glass_attenuation;
    }
}
```

### Drag & Drop Pipeline

```
1. Mouse down on inventory slot
    → DragState::Dragging { slot_index, screen_pos }

2. Mouse move (while dragging)
    → Update screen_pos
    → If cursor is over 3D viewport:
        Ray-cast from camera through mouse position into world
        → DragState::Previewing { world_pos, valid }
        → Render ghost voxels at world_pos (transparent overlay)

3. Mouse up (while Previewing)
    → If valid placement:
        Call SpacetimeDB reducer: drop_item(slot_id, world_pos)
        Server: removes InventorySlot row, inserts voxel deltas at world_pos
        Client: subscription updates remove slot, add world voxels
    → If invalid: snap back to inventory (DragState::None)
```

### Server-Side Drop Reducer

```rust
#[spacetimedb::reducer]
pub fn drop_item(ctx: &ReducerContext, slot_id: u64, wx: i32, wy: i32, wz: i32) -> Result<(), String> {
    let slot = ctx.db.inventory_slot().slot_id().find(slot_id)
        .ok_or("Slot not found")?;
    if slot.owner != ctx.sender { return Err("Not your item".into()); }

    let bp = ctx.db.item_blueprint().blueprint_id().find(slot.blueprint_id)
        .ok_or("Blueprint missing")?;

    // Validate placement position is within range of player
    let player = ctx.db.player().identity().find(ctx.sender).unwrap();
    if distance(player.position, (wx as f32, wy as f32, wz as f32)) > MAX_INTERACTION_RANGE {
        return Err("Too far".into());
    }

    // Validate no overlap with existing solid voxels
    let voxels = deserialize_mini_svo(&bp.voxel_data);
    for (offset, color) in &voxels {
        let pos = IVec3::new(wx, wy, wz) + *offset;
        // Insert voxel deltas into the world
        insert_voxel_delta(ctx, pos, *color);
    }

    // Remove from inventory (decrement quantity or delete row)
    if slot.quantity > 1 {
        let mut updated = slot.clone();
        updated.quantity -= 1;
        ctx.db.inventory_slot().slot_id().update(updated);
    } else {
        ctx.db.inventory_slot().slot_id().delete(slot_id);
    }

    Ok(())
}
```

### Pickup (World → Inventory)

When the player interacts with a recognized voxel structure in the world (matching a blueprint pattern), a `pickup_item` reducer removes the world voxels and adds an `InventorySlot`. Pattern matching runs server-side to prevent fabricating items.

### Item Rotation in UI

While hovering over an inventory slot (not dragging), mouse drag rotates the mini-SVO's local transform. This is purely client-side — the rotation is a `Quat` in `CachedSlot`, fed into the mini-SVO's `world_matrix` uniform.

## Dependencies
- T17 (SpacetimeDB): InventorySlot + ItemBlueprint tables, drop/pickup reducers
- T10a (WorldPanel render): Glass SDF panel rendering for inventory slot containers
- T7a (VoxelWorld API): World voxel insertion when dropping items
- T16a (Core editor): Ray-cast infrastructure for drag placement targeting

## Acceptance Criteria
1. Inventory UI shows up to 36 slots as glass panels with 3D mini-SVO contents
2. Mini-SVOs render with correct lighting and glass refraction in each slot
3. Mouse drag rotates item in slot for 3D inspection
4. Drag-to-world shows ghost preview at valid placement positions
5. Dropping item: server removes from inventory, inserts voxels into world
6. Pickup: interacting with matching voxel pattern adds item to inventory
7. Quantity stacking works (same blueprint stacks, decrement on drop)
8. Items behind glass show correct refraction (Snell's law through panel)
