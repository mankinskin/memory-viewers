# [AOH][Research] Cloud Hypervisor / Firecracker — Dead-End Analysis for Browser Workloads

## Status: CONCLUDED — Dead End for Browser Use Case

**Finding (2026-04-09):** Both `cloud-hypervisor` and Firecracker lack `virtio-gpu` (paravirtualised GPU) support. Without GPU access inside the guest, Chromium requires software rendering (SwiftShader/LLVMpipe), which is:
- Too slow for interactive browser tasks
- Unusable for GPU-accelerated WASM rendering
- Not acceptable given our WASM client latency requirements

**Conclusion:** MicroVM sandboxing is a dead end for the browser-hosting use case. See ADR-1 revision in the design ticket (`34bc4938`).

---

## Root Cause

cloud-hypervisor device support (as of 2026):
- virtio-net ✓
- virtio-blk ✓
- virtio-fs (virtiofs) ✓
- virtio-console ✓
- **virtio-gpu ✗ — not implemented**

Firecracker device support:
- virtio-net ✓
- virtio-block ✓
- **virtio-gpu ✗ — explicitly out of scope (design tradeoff for minimal attack surface)**

QEMU supports virtio-gpu + VirGL through its `virtio-vga` device, but QEMU is not Rust-native and has a significantly larger attack surface and slower startup than cloud-hypervisor/Firecracker.

---

## Why This Matters

The AOH system requires browsers in agent sessions for:
1. UI test execution (Playwright / Puppeteer headless runs)
2. Web scraping within agent tasks
3. Screenshot capture as validation evidence (WASM client rendering verification)

A software-rendered browser inside a microVM cannot provide adequate performance for these workloads.

---

## Alternative Adopted: Container-Based Browser-as-a-Service

**Decision (ADR-1 revised):** Move to Podman/Docker container isolation with GPU passthrough.
- Podman supports `--gpus all` (NVIDIA Container Toolkit) on Linux
- Docker Desktop on Windows supports GPU passthrough via WSL2 + CUDA/WDDM2
- Containers use dedicated **Network Namespaces** per agent/browser pair for isolation
- Chromium inside container uses `--use-gl=angle` (Windows) or `--use-gl=egl` (Linux) for GPU acceleration
- Orchestration in Rust via `bollard` crate (async Docker API)

See: `[AOH][Research] Container-based Browser-as-a-Service — Podman/Docker, GPU passthrough, bollard`

---

## Preserved Finding: cloud-hypervisor for Non-Browser Sandboxing

If future agent tasks do NOT require a browser but need kernel-level isolation (e.g., running untrusted code, full OS simulation), cloud-hypervisor remains viable:
- Fast boot (~200-400ms with minimal Linux guest)
- virtiofs for worktree sharing
- Strong isolation (VM boundary)

File this as a potential **Tier 3 option** in the sandbox manager — selectable per ticket type if browser is not needed.

---

## Source Links
- cloud-hypervisor device list: https://github.com/cloud-hypervisor/cloud-hypervisor/blob/main/docs/devices.md
- Firecracker design constraints: https://github.com/firecracker-microvm/firecracker/blob/main/docs/design.md
- virtio-gpu spec: https://www.kraxel.org/blog/2019/09/display-devices-in-qemu/

## Acceptance Criteria (Concluded)

- [x] cloud-hypervisor evaluated for browser workload viability
- [x] virtio-gpu absence confirmed as blocking issue
- [x] Alternative (container BaaS) identified and documented
- [x] Residual use case for non-browser microVM isolation noted