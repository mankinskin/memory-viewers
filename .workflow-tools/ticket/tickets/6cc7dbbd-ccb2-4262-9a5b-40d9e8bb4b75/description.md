# Multiplayer Characters: SDF Capsule Sync, Interpolation & Voxel Splat Rendering

## Problem

Other players must appear in the local client's 3D world as physical entities that cast shadows, refract through Liquid Glass, and move smoothly despite network latency. We render remote players as SDF capsules in the ray-marching loop and interpolate between server updates for fluid motion at 120Hz.

## Architecture: Server-Authoritative Position → Client-Side SDF Rendering

### Remote Player Data Flow

```
SpacetimeDB Player table update (server, ~20Hz)
    ↓ (subscription push)
RemotePlayerBuffer updated with { prev, curr, timestamp }
    ↓ (every frame)
Interpolation system computes smoothed position/rotation
    ↓
Upload to GPU uniform buffer (player_capsules[MAX_PLAYERS])
    ↓
Ray-marching shader evaluates sd_capsule for each remote player
    ↓
Refraction through Liquid Glass, shadow casting, PBR lighting
```

### Remote Player ECS

```rust
#[derive(Component)]
pub struct RemotePlayer {
    pub entity_id: u64,
    pub identity: Identity,
    pub prev_state: PlayerSnapshot,
    pub curr_state: PlayerSnapshot,
    pub prev_timestamp: f64,
    pub curr_timestamp: f64,
    pub skin_color: u32,
}

pub struct PlayerSnapshot {
    pub position: Vec3,
    pub yaw: f32,
    pub hp: i32,
    pub max_hp: i32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuPlayerCapsule {
    pub bottom: [f32; 3],
    pub radius: f32,
    pub top: [f32; 3],
    pub color: u32,
}
```

### Interpolation System

```rust
fn interpolate_remote_players_system(
    time: Res<Time>,
    mut players: Query<&mut RemotePlayer>,
    mut gpu_buffer: ResMut<PlayerCapsuleBuffer>,
) {
    let now = time.elapsed_seconds_f64();
    let mut capsules = Vec::new();

    for mut rp in players.iter_mut() {
        let dt = rp.curr_timestamp - rp.prev_timestamp;
        let alpha = if dt > 0.0 {
            ((now - rp.curr_timestamp) / dt).clamp(0.0, 1.2)  // allow slight extrapolation
        } else { 1.0 };

        let pos = rp.prev_state.position.lerp(rp.curr_state.position, alpha as f32);
        let yaw = lerp_angle(rp.prev_state.yaw, rp.curr_state.yaw, alpha as f32);

        // Capsule: feet at pos, head at pos + (0, 1.8, 0)
        capsules.push(GpuPlayerCapsule {
            bottom: pos.to_array(),
            radius: 0.3,
            top: (pos + Vec3::Y * 1.8).to_array(),
            color: rp.skin_color,
        });
    }

    gpu_buffer.upload(&capsules);
}
```

### Shader Integration (WGSL)

Remote players participate in the ray-marching loop as SDF primitives:

```wgsl
struct PlayerCapsule {
    bottom: vec3<f32>,
    radius: f32,
    top: vec3<f32>,
    color: u32,
};

@group(2) @binding(0) var<storage, read> players: array<PlayerCapsule>;
@group(2) @binding(1) var<uniform> player_count: u32;

fn sd_capsule(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

fn evaluate_players(p: vec3<f32>) -> HitResult {
    var closest = HitResult(MAX_DIST, vec3(0.0), 0u);
    for (var i = 0u; i < player_count; i++) {
        let cap = players[i];
        let d = sd_capsule(p, cap.bottom, cap.top, cap.radius);
        if d < closest.dist {
            closest = HitResult(d, unpack_color(cap.color), i);
        }
    }
    return closest;
}
```

### Interaction with Existing Systems

- **Liquid Glass**: Player capsule SDFs participate in Snell's law refraction. A player walking behind a glass panel is visible through the refraction.
- **Shadows**: Player SDFs cast shadows via shadow-ray evaluation in the tiled rasterizer's glass pre-loop.
- **splats**: Players do NOT generate splats. They are pure SDF entities evaluated alongside the SVO and glass SDFs, then composited with the splat alpha-blending output.
- **Nameplate**: A small text label rendered as a Dioxus overlay positioned via world-to-screen projection of the capsule top point.

### SpacetimeDB Callbacks

```rust
Player::on_insert(|_ctx, player| {
    // Spawn RemotePlayer entity in Bevy ECS
});

Player::on_update(|_ctx, _old, new_player| {
    // Shift curr → prev, update curr with new data
});

Player::on_delete(|_ctx, player| {
    // Despawn RemotePlayer entity
});
```

## Dependencies
- T18 (Networking): SpacetimeDB connection and subscription for Player table
- T8 (Character): Local player controller — remote players are the network-synced equivalent
- T6d (Tiled rasterizer): SDF evaluation loop where player capsules are rendered

## Acceptance Criteria
1. Remote players appear as smooth capsules in the 3D world
2. Movement interpolation at 120Hz from ~20Hz server updates (no jitter)
3. Player capsules cast shadows on voxels and splats
4. Light refracts correctly through glass panels onto player capsules
5. Player join/leave spawns/despawns entities in <100ms
6. Nameplate text tracks capsule top position in screen space
7. Up to 32 remote players renderable simultaneously without frame drop
8. Slight extrapolation (alpha 1.0–1.2) hides brief network gaps
