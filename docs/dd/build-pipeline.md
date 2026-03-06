# Design Document: Build Pipeline & Embedding

| Field        | Value                        |
|--------------|------------------------------|
| Status       | Draft                        |
| Author       | khamutov, Claude co-authored |
| Last updated | 2026-03-06                   |
| PRD ref      | 1. Initial PRD, §9.1         |
| Depends on   | DD 0.7 (Frontend Architecture) |

---

## 1. Context and Scope

PRD §9.1 requires a single statically-linked binary with the frontend embedded at build time. DD 0.7 defined the frontend tech stack (Vite, React Router, TanStack Query, CSS Modules) and specified build output: a `dist/` directory with content-hash filenames, code splitting per route, and no source maps in production (§19). DD 0.1 chose sqlx with compile-time query checking and migrations in a `migrations/` directory, run at startup. DD 0.3 defined the `create-admin` CLI subcommand.

This document decides: repository layout, how frontend assets are embedded into the Rust binary, build orchestration for dev and release, developer workflow with HMR, CLI structure, Docker build, and CI considerations.

It unblocks:

- **1.1** Initialize Cargo workspace with backend crate
- **1.2** Initialize React+TS project with Vite
- **1.5** Build pipeline: frontend build + rust-embed
- **1.6** Dev workflow: Vite proxy to backend
- **6.5** Dockerfile

## 2. Problem Statement

Before writing any code we need to decide:

- Repository layout: where Cargo.toml, frontend source, and build output live.
- Embedding strategy: how compiled frontend assets become part of the Rust binary.
- Build orchestration: a single command for release builds, separate tasks for dev.
- Dev workflow: frontend HMR with backend proxy, ideally one command.
- CLI structure: subcommands, flags, env vars, startup sequence.
- Docker build: multi-stage, layer caching, minimal runtime image.
- CI checks: lint, test, typecheck, build, bundle size.

## 3. Goals

- Single orchestrated `task build` produces a self-contained binary with embedded frontend.
- Dev workflow supports frontend HMR with backend proxy, one command.
- Conventional, discoverable repo layout.
- Minimal Docker image with efficient layer caching.
- CI catches build failures, lint violations, test failures, and bundle size regressions.

## 4. Non-goals

- Cross-compilation beyond native platform (defer to `cross` when needed).
- Monorepo tooling (nx, turborepo).
- Nix or Bazel build systems.
- Hot module replacement for Rust backend code.

## 5. Repository Layout

### Option A: `backend/` and `frontend/` siblings `[rejected]`

```
s9/
  backend/
    Cargo.toml
    src/
  frontend/
    package.json
    src/
```

**Pros:**
- Clear separation between backend and frontend.

**Cons:**
- Cargo root not at repo root — `cargo` commands require `cd backend` or `--manifest-path`.
- Awkward relative paths for rust-embed (`folder = "../frontend/dist/"`).
- Unconventional for single-binary Rust projects.

### Option B: `Cargo.toml` at root, `frontend/` subdirectory `[selected]`

```
s9/
  Cargo.toml
  Cargo.lock
  Taskfile.yml
  Dockerfile
  .gitignore
  migrations/           # sqlx SQL migrations
  src/                  # Rust backend source
    main.rs
    cli.rs
    config.rs
    db/
    api/
    auth/
    embed.rs            # static file serving
  frontend/             # React+TS project
    package.json
    tsconfig.json
    vite.config.ts
    index.html
    src/
    dist/               # gitignored build output
  docs/
  deploy/
```

**Pros:**
- `cargo build` works from repo root with no extra flags.
- rust-embed path is a clean `folder = "frontend/dist/"`.
- Matches the existing repo structure (`docs/`, `deploy/` already at root).

**Cons:**
- Root directory contains both Cargo and npm config files — mitigated by clear naming.

**Decision:** Option B. Cargo at root keeps the Rust toolchain happy and gives rust-embed a straightforward path.

## 6. Cargo Crate Structure

### Option A: Workspace with binary + library crate `[rejected]`

Split into `s9` (binary) and `s9-core` (library) crates in a workspace.

**Pros:**
- Library crate enables `#[cfg(test)]` integration tests without spinning up the binary.

**Cons:**
- Premature — axum provides `TestClient` and tower `ServiceExt` for integration testing without a workspace split.
- Adds complexity to Cargo.toml, CI, and Docker caching.

### Option B: Single binary crate `[selected]`

One `Cargo.toml` at root, `src/main.rs` as the entry point, `tests/` at crate root for integration tests.

**Pros:**
- Simple. One build target, one artifact.
- Integration tests use `axum::test_helpers` or `tower::ServiceExt::oneshot`.

**Cons:**
- If the project grows significantly, may need to split later — acceptable for v1.

**Decision:** Single crate. Split only if compile times or test ergonomics demand it.

### Key Dependencies

```toml
[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "migrate"] }
clap = { version = "4", features = ["derive", "env"] }
rust-embed = { version = "8", features = ["compression"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
utoipa = { version = "5", features = ["axum_extras"] }
argon2 = "0.5"
mime_guess = "2"
tower-http = { version = "0.6", features = ["cors", "trace"] }

[profile.release]
lto = true
strip = true
codegen-units = 1
```

## 7. Embedding Strategy

### Option A: `include_dir` `[rejected]`

**Pros:**
- Simple macro, no external crate dependency beyond `include_dir`.

**Cons:**
- No built-in compression — binary size bloats with uncompressed assets.
- No MIME type detection.
- No filesystem fallback for development.

### Option B: `rust-embed` `[selected]`

**Pros:**
- Derive macro: `#[derive(RustEmbed)] #[folder = "frontend/dist/"]`.
- Built-in gzip compression — assets stored compressed in the binary, served with `Content-Encoding: gzip`.
- Filesystem fallback in debug mode — no need to rebuild Rust when iterating on frontend.
- MIME type detection via file extension.
- Widely adopted in the Rust ecosystem.

**Cons:**
- Adds ~50 KB to compile time dependencies — negligible.

### Option C: Custom `build.rs` with `include_bytes!` `[rejected]`

**Pros:**
- No external dependency.

**Cons:**
- Reinvents compression, MIME detection, and fallback logic that rust-embed provides.
- Maintenance burden for no benefit.

**Decision:** rust-embed. Compression, MIME detection, and debug fallback out of the box.

### Configuration

```rust
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "frontend/dist/"]
struct Assets;
```

### Axum Integration

API routes are mounted first (higher priority), with static file serving as the fallback:

```rust
let app = Router::new()
    .nest("/api", api_router)
    .fallback(static_handler);
```

The `static_handler` logic:

1. Look up the request path in `Assets`.
2. If found → serve with the correct MIME type and caching headers.
3. If not found → serve `index.html` (SPA fallback for client-side routing).

Caching policy:

| Path pattern              | `Cache-Control` header                |
|---------------------------|---------------------------------------|
| Hashed assets (`*.js`, `*.css` with hash) | `public, immutable, max-age=31536000` |
| `index.html`             | `no-cache`                            |
| Other assets (favicon, etc.) | `public, max-age=3600`             |

Hashed assets are identified by a regex pattern matching Vite's `[name]-[hash].[ext]` output format.

## 8. Build Orchestration

### Option A: Makefile `[rejected]`

**Pros:**
- Universally available on Unix systems.

**Cons:**
- Tab-sensitivity causes subtle errors.
- Painful conditional logic and string manipulation.
- Poor cross-platform support (Windows).

### Option B: `cargo-make` `[rejected]`

**Pros:**
- Rust-native, integrates with Cargo.

**Cons:**
- Requires `cargo install cargo-make`.
- Verbose TOML task definitions.
- Less discoverable than YAML-based alternatives.

### Option C: `just` `[rejected]`

**Pros:**
- Clean syntax, purpose-built for project commands.
- Common in the Rust ecosystem.

**Cons:**
- Requires separate installation (`cargo install just` or package manager).
- Less common outside the Rust ecosystem — frontend developers may not have it.

### Option D: Taskfile (go-task) `[selected]`

**Pros:**
- YAML-based — familiar syntax, easy to read.
- Single binary download, cross-platform (Linux, macOS, Windows).
- Built-in dependency support (`deps:` key).
- `task --list` for discoverability.
- Widely adopted across language ecosystems.

**Cons:**
- Not Rust-native — mitigated by broad adoption and easy installation.

**Decision:** Taskfile. YAML is more accessible to the full team (Rust + frontend devs), dependency management is built-in, and `task --list` makes commands discoverable.

### Taskfile.yml Structure

```yaml
version: "3"

vars:
  FRONTEND_DIR: frontend
  CARGO_RELEASE_FLAGS: --release

tasks:
  frontend:install:
    desc: Install frontend dependencies
    dir: "{{.FRONTEND_DIR}}"
    cmds:
      - npm ci
    sources:
      - package-lock.json
    generates:
      - node_modules/.package-lock.json

  frontend:build:
    desc: Build frontend for production
    dir: "{{.FRONTEND_DIR}}"
    deps: [frontend:install]
    cmds:
      - npm run build
    sources:
      - src/**/*
      - index.html
      - vite.config.ts
      - tsconfig.json
    generates:
      - dist/**/*

  frontend:lint:
    desc: Lint frontend code
    dir: "{{.FRONTEND_DIR}}"
    deps: [frontend:install]
    cmds:
      - npm run lint
      - npm run typecheck
      - npm run format:check

  frontend:test:
    desc: Run frontend tests
    dir: "{{.FRONTEND_DIR}}"
    deps: [frontend:install]
    cmds:
      - npm test

  backend:build:
    desc: Build backend (debug)
    cmds:
      - cargo build

  backend:release:
    desc: Build release binary with embedded frontend
    deps: [frontend:build]
    cmds:
      - cargo build {{.CARGO_RELEASE_FLAGS}}

  backend:lint:
    desc: Lint Rust code
    cmds:
      - cargo fmt --check
      - cargo clippy -- -D warnings

  backend:test:
    desc: Run Rust tests
    cmds:
      - cargo test

  build:
    desc: Full release build
    cmds:
      - task: backend:release

  lint:
    desc: Lint all code
    deps: [backend:lint, frontend:lint]

  test:
    desc: Run all tests
    deps: [backend:test, frontend:test]

  dev:
    desc: Start dev servers (backend + frontend with HMR)
    cmds:
      - task: dev:backend &
      - task: dev:frontend
      # Ctrl+C kills both via process group

  dev:backend:
    desc: Start backend dev server with auto-restart
    cmds:
      - cargo watch -x 'run -- serve --listen 127.0.0.1:3000'

  dev:frontend:
    desc: Start Vite dev server
    dir: "{{.FRONTEND_DIR}}"
    deps: [frontend:install]
    cmds:
      - npm run dev

  docker:build:
    desc: Build Docker image
    cmds:
      - docker build -t s9 .

  clean:
    desc: Remove build artifacts
    cmds:
      - cargo clean
      - rm -rf frontend/dist frontend/node_modules
```

No `build.rs` for frontend — Taskfile orchestrates the sequence explicitly. This keeps builds debuggable and avoids running npm on every `cargo build`.

## 9. Dev Workflow

Two processes run concurrently:

1. **Rust backend** on `:3000` — `cargo run -- serve --listen 127.0.0.1:3000`
2. **Vite dev server** on `:5173` — proxies `/api` requests to the backend

`task dev` starts both. `Ctrl+C` kills the process group.

### Vite Proxy Configuration

```typescript
// frontend/vite.config.ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
    },
  },
  server: {
    proxy: {
      "/api": {
        target: "http://127.0.0.1:3000",
        changeOrigin: true,
      },
    },
  },
});
```

### Backend Auto-Restart

Recommend `cargo-watch` for automatic rebuilds on source changes:

```sh
cargo install cargo-watch
cargo watch -x 'run -- serve --listen 127.0.0.1:3000'
```

In debug mode, rust-embed reads from the filesystem (`frontend/dist/`) instead of the embedded binary, so backend restarts are only needed for Rust code changes — not frontend changes.

## 10. CLI Structure

Built with `clap` derive macros. Config precedence: CLI flags > env vars > defaults. No config file in v1.

### Top-Level Flags

| Flag           | Env var        | Default             | Description               |
|----------------|----------------|---------------------|---------------------------|
| `--data-dir`   | `S9_DATA_DIR`  | `./data`            | Directory for SQLite DB and attachments |
| `--listen`     | `S9_LISTEN`    | `127.0.0.1:8080`    | Bind address and port     |

### Subcommands

| Subcommand       | Description                        |
|------------------|------------------------------------|
| `serve`          | Start the HTTP server (default)    |
| `create-admin`   | Create an admin user (per DD 0.3)  |
| `migrate`        | Run pending database migrations    |

`serve` is the default when no subcommand is given.

### `create-admin` Flags

| Flag           | Required | Description            |
|----------------|----------|------------------------|
| `--login`      | Yes      | Admin username         |
| `--password`   | Yes      | Admin password         |

### Startup Sequence (serve)

1. Parse CLI args and env vars.
2. Create data directory if it doesn't exist.
3. Open SQLite connection pool.
4. Run pending migrations.
5. Clean stale temp attachment files (per DD 0.5 §10).
6. Start session cleanup background task (per DD 0.3 §9).
7. Start orphan attachment cleanup background task (per DD 0.5 §8).
8. Build axum router (API routes + static file fallback).
9. Bind to `--listen` address and serve.

## 11. Frontend Project Structure

Standard Vite + React + TypeScript project in `frontend/`.

### package.json Scripts

| Script          | Command                            |
|-----------------|------------------------------------|
| `dev`           | `vite`                             |
| `build`         | `tsc --noEmit && vite build`       |
| `test`          | `vitest run`                       |
| `lint`          | `eslint src/`                      |
| `typecheck`     | `tsc --noEmit`                     |
| `format:check`  | `prettier --check src/`            |

### TypeScript Configuration

- `strict: true`
- `@/` path alias → `src/`
- `target: ES2022`
- `moduleResolution: bundler`

## 12. Docker Build

Three-stage build: frontend builder, backend builder, runtime.

```dockerfile
# Stage 1: Frontend build
FROM node:22-alpine AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ .
RUN npm run build

# Stage 2: Backend build
FROM rust:1.85-bookworm AS backend
WORKDIR /app

# Cache Rust dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release && rm -rf src target/release/s9*

# Copy frontend build output
COPY --from=frontend /app/frontend/dist frontend/dist/

# Copy real source and build
COPY src/ src/
COPY migrations/ migrations/
ENV SQLX_OFFLINE=true
RUN cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -r -s /bin/false s9
COPY --from=backend /app/target/release/s9 /usr/local/bin/s9

USER s9
EXPOSE 8080
VOLUME /data
ENV S9_DATA_DIR=/data
ENV S9_LISTEN=0.0.0.0:8080

ENTRYPOINT ["s9"]
CMD ["serve"]
```

### Design Decisions

- **bookworm-slim over alpine:** SQLite links against glibc; musl builds require static linking workarounds that complicate the build.
- **bookworm-slim over scratch:** Need CA certificates for potential outbound HTTPS (OIDC, SMTP) and a proper `/tmp`.
- **Dummy `main.rs` trick:** Cargo builds and caches all dependencies before copying real source. Source changes only recompile the application, not dependencies.
- **`SQLX_OFFLINE=true`:** Uses the committed `.sqlx/` directory for compile-time query checking instead of requiring a live database.

## 13. CI Considerations

### Check Matrix

| Check                    | Command                          | Stage    |
|--------------------------|----------------------------------|----------|
| Rust format              | `cargo fmt --check`              | Lint     |
| Rust lint                | `cargo clippy -- -D warnings`    | Lint     |
| Rust tests               | `cargo test`                     | Test     |
| Frontend lint            | `npm run lint`                   | Lint     |
| Frontend typecheck       | `npm run typecheck`              | Lint     |
| Frontend format          | `npm run format:check`           | Lint     |
| Frontend tests           | `npm test`                       | Test     |
| Release build            | `task backend:release`           | Build    |
| Bundle size              | Check `dist/` < 200 KB gzipped JS | Build  |

All commands are CI-platform-agnostic — the Taskfile encapsulates them. CI pipelines call `task lint`, `task test`, `task build`.

### sqlx Offline Mode

`cargo sqlx prepare` generates a `.sqlx/` directory containing JSON query metadata. This directory is committed to the repository. CI and Docker builds set `SQLX_OFFLINE=true` to use these cached query descriptions instead of connecting to a live database.

Developers must run `cargo sqlx prepare` after any schema change and commit the updated `.sqlx/` directory.

## 14. Release Build

### Target Platforms

| Platform         | Priority   | Notes                          |
|------------------|------------|--------------------------------|
| Linux x86_64     | Primary    | Production servers             |
| Linux aarch64    | Primary    | ARM servers, cloud instances   |
| macOS x86_64     | Dev        | Developer machines             |
| macOS aarch64    | Dev        | Apple Silicon developer machines |

### Build Steps

1. `task frontend:build` — Vite compiles TypeScript, bundles, and outputs to `frontend/dist/`.
2. `cargo build --release` — Compiles Rust with rust-embed reading from `frontend/dist/`, applies LTO and stripping.
3. Binary at `target/release/s9`.

Expected binary size: ~15–25 MB (Rust binary + compressed frontend assets).

Cross-compilation deferred — use [`cross`](https://github.com/cross-rs/cross) when needed for non-native targets.

## 15. sqlx Compile-Time Checking

| Context     | Method                                      |
|-------------|---------------------------------------------|
| Development | Local `s9.db` with schema applied via migrations |
| CI          | `SQLX_OFFLINE=true` with committed `.sqlx/` |
| Docker      | `SQLX_OFFLINE=true` with committed `.sqlx/` |

Workflow after schema changes:

1. Write new migration in `migrations/`.
2. Run the app locally to apply the migration.
3. Run `cargo sqlx prepare` to regenerate `.sqlx/`.
4. Commit both the migration and the updated `.sqlx/` directory.

## 16. Open Questions

1. **`cargo-watch` vs `watchexec`.** Recommend `cargo-watch` — it understands Cargo projects natively and ignores `target/` by default. Document installation in the project README.

2. **CI platform choice.** Taskfile commands are CI-agnostic. GitHub Actions is the likely choice but not decided here — the Taskfile abstracts the commands.

3. **Binary release distribution (GitHub Releases).** Defer to when we have a release process. Will use `cargo build --release` per platform (or `cross`) and upload artifacts.

4. **Frontend env vars at build time.** Not needed in v1 — the frontend talks to the same origin it's served from. If needed later, Vite's `import.meta.env` mechanism is available.
