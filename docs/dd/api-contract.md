# Design Document: Backend-Frontend API Contract

| Field        | Value                        |
|--------------|------------------------------|
| Status       | Draft                        |
| Author       | khamutov, Claude co-authored |
| Last updated | 2026-03-06                   |
| PRD ref      | 1. Initial PRD               |

---

## 1. Context and Scope

S9 is a Rust + React/TypeScript bug tracker shipped as a single embedded binary. The compiled React frontend is served from the same origin as the backend — there is no separate frontend host, no API gateway, and no reverse proxy in the default deployment.

This document decides the API contract format between the backend and frontend: how endpoints are defined, how data is serialized on the wire, and how real-time updates are delivered to the browser.

## 2. Problem Statement

Before writing any backend or frontend code we need to lock down the communication protocol. The choice affects:

- Build toolchain complexity (code generation, protobuf compiler, etc.)
- Frontend development ergonomics (DevTools debugging, type safety)
- File upload handling (attachments up to 20 MB)
- Real-time update delivery (live ticket/comment changes)
- Long-term maintainability for a small team

## 3. Goals

- Pick a single API paradigm for the v1 backend-frontend contract.
- Ensure the choice supports all PRD requirements: CRUD for tickets/comments/components/milestones, complex search queries, file attachments, real-time updates.
- Keep the build and deployment pipeline simple (single binary, no sidecar proxies).
- Provide strong type safety between backend and frontend.

## 4. Non-goals

- Designing the actual endpoint schema (that is a separate DD).
- Service-to-service communication (S9 is a monolith).
- Third-party public API stability guarantees (v1 is internal-only).

## 5. Options Considered

### Option A: gRPC-Web `[rejected]`

Use Protocol Buffers for schema definition and gRPC-Web for browser communication.

**How it would work:**

- Define `.proto` files for all services (TicketService, CommentService, etc.).
- Backend implements gRPC services in Rust via `tonic`.
- Frontend uses `grpc-web` or `connect-web` client generated from the same `.proto` files.
- Browsers cannot speak native gRPC (HTTP/2 trailers), but `tonic` supports gRPC-Web natively via its `tonic-web` middleware layer — no Envoy sidecar or Connect adapter required. The translation happens in-process.
- File uploads need a separate HTTP endpoint — gRPC has no native multipart/form-data support, so large file uploads (20 MB) would bypass the gRPC layer entirely.
- Server-push via gRPC server-streaming. `tonic-web` supports unary RPCs and server-streaming. Client-streaming and bidirectional streaming are not supported over gRPC-Web (browser limitation, not a tonic limitation).

**Pros:**

- Strong contract enforcement via `.proto` schema.
- Binary serialization (protobuf) is compact and fast to parse.
- `tonic-web` provides in-process gRPC-Web support — no external proxy needed.
- Server-streaming works over gRPC-Web for real-time push scenarios.
- Proven at scale for service-to-service communication.

**Cons:**

- **No proxy needed, but added in-process complexity.** `tonic-web` handles the gRPC-Web translation in-process, so no Envoy sidecar is required. However, the backend must run both a gRPC-Web service layer and a separate HTTP layer for file uploads — two serving paradigms in one binary.
- **File upload workaround.** gRPC has a ~4 MB default message size limit and no multipart support. Attachments would require a parallel REST endpoint, resulting in two API paradigms.
- **Build toolchain overhead.** Requires `protoc` compiler + Rust/TS codegen plugins in the build pipeline. The `protoc` binary is platform-specific and adds cross-compilation complexity.
- **Debugging opacity.** Binary payloads are not human-readable in browser DevTools without dedicated tooling.
- **Streaming limitations.** gRPC-Web supports server-streaming but not client-streaming or bidirectional streaming (browser limitation). Server-streaming offers no practical advantage over SSE for our use case (unidirectional push of ticket updates) while being more complex to set up on the frontend.
- **Ecosystem mismatch.** gRPC is designed for polyglot microservice architectures. A single-origin monolith serving a browser SPA does not benefit from its strengths.

### Option B: JSON API (REST) + SSE `[suggested]`

Use a conventional JSON-over-HTTP API with Server-Sent Events for real-time push.

**How it would work:**

- Define RESTful resource endpoints (e.g. `GET /api/tickets`, `POST /api/tickets`, `PATCH /api/tickets/:id`).
- Request and response bodies are JSON, serialized via `serde_json` on the backend and native `fetch` + TypeScript interfaces on the frontend.
- File uploads use standard `multipart/form-data` via `POST /api/attachments`.
- Complex search queries are passed as a query string parameter: `GET /api/tickets?q=owner:alex+status:new+crash`.
- Real-time updates use SSE (`EventSource` API): clients open a persistent `GET /api/events` connection and receive JSON-encoded events for ticket/comment changes.
- Type safety is achieved by maintaining an OpenAPI 3.1 spec (either hand-written or generated from Rust types via `utoipa`) and generating TypeScript types/client from it.

**Pros:**

- **Native browser support.** `fetch`, `EventSource`, `FormData` — no adapter libraries, no proxies.
- **Single-binary compatible.** axum handles JSON, multipart, and SSE natively. No sidecar process.
- **First-class Rust ecosystem support.** axum, actix-web, and warp all treat JSON endpoints and SSE as first-class citizens. `serde` is the de-facto standard. `utoipa` or `aide` generate OpenAPI specs from Rust types.
- **File uploads are trivial.** Standard `multipart/form-data` with streaming support. No message size workarounds.
- **Debuggable.** JSON payloads are human-readable in browser DevTools Network tab. SSE events are visible in the EventStream tab.
- **Simple build pipeline.** No `protoc`, no codegen plugins. TypeScript types can be generated from OpenAPI at build time or maintained manually for a small API surface.
- **Search queries map naturally.** The filter micro-syntax (`owner:alex status:new`) is a single query parameter string — RESTful and cacheable.
- **SSE covers real-time needs.** Server-push of ticket updates and notifications is unidirectional by nature (server to client). SSE is purpose-built for this. Automatic reconnection is built into the `EventSource` API.

**Cons:**

- JSON is larger on the wire than protobuf (irrelevant at this scale — ticket payloads are small, and the app is same-origin).
- OpenAPI codegen is opt-in rather than mandatory — discipline required to keep spec in sync (mitigated by generating from Rust types).
- No built-in bidirectional streaming (not needed — all mutations go through regular HTTP requests).

## 6. Evaluation

| Criterion                        | gRPC-Web                  | JSON API + SSE            |
|----------------------------------|---------------------------|---------------------------|
| Single-binary deployment         | tonic-web (in-process)    | Native, no extras         |
| Browser compatibility            | tonic-web + grpc-web client| Native fetch + EventSource|
| File uploads (20 MB)             | Separate HTTP endpoint    | Standard multipart        |
| Real-time server push            | Server-streaming (no bidi)| SSE (purpose-built)       |
| Search query ergonomics          | Protobuf message field    | Query string parameter    |
| Type safety                      | Strong (protobuf)         | Strong (OpenAPI codegen)  |
| Build toolchain complexity       | protoc + plugins          | None (or optional utoipa) |
| Debugging / DevTools             | Binary, opaque            | Human-readable JSON       |
| Wire efficiency                  | Compact binary            | Larger but negligible     |
| Rust ecosystem maturity          | tonic (solid)             | axum/serde (excellent)    |
| Future webhook/bulk ops support  | Possible                  | Natural fit               |

**Decision: Option B — JSON API (REST) + SSE.**

gRPC's strengths — binary efficiency, bidirectional streaming, polyglot service contracts — are not relevant for a browser-only CRUD application served from a single origin. While `tonic-web` eliminates the proxy requirement, gRPC-Web still requires a separate HTTP layer for file uploads, a `protoc` build toolchain, and offers no streaming advantage over SSE (bidirectional streaming is unsupported in browsers regardless).

## 7. High-Level API Shape

This is illustrative, not prescriptive. The full endpoint schema will be defined in a separate DD.

```
# Resources
GET    /api/tickets            # list/search (accepts ?q= filter syntax)
POST   /api/tickets            # create
GET    /api/tickets/:id        # read
PATCH  /api/tickets/:id        # update fields
GET    /api/tickets/:id/comments
POST   /api/tickets/:id/comments
PATCH  /api/tickets/:id/comments/:num

POST   /api/attachments        # multipart upload, returns attachment reference

GET    /api/components         # tree
GET    /api/milestones         # list

# Real-time
GET    /api/events             # SSE stream (filterable via query params)

# Auth
POST   /api/auth/login
POST   /api/auth/logout
GET    /api/auth/oidc/callback
```

All request/response bodies are `application/json` except attachment upload (`multipart/form-data`) and the event stream (`text/event-stream`).

## 8. Security Considerations

- **CORS is not required.** Frontend and backend share the same origin.
- **CSRF protection.** Since the API uses JSON bodies (not form submissions), the `Content-Type: application/json` header acts as a CSRF guard. For the multipart upload endpoint, use a session-bound CSRF token or `SameSite` cookies.
- **Authentication.** HTTP-only, Secure, SameSite=Lax session cookies. Bearer tokens for potential future API consumers.
- **SSE authentication.** The initial SSE connection is authenticated via session cookie. No token in query string (avoids logging credentials in URLs).
- **Input validation.** All JSON payloads are validated and deserialized via serde with strict typing. Reject unknown fields.
- **Rate limiting.** Applied at the HTTP layer (middleware), straightforward with axum's tower middleware stack.

## 9. Backward Compatibility and Rollout

Not applicable — this is a greenfield v1 decision. No existing API to migrate from.

For future API versioning, URL path prefixing (`/api/v1/...`) can be introduced if a breaking change is ever needed. For v1, the `/api/` prefix is sufficient.

## 10. Open Questions

1. **OpenAPI generation strategy.** Generate from Rust types (via `utoipa`) or maintain a hand-written spec? Recommendation: start with `utoipa` annotations on handler functions, generate spec at build time.
2. **SSE event granularity.** Should clients subscribe to all events and filter client-side, or should the server support per-resource subscriptions (e.g. `/api/events?ticket=42`)? Recommendation: start with global stream + client-side filtering, add server-side filtering if needed.
3. **Pagination style.** Offset-based (`?page=2&per_page=50`) or cursor-based (`?after=<cursor>`)? Cursor-based is more robust for live-updating lists but adds complexity. To be decided in the endpoint schema DD.
