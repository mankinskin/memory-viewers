# [AOH][Research] Container BaaS — Podman/Docker, GPU Passthrough, bollard, Network Namespaces

## Context

**ADR-1 (revised 2026-04-09):** MicroVM approach (cloud-hypervisor/Firecracker) ruled out for browser workloads — no virtio-gpu support. Container-based Browser-as-a-Service (BaaS) is the adopted strategy.

**Core architecture**: Each agent session gets a dedicated container. The container runs the agent toolchain + Chromium headless with full GPU acceleration. The orchestrator manages container lifecycle in Rust via `bollard`.

---

## Research Questions

1. What is the minimal container image that provides: Rust toolchain, git, Chromium headless, and cargo registry access?
2. How is GPU passthrough configured for Docker/Podman on WSL2 (Windows dev machine) and Linux CI?
3. How do we enforce per-container network namespace isolation with an allow-list proxy?
4. What is the cold-start latency for a pre-pulled container image? Target: <3s from `docker run` to agent prompt.
5. What bollard API calls cover the full container lifecycle we need?
6. Can we snapshot/restore a container mid-session for session revival? (`docker commit` or overlay FS checkpoint)
7. How does conmon-rs (Podman's Rust container monitor) compare to Docker daemon for programmatic control?
8. How do we inject secrets (Copilot API key, git credentials) without leaking them to `docker inspect` or logs?

---

## Architecture: Browser-as-a-Service (BaaS)

```
Orchestrator (Rust)
  bollard client
      │
      ▼
Docker/Podman daemon
  ├── container-{session-id}-agent   # runs agent implementation tasks
  │     ├── Rust toolchain + cargo
  │     ├── git (bound to host worktree via bind-mount)
  │     ├── MCP client tools
  │     └── Network: isolated namespace + allow-list proxy
  │
  └── container-{session-id}-browser # Chromium headless (optional, separate)
        ├── Chromium + playwright
        ├── GPU: --gpus all / --device /dev/dri
        └── Network: shared namespace with agent container OR separate
```

**Single-container option**: Agent + browser in same container. Simpler; slightly larger image.  
**Split-container option**: Agent tools and browser in separate containers. Better resource isolation but adds IPC complexity (e.g., Playwright remote debugging port).

---

## GPU Passthrough Configuration

### Windows (Docker Desktop + WSL2)
```bash
# NVIDIA GPU (CUDA via WSL2):
docker run --gpus all ...

# Mesa/VirGL (software + GPU via WDDM2 on WSL2):
docker run --device /dev/dxg ...  # DirectX bridge for WSL2
```

Chromium flags:
```bash
chromium --headless --use-gl=angle --use-angle=d3d11 --no-sandbox
```

### Linux (CI / dev on Linux)
```bash
# NVIDIA:
docker run --gpus all --env NVIDIA_DRIVER_CAPABILITIES=compute,utility,display ...

# Mesa/EGL (AMD/Intel/NVIDIA open):
docker run --device /dev/dri/card0 --device /dev/dri/renderD128 ...
```

Chromium flags:
```bash
chromium --headless --use-gl=egl --disable-dev-shm-usage --no-sandbox
```

### Graceful Degradation
If no GPU is available (pure CPU on CI):
```bash
chromium --headless --use-gl=swiftshader --disable-gpu
```
SwiftShader software renderer — slower but correct. Acceptable for screenshot capture and form-fill tasks; not for GPU-heavy rendering.

---

## Network Namespace Isolation

### Per-container network with allow-list proxy

```
agent-container ──▶ allow-list proxy (squid / mitmproxy / custom Rust proxy) ──▶ internet
                     allows: cargo.io, github.com, api.githubcopilot.com
                     blocks: everything else
```

Implementation:
1. Create isolated Docker network per session: `docker network create aoh-{session-id} --internal`
2. Attach allow-list proxy container to that network and to host network
3. Set `HTTP_PROXY`/`HTTPS_PROXY` env vars in agent container pointing to proxy
4. Proxy enforces allow-list via domain rules

**Rust-native proxy option**: Build a minimal tokio-based CONNECT proxy that only allows-listed domains. No external dependency.

### Alternative: iptables/nftables rules on host
```bash
# Block all outbound except specific IPs for session UID
iptables -I OUTPUT -m owner --uid-owner {session-uid} ! -d {allowed-cidr} -j DROP
```
Simpler but requires elevated host permissions.

---

## bollard API (Rust Docker Client)

Relevant bollard operations for AOH:

```rust
use bollard::Docker;
use bollard::container::{CreateContainerOptions, Config, StartContainerOptions};
use bollard::models::{HostConfig, DeviceRequest};

// Connect to daemon
let docker = Docker::connect_with_local_defaults()?;

// Create container with GPU + bind-mount
let config = Config {
    image: Some("aoh-agent:latest"),
    host_config: Some(HostConfig {
        binds: Some(vec![
            format!("{worktree_path}:/workspace:rw"),
        ]),
        device_requests: Some(vec![DeviceRequest {
            driver: Some("".into()),
            count: Some(-1),  // all GPUs
            capabilities: Some(vec![vec!["gpu".into()]]),
            ..Default::default()
        }]),
        network_mode: Some(format!("aoh-{session_id}")),
        ..Default::default()
    }),
    env: Some(vec![
        format!("COPILOT_TOKEN={token}"),  // injected via Docker secret, not env var in prod
    ]),
    ..Default::default()
};
docker.create_container(Some(CreateContainerOptions { name: &session_name, .. }), config).await?;
docker.start_container(&session_name, None::<StartContainerOptions<String>>).await?;
```

### Full lifecycle operations needed:
- `create_container` → `start_container` → `exec_create` + `exec_start` (run commands)
- `stats` (resource monitoring for cost watchdog)
- `logs` (stream to log-viewer)
- `pause_container` / `unpause_container` (for budget soft-limit suspension)
- `commit` (snapshot for revival) OR volume-backup approach
- `stop_container` → `remove_container` (cleanup)
- `create_network` / `remove_network` (per-session isolation)

---

## Podman vs Docker

| Feature | Docker (bollard) | Podman |
|---|---|---|
| Rust client crate | `bollard` ✓ active | API-compatible with Docker; bollard works via socket path |
| Rootless containers | Limited (rootful default) | Native rootless ✓ |
| GPU passthrough | Yes (NVIDIA CTK) | Yes (CDI — Container Device Interface) |
| Windows support | Docker Desktop ✓ | Limited (Podman Desktop experimental) |
| Daemon | Docker daemon required | Daemonless (conmon-rs per container) |
| Remote API | Docker REST API | Docker-compatible REST API |

**Recommendation**: Use bollard against Docker daemon for Windows dev compatibility. Design the client layer as `ContainerRuntime` trait so Podman can substitute on Linux CI.

---

## Secret Injection (Security)

**Never** pass secrets via `--env` flags or environment variables in `docker run` — visible in `docker inspect`.

Options:
1. **Docker Secrets** (Swarm mode): not available without Swarm  
2. **tmpfs-mounted secret file**: mount secret as `tmpfs` volume, read once, zero on read  
3. **In-container secret server**: orchestrator exposes a localhost-only HTTP endpoint that the container calls once to retrieve its credentials; HMAC-signed, single-use token  
4. **Build-time secret** (`docker buildx --secret`): only for build-time credentials, not runtime

**Adopted approach**: Option 3 — orchestrator runs a per-session secret server on a Unix socket or loopback port. Container calls `GET /secret/{one-time-token}` to retrieve credentials. Token expires after 30 seconds.

---

## Session Revival via Container Snapshot

```bash
# On session completion/failure — snapshot container filesystem:
docker commit container-{session-id} aoh-snapshot:{session-id}

# On revival — restore from snapshot:
docker run --name container-{session-id}-revived aoh-snapshot:{session-id} ...
```

**Limitation**: `docker commit` snapshots FS but not running process state. Agent cannot be "paused mid-thought". Revival uses summary injection (ADR-9) to re-create context.

**Alternative**: Use named volumes for workspace data and re-attach to a fresh container with the same volume.

---

## Minimum Container Image Design

Base: `ubuntu:22.04-minimal` (~80MB compressed)

Layers:
1. Base: git, curl, ca-certificates (~40MB)
2. Rust toolchain: rustup + stable (~600MB — heavy but cacheable)
3. Chromium: `chromium-browser` package or `playwright` bundle (~300MB)
4. Agent tools: cargo-installed AOH MCP client tools (~50MB)

**Total**: ~1GB image. Pull once, cached on host.

**Optimize**: Use cargo-chef for reproducible Rust layer caching. Consider Alpine-based image (~600MB total) if glibc compatibility is not an issue.

---

## Resolved Decisions (locked 2026-07-11)

**Status: COMPLETE** — All research questions answered, API verified, decisions locked.

### RQ-1 (Minimal container image)
Ubuntu 22.04-minimal base (~80MB). Layers: git+curl+ca-certs (~40MB), rustup+stable (~600MB), chromium/playwright (~300MB), AOH MCP tools (~50MB). Total ~1GB, pull-once cached. Use cargo-chef for layer caching.

### RQ-2 (GPU passthrough)
- **Windows/WSL2**: `--gpus all` (NVIDIA CUDA) or `--device /dev/dxg` (DirectX bridge). Chromium flags: `--use-gl=angle --use-angle=d3d11`.
- **Linux CI**: `--gpus all` (NVIDIA) or `--device /dev/dri/card0 --device /dev/dri/renderD128` (Mesa/EGL).
- **Graceful degradation**: SwiftShader (`--use-gl=swiftshader --disable-gpu`) on pure-CPU CI.

### RQ-3 (Network isolation)
Per-session Docker network (`docker network create aoh-{session-id} --internal`) + allow-list proxy. Rust-native tokio CONNECT proxy preferred over external squid/mitmproxy.

### RQ-4 (Cold-start latency)
Target <3s for pre-pulled images. Hard ceiling <5s. Measured at implementation time.

### RQ-5 (bollard API coverage) — VERIFIED against bollard v0.20.2
All required lifecycle operations confirmed present:

| Required Operation | bollard v0.20.2 Method | Module |
|---|---|---|
| Create container | `docker.create_container()` | `container` |
| Start container | `docker.start_container()` | `container` |
| Execute command | `docker.create_exec()` + `docker.start_exec()` | `exec` |
| Resource stats | `docker.stats()` → Stream | `container` |
| Stream logs | `docker.logs()` → Stream | `container` |
| Pause/unpause | `docker.pause_container()` / `docker.unpause_container()` | `container` |
| Snapshot | `docker.commit_container()` | `image` |
| Stop container | `docker.stop_container()` | `container` |
| Remove container | `docker.remove_container()` | `container` |
| Create network | `docker.create_network()` | `network` |
| Remove network | `docker.remove_network()` | `network` |

Additional useful APIs: `inspect_container` (health checks), `wait_container` (block on exit), `kill_container` (forced termination), `attach_container` (interactive sessions).

**Connection**: `Docker::connect_with_local_defaults()` auto-detects Unix socket vs Windows named pipe. Windows named-pipe support via default `pipe` feature.

**Note**: v0.20.x uses `ContainerCreateBody` (not the older `Config`). Code samples in ticket body use older API names — update at implementation time.

### RQ-6 (Snapshot/restore)
`docker commit` snapshots FS but not process state. Revival uses summary injection (ADR-9). Alternative: named volumes for workspace data + fresh container.

### RQ-7 (Podman vs Docker)
Docker via bollard for Windows dev compatibility. `ContainerRuntime` trait abstracts backend. Podman works via API-compatible socket path with bollard. Rootless Podman documented for Linux CI.

### RQ-8 (Secret injection)
Option 3 adopted (ADR-13): orchestrator per-session secret server on Unix socket/loopback port. One-time token, 30s expiry. Never pass secrets via `--env`.

### Crate Details
- **bollard v0.20.2**: Docker Engine API v1.52, Apache-2.0 license, 27M+ downloads
- Default features: `http`, `pipe` (Windows named-pipe support)
- Optional: `ssl`, `ssh`, `buildkit`, `websocket`, `chrono`/`time`
- `Docker` struct is `Send + Sync + Clone` — safe for concurrent use

---

## Acceptance Criteria

- [ ] Container image Dockerfile drafted for Ubuntu + Rust + Chromium headless
- [ ] GPU passthrough verified on dev machine (Windows/WSL2): `chromium --headless --screenshot` produces valid PNG with GPU rendering
- [ ] bollard container lifecycle implemented: create → start → exec → stop → remove
- [ ] Per-session Docker network created and removed cleanly
- [ ] Allow-list proxy validated (3 rules: cargo.io, github.com, Copilot API)
- [ ] Secret injection via one-time token server implemented and tested
- [ ] Container snapshot + revival path documented
- [ ] Cold-start latency measured (target: <3s from create to agent-ready)
- [ ] `ContainerRuntime` trait defined (Docker impl + Podman compatiblity note)
- [ ] Podman rootless path documented for Linux CI