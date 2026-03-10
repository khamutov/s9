# TASKS.md

Status legend: `[ ]` pending | `[~]` in progress | `[x]` completed

## Phase 0: Foundation (Design & Prototypes)

### Design Documents

- [x] **0.1** DD: Database Schema & Storage Engine → `docs/dd/database.md`
- [x] **0.2** DD: Full-Text Search → `docs/dd/search.md` [blocked by: 0.1]
- [x] **0.3** DD: Authentication & Sessions → `docs/dd/auth.md`
- [x] **0.4** DD: Endpoint Schema → `docs/dd/api-endpoints.md` [blocked by: 0.1, 0.3]
- [x] **0.5** DD: Attachment Storage → `docs/dd/attachments.md`
- [x] **0.6** DD: Email Notifications → `docs/dd/notifications.md`
- [x] **0.7** DD: Frontend Architecture → `docs/dd/frontend.md`
- [x] **0.8** DD: Build Pipeline & Embedding → `docs/dd/build-pipeline.md` [blocked by: 0.7]
- [x] **0.16** PRD Addendum: Ticket Slugs → `docs/prd/2. Ticket Slugs.md`
- [x] **0.17** DD: Ticket Slugs → `docs/dd/ticket-slugs.md` [blocked by: 0.1, 0.4]

### UI Prototypes

- [x] **0.9** Shared stylesheet → `prototypes/style.css`
- [x] **0.10** Ticket list → `prototypes/01-ticket-list.html` [blocked by: 0.9]
- [x] **0.11** Ticket detail → `prototypes/02-ticket-detail.html` [blocked by: 0.9]
- [x] **0.12** Component tree → `prototypes/03-component-tree.html` [blocked by: 0.9]
- [x] **0.13** Create ticket form → `prototypes/04-create-ticket.html` [blocked by: 0.9]
- [x] **0.14** Layout shell → `prototypes/05-layout.html` [blocked by: 0.9]
- [x] **0.15** Milestone view → `prototypes/06-milestone-view.html` [blocked by: 0.9]

## Phase 1: Project Scaffolding

- [x] **1.1** Initialize Cargo workspace with backend crate [blocked by: 0.8]
- [x] **1.2** Initialize React+TS project with Vite [blocked by: 0.7, 0.8]
- [x] **1.3** Rust linting/formatting setup (rustfmt, clippy) [blocked by: 1.1]
- [x] **1.4** Frontend linting (ESLint, Prettier) [blocked by: 1.2]
- [x] **1.5** Build pipeline: frontend build + rust-embed [blocked by: 1.1, 1.2]
- [x] **1.6** Dev workflow: Vite proxy to backend [blocked by: 1.1, 1.2]

## Phase 2: Backend Core

- [x] **2.1** DB connection pool + migration runner [blocked by: 0.1, 1.1]
- [x] **2.2** Configuration loading (CLI args, env vars, config file) [blocked by: 1.1]
- [x] **2.3** Database schema migrations (all tables) [blocked by: 0.1, 2.1]
- [x] **2.4** Domain models (Rust structs with serde) [blocked by: 2.3]
- [x] **2.5** User repository [blocked by: 2.4]
- [x] **2.6** Component repository (tree ops) [blocked by: 2.4]
- [x] **2.7** Ticket repository (CRUD, filters, pagination) [blocked by: 2.4]
- [x] **2.8** Comment repository [blocked by: 2.4]
- [x] **2.9** Milestone repository [blocked by: 2.4]
- [x] **2.10** Search filter parser (micro-syntax → structured query) [blocked by: 0.2, 0.17]
- [x] **2.11** Full-text search integration [blocked by: 0.2, 2.7]
- [x] **2.12** Attachment storage (SHA-256 content-addressed FS) [blocked by: 0.5]
- [x] **2.13** Slug schema migration (add slug column to components) [blocked by: 0.17, 2.1]
- [x] **2.14** Component slug cache + resolution service [blocked by: 0.17, 2.6, 2.13]

## Phase 3: Backend Auth & API

- [x] **3.1** Password hashing (argon2) [blocked by: 0.3]
- [x] **3.2** Session management [blocked by: 0.3, 2.5]
- [x] **3.3** Auth middleware [blocked by: 3.2]
- [x] **3.4** Login/logout endpoints [blocked by: 0.4, 3.1, 3.2]
- [x] **3.5** OIDC authentication flow [blocked by: 0.3, 3.2]
- [x] **3.6** Ticket API endpoints + OpenAPI [blocked by: 0.4, 0.17, 2.7, 2.14, 3.3]
- [x] **3.7** Comment API endpoints [blocked by: 0.4, 2.8, 3.3]
- [x] **3.8** Component API endpoints [blocked by: 0.4, 0.17, 2.6, 2.13, 3.3]
- [x] **3.9** Milestone API endpoints [blocked by: 0.4, 2.9, 3.3]
- [x] **3.10** Attachment upload/download endpoints [blocked by: 0.4, 2.12, 3.3]
- [x] **3.11** User management endpoints (admin) [blocked by: 0.4, 2.5, 3.3]
- [x] **3.12** SSE event stream [blocked by: 0.4, 3.3]
- [x] **3.13** Role-based authorization middleware [blocked by: 3.3]
- [x] **3.14** OpenAPI spec generation [blocked by: 3.6, 3.7, 3.8, 3.9, 3.10, 3.11]
- [x] **3.15** Unified error handling [blocked by: 1.1]

## Phase 4: Backend Notifications

- [x] **4.1** Notification event producer [blocked by: 0.6, 3.6, 3.7]
- [x] **4.2** Email sender (SMTP/lettre) [blocked by: 0.6]
- [x] **4.3** Notification batching (2-min delay) [blocked by: 4.1, 4.2]
- [x] **4.4** Per-ticket mute preferences [blocked by: 4.1, 2.5]
- [x] **4.5** @mention parsing in comments [blocked by: 2.8, 4.1]
- [x] **4.6** Micro-syntax reference parsing (#ID, #PREFIX-ID, comment#N) [blocked by: 0.17, 2.8, 2.14]

## Phase 5: Frontend

> **Testing policy**: each feature task includes component tests + E2E tests as part of definition of done.

- [x] **5.1** Design tokens + global CSS (from prototype) [blocked by: 0.7, 0.9, 1.2]
- [x] **5.2** Layout shell component [blocked by: 5.1, 0.14]
- [x] **5.3** Routing setup [blocked by: 0.7, 1.2]
- [x] **5.4** API client layer (fetch wrapper, TS types) [blocked by: 0.4, 1.2]
- [x] **5.5** SSE client [blocked by: 5.4]
- [x] **5.6** Auth pages (login, OIDC redirect) [blocked by: 5.2, 5.4]
- [x] **5.7** Ticket list page [blocked by: 5.2, 5.4, 0.10]
- [x] **5.8** Filter bar with autocomplete [blocked by: 5.7]
- [x] **5.9** Ticket detail page [blocked by: 5.2, 5.4, 0.11]
- [x] **5.10** Inline-editable metadata fields [blocked by: 5.9]
- [x] **5.11** Markdown editor (textarea, preview, attachment drop) [blocked by: 5.1]
- [x] **5.12** Markdown renderer (CommonMark, micro-syntax links) [blocked by: 5.1]
- [x] **5.13** Comment thread component [blocked by: 5.9, 5.11, 5.12]
- [x] **5.14** Create/edit ticket form [blocked by: 5.2, 5.4, 0.13]
- [x] **5.15** Component tree view [blocked by: 5.2, 5.4, 0.12]
- [x] **5.16** Milestone view [blocked by: 5.2, 5.4, 0.15]
- [x] **5.17** Admin panel [blocked by: 5.2, 5.4]
- [ ] **5.18** Keyboard navigation (j/k, Enter, c) [blocked by: 5.7]
- [ ] **5.19** Attachment upload (drag-drop, paste) [blocked by: 5.11]

## Phase 6: Integration & Deployment

- [ ] **6.5** Dockerfile [blocked by: 0.8]
- [ ] **6.6** Deployment documentation [blocked by: Phase 4]
- [ ] **6.7** Initial admin user seeding [blocked by: 3.1, 2.5]
- [ ] **6.8** Performance testing (10k tickets) [blocked by: 3.6]

## Critical Path

```
DD: Database (0.1) → Schema migrations (2.3) → Models (2.4) → Ticket repo (2.7) → Ticket API (3.6) → Ticket list frontend (5.7)
                  ↘ DD: Search (0.2) → Search parser (2.10)
DD: Auth (0.3) → DD: Endpoints (0.4) → all API handlers
                                     ↘ API client (5.4) → all frontend pages
Prototype CSS (0.9) → all prototypes → frontend components
DD: Frontend (0.7) → DD: Build (0.8) → scaffolding (1.1, 1.2)
```
