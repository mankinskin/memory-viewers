# Arch: context-editor crate scaffold — Kernel vs World Crate Split

## Problem

The context-editor must not be a monolithic crate. It requires a high-performance separation between the **Engine (Kernel)** and the **Game Logic (World-Crate)**. The Kernel provides heavy infrastructure (Bevy, SVO, Voxel Splatting, WebGPU, SpacetimeDB sync, generic Dioxus UI), while the World-Crate injects the "soul" (semantics, rules, custom HTTP/LLM prompts, and domain data models).

## Architecture Overview: Kernel <-> World-Crate

The data flow ensures the Kernel keeps control over hardware resources, while the World-Crate defines semantic meaning:

1. **World-Crate (Specific)**: Implements `SandboxWorld` trait, defining App Schema, Rules, LLM Prompts, Domains, and Custom RPG/Sandbox logic.
2. **Kernel (Generic & Inclusive)**: 
   - `Dioxus` UI-Framework (provides Compound Components like `GlassScaffold`, `GlassPanel`, `LiquidTerminal`).
   - `WebGPU` Render-Loop (SDF/Voxel Buffers, Voxel Splatting, Tiled Forward+).
   - `SpacetimeDB` Sync-Engine.
   - Event-Bus / Dispatcher linking UI, logic, and networking.

## Scope

### Workspace Structure
```
tools/context-editor/
├── Cargo.toml               # Virtual workspace manifest
├── kernel/                  # The Generic Engine
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs           # Kernel API, SandboxWorld trait
│   │   ├── ui/              # GlassScaffold, GlassPanel, LiquidTerminal
│   │   ├── svo/             # SVO, Octree, GPU Upload
│   │   ├── splat/           # splat generation, GPU sorting
│   │   ├── net/             # SpacetimeDB connection, Event-Bus
│   │   └── gpu/             # WebGPU render pipeline
│   └── shaders/             # WGSL shaders
└── sandbox-app/             # The Specific World (App)
    ├── Cargo.toml
    ├── Trunk.toml
    ├── index.html
    └── src/
        ├── main.rs          # entrypoint: kernel::launch::<MyWorld>()
        ├── world.rs         # impl SandboxWorld for MyWorld
        ├── inventory/       # Custom inventory blueprints
        └── llm/             # Domain-specific prompts
```

### Core PoC Interfaces (Kernel)

```rust
// kernel/src/lib.rs

pub trait SandboxWorld: 'static + Send + Sync {
    fn name(&self) -> &str;
    fn process_event(&self, event: WorldEvent);
    fn trigger_generation(&self, prompt: String);
    // Providers for generic UI components
    fn sidebar_content<'a>(&'a self, cx: dioxus::core::Scope<'a>) -> dioxus::core::Element<'a>;
    fn inventory_content<'a>(&'a self, cx: dioxus::core::Scope<'a>) -> dioxus::core::Element<'a>;
}

// kernel/src/ui/mod.rs
use dioxus::prelude::*;

#[inline_props]
pub fn GlassScaffold<'a, W: SandboxWorld>(cx: Scope<'a>, world: &'a W) -> Element {
    render! {
        div { class: "kernel-root",
            canvas { id: "gpu-canvas", class: "absolute inset-0 z-0" } // WebGPU layer
            div { class: "relative z-10 flex h-screen w-screen p-4 pointer-events-none",
                // Left: Status & Inspection (injected from world)
                div { class: "w-1/4 pointer-events-auto", world.sidebar_content(cx) }
                // Right: Generative Terminal
                div { class: "w-1/4 absolute right-4 pointer-events-auto",
                    LiquidTerminal { world: world }
                }
            }
        }
    }
}

#[inline_props]
pub fn GlassPanel<'a>(cx: Scope<'a>, title: &'a str, children: Element<'a>) -> Element {
    render! {
        div { class: "glass-panel rounded-xl border border-white/20 bg-white/5 backdrop-blur-md p-4 shadow-2xl",
            div { class: "panel-header border-b border-white/10 pb-2 mb-4 font-bold text-white/80", "{title}" }
            div { class: "panel-content overflow-y-auto max-h-64", children }
        }
    }
}

#[inline_props]
pub fn LiquidTerminal<'a, W: SandboxWorld>(cx: Scope<'a>, world: &'a W) -> Element {
    let input_val = use_state(cx, || String::new());
    render! {
        GlassPanel { title: "Nexus Terminal",
            div { class: "space-y-4",
                p { class: "text-xs opacity-50", "Generative Mode Active..." }
                input {
                    class: "w-full bg-black/40 border border-white/20 p-2 text-green-400 font-mono text-sm",
                    placeholder: "> Enter prompt...",
                    value: "{input_val}",
                    oninput: move |evt| input_val.set(evt.value.clone()),
                    onkeydown: move |evt| {
                        if evt.key() == "Enter" {
                            world.trigger_generation(input_val.get().clone());
                            input_val.set(String::new());
                        }
                    }
                }
            }
        }
    }
}
```

### Dependencies (Latest Versions)
- `bevy = "0.14"` (upgraded for render features, `bevy_render`, `bevy_core_pipeline`)
- `bevy_rapier3d = "0.27"`
- `dioxus = { version = "0.5", features = ["web"] }` (upgraded component macros)
- `taffy = "0.3"`
- `spacetimedb-sdk = "0.10"` (latest stable)
- `naga = "0.20"` (latest WGSL compiler/validator)
- `noise = "0.9"` (procedural noise functions)
- `wgpu = "0.20"` (aligned with Bevy 0.14)

## Acceptance Criteria
1. Workspace splits into `kernel` and `sandbox-app` crates.
2. `kernel` compiles as a library containing Bevy loop, SpacetimeDB connection, and Dioxus Compound Components.
3. `sandbox-app` implements `SandboxWorld` and builds via `trunk build`, launching the Dioxus UI overlaid on the Bevy WebGPU canvas.
4. typing in the `LiquidTerminal` properly routes via the `SandboxWorld::trigger_generation` trait method.
