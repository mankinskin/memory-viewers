# Character: First-Person Camera Controller with SVO-Derived Rapier Collision

## Problem

The user navigates the 3D Voxel-splatted world via a first-person character controller. Physics (gravity, ground detection, collision response) is handled by bevy_rapier3d with collision geometry derived from the SVO. The character doesn't interact with splats directly — splats are visual-only.

## Architecture: Kinematic Character on SVO Terrain

### Character ECS Setup

```rust
#[derive(Component)]
pub struct CharacterController {
    pub move_speed: f32,
    pub sprint_multiplier: f32,
    pub jump_impulse: f32,
    pub mouse_sensitivity: f32,
    pub is_grounded: bool,
    pub vertical_velocity: f32,
}

#[derive(Bundle)]
pub struct CharacterBundle {
    pub controller: CharacterController,
    pub rigid_body: RigidBody,     // KinematicPositionBased
    pub collider: Collider,        // Capsule
    pub transform: Transform,
    pub global_transform: GlobalTransform,
}
```

### Movement + Rapier Kinematic Move

```rust
fn character_movement_system(
    input: Res<InputState>,
    mut characters: Query<(&mut CharacterController, &mut Transform)>,
    rapier_context: Res<RapierContext>,
    time: Res<Time>,
) {
    for (mut ctrl, mut transform) in characters.iter_mut() {
        let forward = transform.forward();
        let right = transform.right();
        let mut move_dir = Vec3::ZERO;
        if input.w { move_dir += forward; }
        if input.s { move_dir -= forward; }
        if input.d { move_dir += right; }
        if input.a { move_dir -= right; }
        move_dir.y = 0.0;
        if move_dir.length_squared() > 0.0 { move_dir = move_dir.normalize(); }

        let speed = ctrl.move_speed * if input.shift { ctrl.sprint_multiplier } else { 1.0 };

        if ctrl.is_grounded && input.space {
            ctrl.vertical_velocity = ctrl.jump_impulse;
        }
        ctrl.vertical_velocity -= 9.81 * time.delta_seconds();

        let displacement = move_dir * speed * time.delta_seconds()
            + Vec3::Y * ctrl.vertical_velocity * time.delta_seconds();

        let result = rapier_context.move_shape(
            transform.translation, &Collider::capsule_y(0.4, 0.3),
            displacement, QueryFilter::default(), 0.01,
        );

        transform.translation += result.effective_translation;
        ctrl.is_grounded = result.grounded;
        if ctrl.is_grounded && ctrl.vertical_velocity < 0.0 {
            ctrl.vertical_velocity = 0.0;
        }
    }
}
```

### Camera Look + PBR Material Interaction

Camera rotation affects not just the view but also how PBR materials are evaluated — rotating the camera causes materials to shift in appearance (glossy highlights move, metallic reflections change). This is automatic: the AABB screen projection pass re-evaluates PBR for the new view direction every frame.

### Input Handling (WASM)

```rust
#[derive(Resource, Default)]
pub struct InputState {
    pub w: bool, pub a: bool, pub s: bool, pub d: bool,
    pub space: bool, pub shift: bool,
    pub mouse_locked: bool,
}
```

Pointer lock on canvas click for FPS-style mouse control.

## Dependencies
- T7 (physics): Rapier + SVO-derived chunk colliders
- T6 (3D scene): Camera3D component; PBR evaluation depends on view direction
- T2 (render init): Canvas + window for input capture

> **Scope note:** This ticket covers the **local player** only (first-person camera, physics, input). Multiplayer remote-player rendering (SDF capsules, interpolation, nameplates) is handled by T20 — Multiplayer Characters, which depends on this ticket for the local-player baseline.

## Acceptance Criteria
1. WASD movement, mouse look with pitch clamp
2. Character falls under gravity, lands on SVO voxel surface
3. Jump + gravity works correctly
4. Slides along walls (Rapier kinematic response)
5. Sprint with shift
6. Pointer lock on click, ESC releases
7. Camera rotation causes visible material appearance shift on glossy/metallic splats
