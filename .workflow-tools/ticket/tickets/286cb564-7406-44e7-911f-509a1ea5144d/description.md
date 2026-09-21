# Multiplayer Networking: WebTransport, Spatial Subscriptions & Chunk Sync

## Problem

The client must efficiently synchronize with SpacetimeDB in real time. In an open world with millions of voxels, the client cannot load everything — it needs spatial subscriptions that load/unload chunks based on player position, with low-latency WebTransport for movement and combat data.

## Architecture: Spatial Subscription Manager

### Subscription Strategy

```
┌────────────────────────────────────────────┐
│           Subscription Layers               │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ Near (3×3×3 chunks) — Full SVO      │   │
│  │  All octree leaf nodes              │   │
│  │  All VoxelDelta for these chunks    │   │
│  │  All Players in range               │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ Mid (7×7×7 chunks) — LOD level 2    │   │
│  │  Aggregated meta-voxels (depth-3)   │   │
│  │  Player positions only (no inv)     │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ Far (15×15×15 chunks) — LOD level 4 │   │
│  │  Chunk color averages only          │   │
│  │  No player data                     │   │
│  └─────────────────────────────────────┘   │
└────────────────────────────────────────────┘
```

### Client-Side Subscription Manager

```rust
#[derive(Resource)]
pub struct ChunkSubscriptionManager {
    pub current_chunk: IVec3,
    pub near_radius: i32,       // 1 → 3×3×3
    pub mid_radius: i32,        // 3 → 7×7×7
    pub far_radius: i32,        // 7 → 15×15×15
    pub loaded_chunks: HashMap<IVec3, ChunkState>,
    pub pending_unsubscribe: Vec<IVec3>,
}

#[derive(Clone)]
pub enum ChunkState {
    Loading,
    FullDetail(ChunkData),
    LodLevel(u8, LodChunkData),
    Unloading,
}
```

### Subscription Update System (Bevy)

```rust
fn update_subscriptions_system(
    player: Query<&Transform, With<CharacterController>>,
    mut sub_mgr: ResMut<ChunkSubscriptionManager>,
    stdb_conn: Res<SpacetimeDbConnection>,
) {
    let pos = player.single().translation;
    let new_chunk = world_to_chunk(pos);

    if new_chunk != sub_mgr.current_chunk {
        sub_mgr.current_chunk = new_chunk;

        // Compute desired set for each LOD tier
        let near_set = chunk_sphere(new_chunk, sub_mgr.near_radius);
        let mid_set = chunk_sphere(new_chunk, sub_mgr.mid_radius);
        let far_set = chunk_sphere(new_chunk, sub_mgr.far_radius);

        // Diff against loaded_chunks → subscribe new, unsubscribe old
        let to_sub = near_set.difference(&sub_mgr.loaded_chunks.keys().collect());
        let to_unsub = sub_mgr.loaded_chunks.keys()
            .filter(|k| !far_set.contains(k))
            .collect::<Vec<_>>();

        for chunk in to_sub {
            stdb_conn.subscribe_chunk(*chunk, lod_for_distance(new_chunk, *chunk));
        }
        for chunk in to_unsub {
            stdb_conn.unsubscribe_chunk(*chunk);
            sub_mgr.loaded_chunks.remove(chunk);
        }
    }
}
```

### SpacetimeDB Connection Wrapper

```rust
#[derive(Resource)]
pub struct SpacetimeDbConnection {
    pub client: spacetimedb_sdk::DbConnection,
    pub status: ConnectionStatus,
}

pub enum ConnectionStatus {
    Connecting,
    Connected,
    Reconnecting { attempts: u32 },
    Disconnected,
}
```

### WebTransport vs WebSocket

- **WebTransport** (preferred): UDP-based, lower latency for position updates. Use `web_sys::WebTransport` when available.
- **WebSocket** (fallback): TCP-based, reliable for voxel mutations and inventory changes.
- Movement packets use unreliable datagrams (WebTransport) — lost packets are fine since next position overwrites.
- Voxel mutations use reliable ordered stream — every delta must arrive.

### Chunk Data Flow

```
Server DB change
    ↓ (subscription push via WebSocket/WebTransport)
SpacetimeDB SDK on_insert / on_update callback
    ↓
ChunkSubscriptionManager updates ChunkState
    ↓
VoxelWorld.apply_server_delta(chunk, delta)
    ↓
Mark dirty_ranges for GPU upload (T7b)
    ↓
splats regenerated for affected chunks (T6a)
```

### Delta Application

```rust
fn apply_server_updates_system(
    mut world: ResMut<VoxelWorld>,
    sub_mgr: Res<ChunkSubscriptionManager>,
) {
    // Process queued deltas from SpacetimeDB callbacks
    for delta in sub_mgr.drain_pending_deltas() {
        let (wx, wy, wz) = chunk_local_to_world(delta.chunk_hash, delta.local_x, delta.local_y, delta.local_z);
        world.set_voxel(wx, wy, wz, delta.new_color);
        // dirty_ranges updated automatically by VoxelWorld
    }
}
```

### Reconnection & Conflict Resolution

- On disconnect: queue local mutations, pause subscription manager
- On reconnect: replay queued mutations via reducers, re-subscribe current chunks
- Server is authoritative — client-side prediction is overridden by server response

## Dependencies
- T17 (SpacetimeDB module): Server tables and reducers must exist
- T7a (VoxelWorld API): `set_voxel()` and dirty-range tracking for applying server deltas

## Acceptance Criteria
1. Client connects to SpacetimeDB instance and authenticates
2. Moving between chunks triggers subscribe/unsubscribe cycle
3. Near chunks load full octree detail; mid/far load LOD data
4. Voxel changes by other players appear within ~100ms
5. Player positions stream via unreliable datagrams (WebTransport) or WebSocket fallback
6. Chunk unsubscribe frees local memory (VoxelWorld nodes removed)
7. Reconnection re-subscribes current chunk set and replays queued mutations
8. Bandwidth stays under 50KB/s during normal movement (no bulk re-download)
