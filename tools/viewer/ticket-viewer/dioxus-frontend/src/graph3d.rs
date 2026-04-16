//! Graph3D — Hybrid GPU+DOM 3-D dependency graph renderer.
//!
//! Architecture (mirrors viewer-api Graph3DView):
//!   - **GPU canvas** (`#webgpu-canvas`): renders edges as animated energy beams
//!   - **DOM layer**: renders ticket nodes as styled HTML card elements
//!     positioned each frame via CSS 3D transforms
//!   - **Camera**: orbit camera with yaw/pitch/distance/target
//!
//! Nodes are NOT GPU-rendered spheres.  They are real DOM elements styled
//! with CSS, clicked and navigated normally, and projected from 3D world
//! coordinates to screen coordinates via the camera's view-projection matrix.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use dioxus::prelude::*;
use gloo_events::EventListener;
use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    GpuBuffer, GpuDevice, GpuRenderPipeline, GpuCanvasContext,
    HtmlCanvasElement, HtmlElement,
};

use crate::backend::{HttpTicketBackend, TicketBackend};
use crate::components::ticket_card;
use crate::graph::GraphLayout;

// ── Constants ────────────────────────────────────────────────────────────────

const CAMERA_FOV: f32 = std::f32::consts::FRAC_PI_4; // 45°
const CAMERA_NEAR: f32 = 0.1;
const CAMERA_FAR: f32 = 200.0;
/// viewProj(64) + eye(16) + time(16) = 96 bytes = 24 floats.
const CAM_UNIFORM_FLOATS: usize = 24;
/// ThemePalette: 24 × vec4f = 384 bytes = 96 floats.
const PALETTE_FLOATS: usize = 96;
/// Per-edge instance: posA(3)+posB(3)+color(4)+flags(1)+edgeType(1) = 12 floats.
const EDGE_INST_FLOATS: usize = 12;

const USAGE_VERTEX: u32 = 0x0020;
const USAGE_UNIFORM: u32 = 0x0040;
const USAGE_COPY_DST: u32 = 0x0008;

// ── 3-D Layout ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Node3D {
    pub id: String,
    pub title: Option<String>,
    pub state: Option<String>,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone)]
struct EdgeRef {
    from_idx: usize,
    to_idx: usize,
    kind: String,
}

#[derive(Debug, Clone)]
pub struct Layout3D {
    pub nodes: Vec<Node3D>,
    edges: Vec<EdgeRef>,
}

impl Layout3D {
    /// Build a 3-D layout from the 2-D force-directed graph layout.
    ///
    /// The 2-D positions (in layout-pixel space) are scaled to world units
    /// and the BFS depth is mapped to the Z axis.
    pub fn from_2d(gl: GraphLayout) -> Self {
        let scale = 1.0 / 100.0_f32;

        let nodes: Vec<Node3D> = gl
            .nodes
            .iter()
            .map(|gn| Node3D {
                id: gn.id.clone(),
                title: gn.title.clone(),
                state: gn.state.clone(),
                x: gn.x as f32 * scale,
                y: -(gn.y as f32 * scale), // flip Y so screen-up = world-up
                z: gn.depth as f32 * -4.0,
            })
            .collect();

        let idx_map: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();

        let edges: Vec<EdgeRef> = gl
            .edges
            .iter()
            .filter_map(|e| {
                let &from = idx_map.get(e.from.as_str())?;
                let &to = idx_map.get(e.to.as_str())?;
                Some(EdgeRef {
                    from_idx: from,
                    to_idx: to,
                    kind: e.kind.clone(),
                })
            })
            .collect();

        Self { nodes, edges }
    }

    /// Build a flat f32 buffer of edge instance data for the GPU.
    fn build_edge_instances(&self) -> (Vec<f32>, u32) {
        let mut data = Vec::with_capacity(self.edges.len() * EDGE_INST_FLOATS);
        let mut count = 0u32;
        for edge in &self.edges {
            let a = &self.nodes[edge.from_idx];
            let b = &self.nodes[edge.to_idx];
            let (r, g, bl, alpha) = match edge.kind.as_str() {
                "depends_on" => (0.15, 0.75, 0.90, 0.70),
                "blocks" => (0.90, 0.40, 0.20, 0.70),
                _ => (0.50, 0.50, 0.70, 0.50),
            };
            data.extend_from_slice(&[
                a.x, a.y, a.z, // posA
                b.x, b.y, b.z, // posB
                r, g, bl, alpha, // color
                0.0,  // flags
                1.0,  // edgeType = normal energy beam
            ]);
            count += 1;
        }
        (data, count)
    }
}

// ── Camera ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Camera {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: [f32; 3],
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: 0.3,
            pitch: 0.4,
            distance: 25.0,
            target: [0.0, 0.0, -4.0],
        }
    }
}

impl Camera {
    fn eye(&self) -> [f32; 3] {
        let cp = self.pitch.cos();
        [
            self.target[0] + self.distance * cp * self.yaw.sin(),
            self.target[1] + self.distance * self.pitch.sin(),
            self.target[2] + self.distance * cp * self.yaw.cos(),
        ]
    }
}

// ── Linear algebra helpers ───────────────────────────────────────────────────

mod math3d {
    /// Perspective projection matrix (WebGPU clip-space z ∈ [0, 1]).
    pub fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
        let f = 1.0 / (fov * 0.5).tan();
        let nf = 1.0 / (near - far);
        let mut m = [0.0f32; 16];
        m[0] = f / aspect;
        m[5] = f;
        m[10] = far * nf;
        m[11] = -1.0;
        m[14] = near * far * nf;
        m
    }

    /// Look-at view matrix (column-major, right-handed).
    pub fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [f32; 16] {
        let fwd = normalise([
            target[0] - eye[0],
            target[1] - eye[1],
            target[2] - eye[2],
        ]);
        let side = normalise(cross(fwd, up));
        let u = cross(side, fwd);
        let mut m = [0.0f32; 16];
        m[0] = side[0];
        m[4] = side[1];
        m[8] = side[2];
        m[1] = u[0];
        m[5] = u[1];
        m[9] = u[2];
        m[2] = -fwd[0];
        m[6] = -fwd[1];
        m[10] = -fwd[2];
        m[12] = -dot(side, eye);
        m[13] = -dot(u, eye);
        m[14] = dot(fwd, eye);
        m[15] = 1.0;
        m
    }

    /// Column-major 4×4 matrix multiply: out = a · b.
    pub fn mul(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
        let mut out = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut s = 0.0;
                for k in 0..4 {
                    s += a[k * 4 + row] * b[col * 4 + k];
                }
                out[col * 4 + row] = s;
            }
        }
        out
    }

    fn normalise(v: [f32; 3]) -> [f32; 3] {
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if len < 1e-10 {
            return [0.0, 0.0, 1.0];
        }
        [v[0] / len, v[1] / len, v[2] / len]
    }

    fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
}

// ── JS / WebGPU interop helpers ──────────────────────────────────────────────

fn obj() -> Object {
    Object::new()
}

fn set(o: &Object, key: &str, val: &JsValue) {
    Reflect::set(o, &JsValue::from_str(key), val).ok();
}

fn js_str(s: &str) -> JsValue {
    JsValue::from_str(s)
}

fn js_f64(v: f64) -> JsValue {
    JsValue::from_f64(v)
}

fn preferred_format() -> String {
    let nav: JsValue = web_sys::window().unwrap().navigator().into();
    let gpu = Reflect::get(&nav, &js_str("gpu")).unwrap_or(JsValue::UNDEFINED);
    if gpu.is_undefined() {
        return "bgra8unorm".into();
    }
    let fmt = Reflect::get(&gpu, &js_str("getPreferredCanvasFormat"))
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok())
        .and_then(|f| f.call0(&gpu).ok())
        .and_then(|v| v.as_string());
    fmt.unwrap_or_else(|| "bgra8unorm".into())
}

fn write_buffer(device: &GpuDevice, buf: &GpuBuffer, data: &[f32]) {
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4) };
    let uint8 = js_sys::Uint8Array::from(bytes);
    let _ = device
        .queue()
        .write_buffer_with_u32_and_u8_array(buf, 0, &uint8);
}

fn create_buf(device: &GpuDevice, size: usize, usage: u32) -> GpuBuffer {
    let desc = obj();
    set(&desc, "size", &js_f64(size as f64));
    set(&desc, "usage", &js_f64(usage as f64));
    device
        .create_buffer(&web_sys::GpuBufferDescriptor::from(JsValue::from(desc)))
        .expect("create_buffer")
}

fn create_buf_init(device: &GpuDevice, data: &[f32], usage: u32) -> GpuBuffer {
    let buf = create_buf(device, data.len() * 4, usage | USAGE_COPY_DST);
    write_buffer(device, &buf, data);
    buf
}

fn make_shader(device: &GpuDevice, code: &str) -> web_sys::GpuShaderModule {
    let desc = web_sys::GpuShaderModuleDescriptor::new(code);
    device.create_shader_module(&desc)
}

/// Call a method on a JS object with 0–3 arguments via Reflect.
fn js_call(target: &JsValue, method: &str, args: &[&JsValue]) {
    let Ok(f_val) = Reflect::get(target, &js_str(method)) else {
        return;
    };
    let Ok(f) = f_val.dyn_into::<Function>() else {
        return;
    };
    match args.len() {
        0 => { f.call0(target).ok(); }
        1 => { f.call1(target, args[0]).ok(); }
        2 => { f.call2(target, args[0], args[1]).ok(); }
        3 => { f.call3(target, args[0], args[1], args[2]).ok(); }
        _ => {}
    }
}

// ── GPU resource bundle ──────────────────────────────────────────────────────

struct GpuResources {
    device: GpuDevice,
    ctx: GpuCanvasContext,
    format: String,
    edge_pipeline: GpuRenderPipeline,
    bind_group: JsValue,
    cam_buf: GpuBuffer,
    palette_buf: GpuBuffer,
    quad_buf: GpuBuffer,
    canvas_w: u32,
    canvas_h: u32,
}

struct RenderState {
    gpu: GpuResources,
    layout: Layout3D,
    camera: Camera,
    edge_buf: GpuBuffer,
    edge_count: u32,
}

// ── GPU initialisation ───────────────────────────────────────────────────────

async fn init_gpu(canvas: HtmlCanvasElement) -> Result<GpuResources, String> {
    let nav: JsValue = web_sys::window().unwrap().navigator().into();
    let gpu = Reflect::get(&nav, &js_str("gpu")).map_err(|_| "No navigator.gpu")?;
    if gpu.is_undefined() {
        return Err("WebGPU not supported".into());
    }

    // Request adapter
    let adapter_promise = Reflect::get(&gpu, &js_str("requestAdapter"))
        .and_then(|f| f.dyn_into::<Function>())
        .map_err(|_| "requestAdapter missing")?
        .call0(&gpu)
        .map_err(|_| "requestAdapter call failed")?;
    let adapter = JsFuture::from(js_sys::Promise::from(adapter_promise))
        .await
        .map_err(|_| "adapter request failed")?;
    if adapter.is_null() || adapter.is_undefined() {
        return Err("No GPU adapter".into());
    }

    // Request device
    let device_promise = Reflect::get(&adapter, &js_str("requestDevice"))
        .and_then(|f| f.dyn_into::<Function>())
        .map_err(|_| "requestDevice missing")?
        .call0(&adapter)
        .map_err(|_| "requestDevice call failed")?;
    let device_js = JsFuture::from(js_sys::Promise::from(device_promise))
        .await
        .map_err(|_| "device request failed")?;
    let device: GpuDevice = device_js.dyn_into().map_err(|_| "device cast failed")?;

    // Configure canvas
    let format = preferred_format();
    let canvas_js: JsValue = canvas.clone().into();
    let ctx_js = Reflect::get(&canvas_js, &js_str("getContext"))
        .and_then(|f| f.dyn_into::<Function>())
        .map_err(|_| "getContext missing")?
        .call1(&canvas_js, &js_str("webgpu"))
        .map_err(|_| "getContext call failed")?;
    let ctx: GpuCanvasContext = ctx_js.dyn_into().map_err(|_| "ctx cast failed")?;
    let cfg = obj();
    set(&cfg, "device", &device.clone().into());
    set(&cfg, "format", &js_str(&format));
    set(&cfg, "alphaMode", &js_str("opaque"));
    js_call(&ctx.clone().into(), "configure", &[&JsValue::from(cfg)]);

    let canvas_w = canvas.width();
    let canvas_h = canvas.height();

    // ── Shader ──
    let shader_code = include_str!("graph3d_edge.wgsl");
    let shader = make_shader(&device, shader_code);

    // ── Bind group layout: camera(0) + palette(1) ──
    let bgl_entry0 = obj();
    set(&bgl_entry0, "binding", &js_f64(0.0));
    set(&bgl_entry0, "visibility", &js_f64(3.0)); // VERTEX | FRAGMENT
    let buf_type0 = obj();
    set(&buf_type0, "type", &js_str("uniform"));
    set(&bgl_entry0, "buffer", &JsValue::from(buf_type0));

    let bgl_entry1 = obj();
    set(&bgl_entry1, "binding", &js_f64(1.0));
    set(&bgl_entry1, "visibility", &js_f64(2.0)); // FRAGMENT
    let buf_type1 = obj();
    set(&buf_type1, "type", &js_str("uniform"));
    set(&bgl_entry1, "buffer", &JsValue::from(buf_type1));

    let bgl_entries = Array::new();
    bgl_entries.push(&JsValue::from(bgl_entry0));
    bgl_entries.push(&JsValue::from(bgl_entry1));
    let bgl_desc = obj();
    set(&bgl_desc, "entries", &JsValue::from(bgl_entries));
    let bgl_js = Reflect::get(&device.clone().into(), &js_str("createBindGroupLayout"))
        .and_then(|f| f.dyn_into::<Function>())
        .map_err(|_| "createBindGroupLayout")?
        .call1(&device.clone().into(), &JsValue::from(bgl_desc))
        .map_err(|_| "bgl call")?;

    // ── Pipeline layout ──
    let pl_bgls = Array::new();
    pl_bgls.push(&bgl_js);
    let pl_desc = obj();
    set(&pl_desc, "bindGroupLayouts", &JsValue::from(pl_bgls));
    let pipeline_layout = Reflect::get(&device.clone().into(), &js_str("createPipelineLayout"))
        .and_then(|f| f.dyn_into::<Function>())
        .map_err(|_| "createPipelineLayout")?
        .call1(&device.clone().into(), &JsValue::from(pl_desc))
        .map_err(|_| "pl call")?;

    // ── Edge pipeline ──
    // Quad vertex buffer layout (per-vertex, location 0)
    let quad_attr = obj();
    set(&quad_attr, "format", &js_str("float32x2"));
    set(&quad_attr, "offset", &js_f64(0.0));
    set(&quad_attr, "shaderLocation", &js_f64(0.0));
    let quad_attrs = Array::new();
    quad_attrs.push(&JsValue::from(quad_attr));
    let quad_layout = obj();
    set(&quad_layout, "arrayStride", &js_f64(8.0));
    set(&quad_layout, "stepMode", &js_str("vertex"));
    set(&quad_layout, "attributes", &JsValue::from(quad_attrs));

    // Instance buffer layout (per-instance, locations 6..10)
    let inst_attrs = Array::new();
    let locs: &[(u32, &str, f64)] = &[
        (6, "float32x3", 0.0),   // posA
        (7, "float32x3", 12.0),  // posB
        (8, "float32x4", 24.0),  // color
        (9, "float32", 40.0),    // flags
        (10, "float32", 44.0),   // edgeType
    ];
    for &(loc, fmt, offset) in locs {
        let a = obj();
        set(&a, "format", &js_str(fmt));
        set(&a, "offset", &js_f64(offset));
        set(&a, "shaderLocation", &js_f64(loc as f64));
        inst_attrs.push(&JsValue::from(a));
    }
    let inst_layout = obj();
    set(&inst_layout, "arrayStride", &js_f64(48.0));
    set(&inst_layout, "stepMode", &js_str("instance"));
    set(&inst_layout, "attributes", &JsValue::from(inst_attrs));

    let vert_bufs = Array::new();
    vert_bufs.push(&JsValue::from(quad_layout));
    vert_bufs.push(&JsValue::from(inst_layout));

    let vertex_state = obj();
    set(&vertex_state, "module", &shader.clone().into());
    set(&vertex_state, "entryPoint", &js_str("vs_edge"));
    set(&vertex_state, "buffers", &JsValue::from(vert_bufs));

    // Fragment state with premultiplied alpha blend
    let blend_comp = obj();
    set(&blend_comp, "srcFactor", &js_str("one"));
    set(&blend_comp, "dstFactor", &js_str("one-minus-src-alpha"));
    let blend = obj();
    set(&blend, "color", &JsValue::from(blend_comp.clone()));
    set(&blend, "alpha", &JsValue::from(blend_comp));
    let target0 = obj();
    set(&target0, "format", &js_str(&format));
    set(&target0, "blend", &JsValue::from(blend));
    let targets = Array::new();
    targets.push(&JsValue::from(target0));

    let frag_state = obj();
    set(&frag_state, "module", &shader.into());
    set(&frag_state, "entryPoint", &js_str("fs_edge"));
    set(&frag_state, "targets", &JsValue::from(targets));

    let primitive = obj();
    set(&primitive, "topology", &js_str("triangle-strip"));

    let pipe_desc = obj();
    set(&pipe_desc, "layout", &pipeline_layout);
    set(&pipe_desc, "vertex", &JsValue::from(vertex_state));
    set(&pipe_desc, "fragment", &JsValue::from(frag_state));
    set(&pipe_desc, "primitive", &JsValue::from(primitive));

    let pipe_js = Reflect::get(&device.clone().into(), &js_str("createRenderPipeline"))
        .and_then(|f| f.dyn_into::<Function>())
        .map_err(|_| "createRenderPipeline")?
        .call1(&device.clone().into(), &JsValue::from(pipe_desc))
        .map_err(|_| "edge pipeline call")?;
    let edge_pipeline: GpuRenderPipeline =
        pipe_js.dyn_into().map_err(|_| "edge pipeline cast")?;

    // ── Uniform buffers ──
    let cam_buf = create_buf(
        &device,
        CAM_UNIFORM_FLOATS * 4,
        USAGE_UNIFORM | USAGE_COPY_DST,
    );
    let palette_buf = create_buf(
        &device,
        PALETTE_FLOATS * 4,
        USAGE_UNIFORM | USAGE_COPY_DST,
    );
    // Zero-fill the palette (unused by edge shader but binding must be valid)
    write_buffer(&device, &palette_buf, &vec![0.0f32; PALETTE_FLOATS]);

    // ── Bind group ──
    let bg_entry0 = obj();
    set(&bg_entry0, "binding", &js_f64(0.0));
    let res0 = obj();
    set(&res0, "buffer", &cam_buf.clone().into());
    set(&bg_entry0, "resource", &JsValue::from(res0));

    let bg_entry1 = obj();
    set(&bg_entry1, "binding", &js_f64(1.0));
    let res1 = obj();
    set(&res1, "buffer", &palette_buf.clone().into());
    set(&bg_entry1, "resource", &JsValue::from(res1));

    let bg_entries = Array::new();
    bg_entries.push(&JsValue::from(bg_entry0));
    bg_entries.push(&JsValue::from(bg_entry1));
    let bg_desc = obj();
    set(&bg_desc, "layout", &bgl_js);
    set(&bg_desc, "entries", &JsValue::from(bg_entries));
    let bind_group = Reflect::get(&device.clone().into(), &js_str("createBindGroup"))
        .and_then(|f| f.dyn_into::<Function>())
        .map_err(|_| "createBindGroup")?
        .call1(&device.clone().into(), &JsValue::from(bg_desc))
        .map_err(|_| "bg call")?;

    // ── Quad vertex buffer (triangle-strip: 4 verts) ──
    let quad_data: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
    let quad_buf = create_buf_init(&device, &quad_data, USAGE_VERTEX);

    Ok(GpuResources {
        device,
        ctx,
        format,
        edge_pipeline,
        bind_group,
        cam_buf,
        palette_buf,
        quad_buf,
        canvas_w,
        canvas_h,
    })
}

// ── Screen projection ────────────────────────────────────────────────────────

struct ScreenPos {
    x: f32,
    y: f32,
    #[allow(dead_code)]
    z: f32,
    visible: bool,
}

/// Project a 3-D world position to screen pixels using the view-projection
/// matrix (column-major).  Returns pixel coordinates with (0,0) at top-left.
fn world_to_screen(pos: [f32; 3], vp: &[f32; 16], vw: f32, vh: f32) -> ScreenPos {
    let x = vp[0] * pos[0] + vp[4] * pos[1] + vp[8] * pos[2] + vp[12];
    let y = vp[1] * pos[0] + vp[5] * pos[1] + vp[9] * pos[2] + vp[13];
    let z = vp[2] * pos[0] + vp[6] * pos[1] + vp[10] * pos[2] + vp[14];
    let w = vp[3] * pos[0] + vp[7] * pos[1] + vp[11] * pos[2] + vp[15];
    if w <= 0.001 {
        return ScreenPos { x: 0.0, y: 0.0, z: 0.0, visible: false };
    }
    let ndc_x = x / w;
    let ndc_y = y / w;
    let ndc_z = z / w;
    let sx = (ndc_x + 1.0) * 0.5 * vw;
    let sy = (1.0 - ndc_y) * 0.5 * vh; // flip Y
    ScreenPos {
        x: sx,
        y: sy,
        z: ndc_z,
        visible: ndc_z >= 0.0 && ndc_z <= 1.0,
    }
}

// ── DOM node positioning (called each frame) ─────────────────────────────────

fn position_dom_nodes(state: &RenderState, vw: f32, vh: f32) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let eye = state.camera.eye();
    let aspect = vw / vh.max(1.0);
    let proj = math3d::perspective(CAMERA_FOV, aspect, CAMERA_NEAR, CAMERA_FAR);
    let view = math3d::look_at(eye, state.camera.target, [0.0, 1.0, 0.0]);
    let view_proj = math3d::mul(proj, view);

    let Ok(node_list) = doc.query_selector_all("[data-node-idx]") else {
        return;
    };
    for i in 0..node_list.length() {
        let Some(el) = node_list.item(i) else { continue };
        let Ok(html_el) = el.dyn_into::<HtmlElement>() else {
            continue;
        };
        let Some(idx_str) = html_el.get_attribute("data-node-idx") else {
            continue;
        };
        let Ok(idx) = idx_str.parse::<usize>() else {
            continue;
        };
        let Some(node) = state.layout.nodes.get(idx) else {
            continue;
        };

        let screen = world_to_screen([node.x, node.y, node.z], &view_proj, vw, vh);

        // Depth-based scale: cards shrink with distance
        let dx = eye[0] - node.x;
        let dy = eye[1] - node.y;
        let dz = eye[2] - node.z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(0.1);
        let pixel_scale = (15.0 / dist).clamp(0.15, 2.5);

        // Culling
        let margin = 300.0;
        if !screen.visible
            || screen.x < -margin
            || screen.x > vw + margin
            || screen.y < -margin
            || screen.y > vh + margin
            || pixel_scale < 0.08
        {
            let _ = html_el.style().set_property("display", "none");
            continue;
        }

        let _ = html_el.style().set_property("display", "");
        let z_idx = ((1.0 - screen.z) * 10000.0) as i32;
        let _ = html_el
            .style()
            .set_property("z-index", &z_idx.to_string());

        let transform = format!(
            "translate(-50%, -50%) translate({:.1}px, {:.1}px) scale({:.3})",
            screen.x, screen.y, pixel_scale,
        );
        let _ = html_el.style().set_property("transform", &transform);
    }
}

// ── Render one frame ─────────────────────────────────────────────────────────

fn render_frame(state: &mut RenderState) {
    let gpu = &state.gpu;

    // Resize canvas if needed
    let canvas: HtmlCanvasElement = gpu.ctx.canvas()
        .dyn_into()
        .unwrap();
    let w = canvas.client_width().max(1) as u32;
    let h = canvas.client_height().max(1) as u32;
    if w != gpu.canvas_w || h != gpu.canvas_h {
        canvas.set_width(w);
        canvas.set_height(h);
    }

    // Build camera uniform
    let eye = state.camera.eye();
    let aspect = w as f32 / h.max(1) as f32;
    let proj = math3d::perspective(CAMERA_FOV, aspect, CAMERA_NEAR, CAMERA_FAR);
    let view = math3d::look_at(eye, state.camera.target, [0.0, 1.0, 0.0]);
    let view_proj = math3d::mul(proj, view);

    let time = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now() as f32 / 1000.0)
        .unwrap_or(0.0);

    let mut cam_data = [0.0f32; CAM_UNIFORM_FLOATS];
    cam_data[..16].copy_from_slice(&view_proj);
    cam_data[16] = eye[0];
    cam_data[17] = eye[1];
    cam_data[18] = eye[2];
    cam_data[19] = 1.0;
    cam_data[20] = time;
    write_buffer(&gpu.device, &gpu.cam_buf, &cam_data);

    // Get current texture view
    let Ok(tex) = gpu.ctx.get_current_texture() else { return };
    let tex_js: JsValue = tex.into();
    let tv_fn = Reflect::get(&tex_js, &js_str("createView"))
        .ok()
        .and_then(|f| f.dyn_into::<Function>().ok());
    let Some(tv_fn) = tv_fn else { return };
    let tex_view = tv_fn.call0(&tex_js).unwrap_or(JsValue::UNDEFINED);

    // Render pass descriptor
    let clear_val = obj();
    set(&clear_val, "r", &js_f64(0.05));
    set(&clear_val, "g", &js_f64(0.05));
    set(&clear_val, "b", &js_f64(0.07));
    set(&clear_val, "a", &js_f64(1.0));
    let color_att = obj();
    set(&color_att, "view", &tex_view);
    set(&color_att, "clearValue", &JsValue::from(clear_val));
    set(&color_att, "loadOp", &js_str("clear"));
    set(&color_att, "storeOp", &js_str("store"));
    let color_atts = Array::new();
    color_atts.push(&JsValue::from(color_att));
    let rp_desc = obj();
    set(&rp_desc, "colorAttachments", &JsValue::from(color_atts));

    // Create command encoder + begin pass
    let encoder = gpu.device.create_command_encoder();
    let encoder_js: JsValue = encoder.into();

    let pass_desc = web_sys::GpuRenderPassDescriptor::from(JsValue::from(rp_desc));
    let enc_typed: web_sys::GpuCommandEncoder = encoder_js.clone().dyn_into().unwrap();
    let Ok(pass_enc) = enc_typed.begin_render_pass(&pass_desc) else { return };
    let pass: JsValue = JsValue::from(pass_enc);

    // Draw edges
    if state.edge_count > 0 {
        js_call(&pass, "setPipeline", &[&gpu.edge_pipeline.clone().into()]);
        js_call(
            &pass,
            "setBindGroup",
            &[&js_f64(0.0), &gpu.bind_group],
        );
        js_call(
            &pass,
            "setVertexBuffer",
            &[&js_f64(0.0), &gpu.quad_buf.clone().into()],
        );
        js_call(
            &pass,
            "setVertexBuffer",
            &[&js_f64(1.0), &state.edge_buf.clone().into()],
        );
        js_call(
            &pass,
            "draw",
            &[&js_f64(4.0), &js_f64(state.edge_count as f64)],
        );
    }

    js_call(&pass, "end", &[]);

    // Submit
    let cmd_buf: JsValue = Reflect::get(&encoder_js, &js_str("finish"))
        .and_then(|f| f.dyn_into::<Function>())
        .ok()
        .and_then(|f| f.call0(&encoder_js).ok())
        .unwrap_or(JsValue::UNDEFINED);
    let bufs = Array::new();
    bufs.push(&cmd_buf);
    js_call(&gpu.device.queue().into(), "submit", &[&JsValue::from(bufs)]);

    // Position DOM nodes
    position_dom_nodes(state, w as f32, h as f32);
}

// ── WebGPU feature detection ─────────────────────────────────────────────────

pub fn can_use_webgpu() -> bool {
    web_sys::window()
        .map(|w| {
            let nav: JsValue = w.navigator().into();
            let gpu =
                Reflect::get(&nav, &js_str("gpu")).unwrap_or(JsValue::UNDEFINED);
            !gpu.is_undefined()
        })
        .unwrap_or(false)
}

// ── Mouse interaction state ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct MouseState {
    orbiting: bool,
    panning: bool,
    last_x: f64,
    last_y: f64,
}

// ── Graph3D component ────────────────────────────────────────────────────────

#[derive(Props, Clone, PartialEq)]
pub struct Graph3DProps {
    pub workspace: String,
    pub root_id: String,
    pub on_select: EventHandler<String>,
}

#[component]
pub fn Graph3D(props: Graph3DProps) -> Element {
    let workspace = props.workspace.clone();
    let root_id = props.root_id.clone();
    let on_select = props.on_select;

    // Signals
    let mut status: Signal<String> = use_signal(|| "Initialising WebGPU\u{2026}".to_string());
    let layout_sig: Signal<Option<Layout3D>> = use_signal(|| None);
    let alive: Signal<Option<()>> = use_signal(|| Some(()));
    // Event listeners kept alive for the component's lifetime (auto-removed on drop)
    let _listeners: Signal<Vec<EventListener>> = use_signal(Vec::new);

    // Shared render state (accessed by rAF loop + event listeners)
    let render_rc: Signal<Option<Rc<RefCell<RenderState>>>> = use_signal(|| None);

    // ── Async initialisation ─────────────────────────────────────────────
    use_effect(move || {
        let ws = workspace.clone();
        let rid = root_id.clone();
        let mut status_w = status;
        let mut layout_w = layout_sig;
        let mut render_w = render_rc;
        let mut listeners_w = _listeners;
        let alive_r = alive;

        spawn(async move {
            // 1. Find canvas
            let doc = web_sys::window().unwrap().document().unwrap();
            let canvas: HtmlCanvasElement = match doc.get_element_by_id("webgpu-canvas") {
                Some(el) => match el.dyn_into() {
                    Ok(c) => c,
                    Err(_) => {
                        status_w.set("Canvas element not found".into());
                        return;
                    }
                },
                None => {
                    status_w.set("No #webgpu-canvas".into());
                    return;
                }
            };

            // Match the canvas pixel size to CSS size
            canvas.set_width(canvas.client_width().max(1) as u32);
            canvas.set_height(canvas.client_height().max(1) as u32);

            // 2. Init GPU
            status_w.set("Requesting GPU device\u{2026}".into());
            let gpu = match init_gpu(canvas).await {
                Ok(g) => g,
                Err(e) => {
                    status_w.set(format!("GPU init failed: {e}"));
                    return;
                }
            };

            // 3. Fetch subgraph
            status_w.set("Fetching graph data\u{2026}".into());
            let backend = HttpTicketBackend::new(None);
            let resp = match backend.get_subgraph(&ws, &rid, 4).await {
                Ok(r) => r,
                Err(e) => {
                    status_w.set(format!("Fetch failed: {e}"));
                    return;
                }
            };

            // 4. Build 3D layout
            let gl = GraphLayout::build(resp.nodes, resp.edges);
            let layout = Layout3D::from_2d(gl);
            layout_w.set(Some(layout.clone()));

            // 5. Upload edge instance data
            let (edge_data, edge_count) = layout.build_edge_instances();
            let edge_buf = if edge_data.is_empty() {
                create_buf(&gpu.device, 48, USAGE_VERTEX | USAGE_COPY_DST)
            } else {
                create_buf_init(&gpu.device, &edge_data, USAGE_VERTEX)
            };

            // 6. Camera defaults — centre on graph
            let mut camera = Camera::default();
            if !layout.nodes.is_empty() {
                let n = layout.nodes.len() as f32;
                let cx: f32 = layout.nodes.iter().map(|n| n.x).sum::<f32>() / n;
                let cy: f32 = layout.nodes.iter().map(|n| n.y).sum::<f32>() / n;
                let cz: f32 = layout.nodes.iter().map(|n| n.z).sum::<f32>() / n;
                camera.target = [cx, cy, cz];
                camera.distance = (layout.nodes.len() as f32 * 1.5).clamp(12.0, 60.0);
            }

            let state_rc = Rc::new(RefCell::new(RenderState {
                gpu,
                layout,
                camera,
                edge_buf,
                edge_count,
            }));
            render_w.set(Some(state_rc.clone()));
            status_w.set(String::new());

            // 7. Set up mouse event listeners
            let ms = Rc::new(RefCell::new(MouseState::default()));
            let container = doc.get_element_by_id("graph3d-container");
            let container_target: &web_sys::EventTarget = match &container {
                Some(el) => el.as_ref(),
                None => &doc,
            };

            // mousedown (on container)
            let md = EventListener::new(container_target, "mousedown", {
                let ms = ms.clone();
                move |evt| {
                    let Some(e) = evt.dyn_ref::<web_sys::MouseEvent>() else {
                        return;
                    };
                    // Ignore clicks on ticket cards (they handle their own events)
                    if let Some(target) = e.target() {
                        if let Ok(el) = target.dyn_into::<web_sys::Element>() {
                            if el.closest("[data-node-idx]").ok().flatten().is_some() {
                                return;
                            }
                        }
                    }
                    let mut m = ms.borrow_mut();
                    m.last_x = e.client_x() as f64;
                    m.last_y = e.client_y() as f64;
                    let button = e.button();
                    if button == 2 || (button == 0 && e.shift_key()) {
                        m.panning = true;
                    } else if button == 0 {
                        m.orbiting = true;
                    }
                }
            });

            // mousemove (on document for drag continuity)
            let mm = EventListener::new(&doc, "mousemove", {
                let ms = ms.clone();
                let st = state_rc.clone();
                move |evt| {
                    let m = ms.borrow().clone();
                    if !m.orbiting && !m.panning {
                        return;
                    }
                    let Some(e) = evt.dyn_ref::<web_sys::MouseEvent>() else {
                        return;
                    };
                    let cx = e.client_x() as f64;
                    let cy = e.client_y() as f64;
                    let dx = (cx - m.last_x) as f32;
                    let dy = (cy - m.last_y) as f32;
                    ms.borrow_mut().last_x = cx;
                    ms.borrow_mut().last_y = cy;

                    let Ok(mut s) = st.try_borrow_mut() else { return };
                    if m.orbiting {
                        s.camera.yaw -= dx * 0.005;
                        s.camera.pitch =
                            (s.camera.pitch + dy * 0.005).clamp(-1.4, 1.4);
                    } else if m.panning {
                        let speed = s.camera.distance * 0.002;
                        let cos_y = s.camera.yaw.cos();
                        let sin_y = s.camera.yaw.sin();
                        s.camera.target[0] -= dx * speed * cos_y;
                        s.camera.target[1] += dy * speed;
                        s.camera.target[2] += dx * speed * sin_y;
                    }
                }
            });

            // mouseup (on document)
            let mu = EventListener::new(&doc, "mouseup", {
                let ms = ms.clone();
                move |_| {
                    let mut m = ms.borrow_mut();
                    m.orbiting = false;
                    m.panning = false;
                }
            });

            // wheel (on container, passive: false for preventDefault)
            let wh = EventListener::new_with_options(
                container_target,
                "wheel",
                gloo_events::EventListenerOptions::enable_prevent_default(),
                {
                    let st = state_rc.clone();
                    move |evt| {
                        evt.prevent_default();
                        let Some(e) = evt.dyn_ref::<web_sys::WheelEvent>() else {
                            return;
                        };
                        let delta = e.delta_y() as f32;
                        let factor = if delta < 0.0 { 0.92 } else { 1.08 };
                        if let Ok(mut s) = st.try_borrow_mut() {
                            s.camera.distance =
                                (s.camera.distance * factor).clamp(3.0, 100.0);
                        }
                    }
                },
            );

            // Prevent context menu on right-click
            let cm = EventListener::new(container_target, "contextmenu", |evt| {
                evt.prevent_default();
            });

            listeners_w.set(vec![md, mm, mu, wh, cm]);

            // 8. Start render loop
            schedule_raf(state_rc, alive_r);
        });
    });

    // ── RSX ──────────────────────────────────────────────────────────────
    let status_text = status.read().clone();
    let layout_opt = layout_sig.read().clone();
    let node_count = layout_opt.as_ref().map_or(0, |l| l.nodes.len());

    rsx! {
        div {
            id: "graph3d-container",
            style: "position: absolute; inset: 0; overflow: hidden; user-select: none; cursor: grab;",

            // DOM ticket-card layer
            div {
                id: "graph3d-nodes",
                style: "position: absolute; inset: 0; pointer-events: none;",

                if let Some(layout) = layout_opt.as_ref() {
                    for (idx, node) in layout.nodes.iter().enumerate() {
                        {
                            let node_id = node.id.clone();
                            let title = node.title.clone().unwrap_or_else(|| "Untitled".into());
                            let state_str = node.state.clone().unwrap_or_else(|| "new".into());
                            let color = ticket_card::state_color(Some(state_str.as_str()));
                            let short_id = if node_id.len() > 8 {
                                &node_id[..8]
                            } else {
                                &node_id
                            };
                            let node_id_click = node_id.clone();
                            rsx! {
                                div {
                                    key: "{node_id}",
                                    "data-node-idx": "{idx}",
                                    style: "\
                                        position: absolute; top: 0; left: 0;\
                                        pointer-events: auto;\n\
                        transform-origin: center center;\
                
                                        display: none;\
                                        width: 160px;\
                                        box-sizing: border-box;\
                                        border: 1px solid rgba(200,200,200,0.35);\
                                        border-left: 3px solid {color};\
                                        border-radius: 6px;\
                                        background: rgba(30,30,40,0.92);\
                                        backdrop-filter: blur(2px);\
                                        padding: 6px 8px;\
                                        cursor: pointer;\
                                        overflow: hidden;\
                                        font-family: sans-serif;\
                                        box-shadow: 0 2px 8px rgba(0,0,0,0.5);\
                                    ",
                                    onclick: move |evt: Event<MouseData>| {
                                        evt.stop_propagation();
                                        on_select.call(node_id_click.clone());
                                    },
                                    // Title
                                    div {
                                        style: "font-size: 12px; font-weight: 600; color: #e8e8f0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
                                        "{title}"
                                    }
                                    // State + short ID
                                    div {
                                        style: "display: flex; align-items: center; gap: 6px; margin-top: 3px;",
                                        span {
                                            style: "font-size: 10px; color: {color}; font-weight: 500;",
                                            "{state_str}"
                                        }
                                        span {
                                            style: "font-size: 9px; color: #888;",
                                            "{short_id}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Status overlay
            if !status_text.is_empty() {
                div {
                    style: "position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); color: #aaa; font-size: 14px; font-family: sans-serif; text-align: center; pointer-events: none;",
                    "{status_text}"
                }
            }

            // Camera hint
            div {
                style: "position: absolute; bottom: 12px; left: 50%; transform: translateX(-50%); font-size: 11px; color: rgba(255,255,255,0.3); font-family: sans-serif; pointer-events: none; white-space: nowrap;",
                "Left-drag: orbit \u{00b7} Right-drag: pan \u{00b7} Scroll: zoom \u{00b7} Click card: open"
            }

            // Node count badge
            if node_count > 0 {
                div {
                    style: "position: absolute; top: 12px; right: 12px; font-size: 11px; color: rgba(255,255,255,0.35); font-family: sans-serif; pointer-events: none;",
                    "{node_count} nodes"
                }
            }
        }
    }
}

// ── requestAnimationFrame loop ───────────────────────────────────────────────

fn schedule_raf(state_rc: Rc<RefCell<RenderState>>, alive: Signal<Option<()>>) {
    let cb: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let cb2 = cb.clone();
    let closure = Closure::wrap(Box::new(move || {
        // Stop if the component was unmounted
        if alive.read().is_none() {
            return;
        }
        // Render
        if let Ok(mut st) = state_rc.try_borrow_mut() {
            render_frame(&mut st);
        }
        // Schedule next frame
        if let Some(win) = web_sys::window() {
            if let Some(ref c) = *cb2.borrow() {
                let _ = win.request_animation_frame(c.as_ref().unchecked_ref());
            }
        }
    }) as Box<dyn FnMut()>);

    // Kick off
    if let Some(win) = web_sys::window() {
        let _ = win.request_animation_frame(closure.as_ref().unchecked_ref());
    }
    *cb.borrow_mut() = Some(closure);
}
