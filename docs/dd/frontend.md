# Design Document: Frontend Architecture

| Field        | Value                        |
|--------------|------------------------------|
| Status       | Draft                        |
| Author       | khamutov, Claude co-authored |
| Last updated | 2026-03-06                   |
| PRD ref      | 1. Initial PRD, §8           |
| Depends on   | DD 0.1 (Database), DD 0.3 (Auth), DD 0.4 (Endpoints) |

---

## 1. Context and Scope

S9 is a Rust/axum + React bug tracker shipped as a single embedded binary. All backend DDs are complete: DD 0.1 defined the database schema, DD 0.2 the search strategy, DD 0.3 the authentication design, and DD 0.4 the full endpoint schema. The API contract DD chose JSON REST + SSE with HTTP-only session cookies.

This document decides the frontend technology choices and architectural patterns: state management, CSS strategy, routing, API client, component organization, Markdown rendering, keyboard navigation, SSE integration, and testing. It unblocks:

- **0.8** DD: Build Pipeline & Embedding (needs Vite config, output paths, dev proxy)
- **1.2** React+TS project scaffolding (needs dependency list and project structure)
- **5.1** Design tokens + global CSS (needs CSS strategy decision)
- **5.3** Routing setup (needs routing library choice)
- Indirectly, all Phase 5 frontend tasks

## 2. Problem Statement

Before writing any frontend code we need to decide:

- How to manage server state (loading, caching, refetch, deduplication).
- How to manage the small amount of client state (auth user, UI toggles).
- CSS authoring strategy (must support typographic control for Swiss/Foundry aesthetic).
- Routing library and route map.
- API client approach (type safety across 33+ endpoints).
- SSE integration for real-time updates.
- Component organization and naming conventions.
- Markdown rendering and the custom micro-syntax plugin.
- Forms, inline editing, keyboard navigation.
- Testing strategy.

## 3. Goals

- **Type-safe end-to-end.** TypeScript types generated from the OpenAPI spec — no manual type maintenance.
- **Keyboard-friendly.** PRD §8.3 shortcuts (j/k, Enter, c, /, Escape) as first-class citizens.
- **Embeddable.** Static SPA output suitable for `rust-embed`; no SSR.
- **Minimal dependencies.** ~80 KB gzipped runtime budget.
- **Real-time.** SSE-driven cache invalidation for multi-user consistency.

## 4. Non-goals

- Mobile-native app.
- Offline mode / PWA.
- Internationalization (i18n).
- Server-side rendering.
- Dark mode (deferred per PRD §11).
- Reusable component library for other projects.

## 5. State Management

### Option A: React Context + useReducer `[rejected]`

**Pros:**
- Zero additional dependencies.
- Familiar to all React developers.

**Cons:**
- No built-in caching, refetch, deduplication, or stale-while-revalidate.
- The app is ~90% server state — would require reimplementing query lifecycle management.
- Every context update re-renders all consumers unless carefully memoized.

### Option B: Redux Toolkit + RTK Query `[rejected]`

**Pros:**
- Full-featured data fetching with caching and normalization.
- Large ecosystem.

**Cons:**
- ~11 KB gzipped — disproportionate for a CRUD app with minimal complex client state.
- Boilerplate-heavy (slices, reducers, selectors) for what is primarily server-state management.
- RTK Query's code generation is tied to Redux patterns.

### Option C: TanStack Query v5 + React Context `[selected]`

**Pros:**
- Purpose-built for server state: handles loading, error, stale, refetch, pagination, deduplication.
- SSE integrates cleanly via `queryClient.invalidateQueries()`.
- ~13 KB gzipped — focused on exactly what the app needs.
- React Context handles the small amount of client state (auth user, sidebar toggle) without a library.

**Cons:**
- Does not manage client-only state (acceptable — minimal client state uses Context).

**Decision:** TanStack Query v5 for all server state. React Context for auth state and UI toggles. This matches the app's data flow: almost everything comes from the API, very little state is client-only.

## 6. CSS Strategy

### Option A: Tailwind CSS `[rejected]`

**Pros:**
- Fast prototyping with utility classes.
- Consistent spacing/color scales.

**Cons:**
- Utility classes work against the typographic control needed for the Swiss/Foundry aesthetic (PRD §8.1).
- Misaligns with prototype CSS workflow (task 0.9 produces hand-authored CSS).
- Constrained grid/spacing customization requires heavy config overrides.

### Option B: CSS-in-JS (styled-components / Emotion) `[rejected]`

**Pros:**
- Scoped styles co-located with components.
- Dynamic styles based on props.

**Cons:**
- Runtime overhead (CSS injection at render time).
- Bundle size cost (~8–12 KB).
- Unnecessary for a static-palette, data-dense UI where styles don't change dynamically.

### Option C: CSS Modules `[selected]`

**Pros:**
- Zero runtime cost — styles extracted at build time.
- Vite-native support (no plugins needed).
- Full CSS power for typographic control, grid layouts, and the design token system.
- Scoped by default — no class name collisions.
- Co-located with components (`Component.module.css` next to `Component.tsx`).

**Cons:**
- No dynamic styles based on props (use CSS custom properties or className toggling instead).

**Decision:** CSS Modules for component styles. Global `tokens.css` with CSS custom properties for the design token system. This gives full typographic control for the Swiss/Foundry aesthetic while keeping zero runtime overhead.

### Design Token Sketch

```css
/* src/styles/tokens.css */
:root {
  /* Colors */
  --color-bg-primary: #fafafa;
  --color-bg-secondary: #f0f0f0;
  --color-bg-surface: #ffffff;
  --color-text-primary: #1a1a1a;
  --color-text-secondary: #6b7280;
  --color-accent: #2563eb;
  --color-accent-hover: #1d4ed8;
  --color-border: #e5e7eb;
  --color-error: #dc2626;
  --color-success: #16a34a;
  --color-warning: #d97706;

  /* Priority colors (restrained) */
  --color-priority-critical: #dc2626;
  --color-priority-high: #ea580c;
  --color-priority-medium: #2563eb;
  --color-priority-low: #6b7280;

  /* Typography */
  --font-sans: 'Inter', system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', ui-monospace, monospace;
  --text-xs: 0.75rem;
  --text-sm: 0.875rem;
  --text-base: 1rem;
  --text-lg: 1.125rem;

  /* Spacing (4px grid) */
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-6: 24px;
  --space-8: 32px;

  /* Layout */
  --sidebar-width: 240px;
  --content-max-width: 960px;
  --border-radius: 4px;
}
```

## 7. Routing

### Option A: TanStack Router `[rejected]`

**Pros:**
- Type-safe route params.
- Built-in search param validation.

**Cons:**
- Overkill for ~12 routes.
- Adds codegen complexity and a learning curve.
- Smaller ecosystem than React Router.

### Option B: React Router v7 `[selected]`

**Pros:**
- De facto standard for React SPAs.
- `createBrowserRouter` with `lazy()` for route-level code splitting.
- Large ecosystem, well-documented.

**Cons:**
- Route params are untyped strings (mitigated by wrapper hooks).

**Decision:** React Router v7 with `createBrowserRouter`. Route-level code splitting via `React.lazy()`.

### Route Map

| Path                    | Component              | Auth Required |
|-------------------------|------------------------|---------------|
| `/`                     | Redirect → `/tickets`  | Yes           |
| `/login`                | LoginPage              | No            |
| `/reset-password`       | ResetPasswordPage      | No            |
| `/tickets`              | TicketListPage         | Yes           |
| `/tickets/new`          | CreateTicketPage       | Yes           |
| `/tickets/:id`          | TicketDetailPage       | Yes           |
| `/components`           | ComponentTreePage      | Yes           |
| `/milestones`           | MilestoneListPage      | Yes           |
| `/milestones/:id`       | MilestoneDetailPage    | Yes           |
| `/admin`                | AdminPanel             | Yes (admin)   |
| `/admin/users`          | UserManagement         | Yes (admin)   |
| `/admin/components`     | ComponentManagement    | Yes (admin)   |
| `/admin/settings`       | SystemSettings         | Yes (admin)   |

**SPA fallback:** The backend serves `index.html` for all `GET` requests that do not match `/api/*` or static asset paths. This allows React Router to handle client-side navigation.

**AuthGuard:** A wrapper component checks auth state and redirects unauthenticated users to `/login`. Admin routes additionally check user role.

## 8. API Client Layer

### Option A: Hand-written fetch wrapper `[rejected]`

**Pros:**
- No tooling dependencies.

**Cons:**
- Manual type maintenance for 33+ endpoints is error-prone and drifts from the actual API.

### Option B: openapi-typescript + thin fetch wrapper `[selected]`

**Pros:**
- Auto-generates TypeScript types from the OpenAPI spec (`/api/openapi.json`).
- Zero runtime cost — types are compile-time only.
- Full control over fetch logic (no framework lock-in).

**Cons:**
- Requires OpenAPI spec to be generated first (task 3.14).

**Decision:** `openapi-typescript` generates `schema.d.ts` from the OpenAPI spec. A thin hand-written fetch wrapper provides request execution, credential handling, and error mapping. Per-feature modules export typed API functions.

### Directory Structure

```
src/api/
  schema.d.ts       ← generated by openapi-typescript (committed to repo)
  client.ts         ← base fetch wrapper
  tickets.ts        ← ticket API functions
  comments.ts       ← comment API functions
  components.ts     ← component API functions
  milestones.ts     ← milestone API functions
  users.ts          ← user API functions
  auth.ts           ← auth API functions
  attachments.ts    ← attachment API functions
```

### Base Client

```typescript
// src/api/client.ts
export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    public details?: Record<string, string>,
  ) {
    super(`API error ${status}: ${code}`);
  }
}

export async function apiRequest<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T> {
  const res = await fetch(path, {
    method,
    credentials: 'same-origin',
    headers: body ? { 'Content-Type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  });

  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new ApiError(res.status, err.error ?? 'unknown', err.details);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}
```

### Integration with TanStack Query

Each feature module exports typed API functions. Custom hooks wrap them with TanStack Query:

```typescript
// src/features/tickets/useTickets.ts
import { useQuery } from '@tanstack/react-query';
import { listTickets, type TicketListParams } from '../../api/tickets';

export function useTickets(params: TicketListParams) {
  return useQuery({
    queryKey: ['tickets', params],
    queryFn: () => listTickets(params),
  });
}
```

## 9. SSE Client

Singleton `EventSource` connected to `GET /api/events`. Connects on successful authentication, disconnects on logout.

### Integration with TanStack Query

SSE events trigger `queryClient.invalidateQueries()` with matching query keys. The SSE client does not update the cache directly — invalidation triggers a background refetch, which is simpler and always correct.

```typescript
// src/hooks/useSSE.ts
const EVENT_KEY_MAP: Record<string, string[][]> = {
  ticket_created: [['tickets']],
  ticket_updated: [['tickets']],  // detail key added dynamically
  comment_created: [['tickets']], // detail key added dynamically
  component_updated: [['components']],
  milestone_updated: [['milestones']],
};

function handleSSEEvent(queryClient: QueryClient, event: MessageEvent) {
  const data = JSON.parse(event.data);
  const keys = EVENT_KEY_MAP[data.type] ?? [];

  for (const key of keys) {
    queryClient.invalidateQueries({ queryKey: key });
  }

  // Invalidate specific detail queries when an ID is present
  if (data.payload?.id) {
    queryClient.invalidateQueries({
      queryKey: [data.type.split('_')[0], data.payload.id],
    });
  }
}
```

### Reconnection

`EventSource` handles reconnection natively. On auth expiry (401 from the SSE endpoint), close the connection and redirect to login.

No optimistic cache updates from SSE in v1. Can optimize later if needed.

## 10. Auth Flow

### AuthContext

```typescript
interface AuthState {
  user: User | null;
  isLoading: boolean;
}
```

- `AuthContext` provides `{ user, isLoading }` and `login()` / `logout()` actions.
- Auth state lives in React Context (not TanStack Query) because it drives routing decisions synchronously.

### Flow

1. **On mount:** `GET /api/auth/me` → 200 sets user, 401 means unauthenticated.
2. **Login:** `POST /api/auth/login` → set user, connect SSE, navigate to `/tickets`.
3. **Logout:** `POST /api/auth/logout` → clear user, close SSE, clear query cache, navigate to `/login`.
4. **Session expiry:** Any 401 response from the API → TanStack Query global `onError` handler clears auth state and redirects to `/login`.
5. **OIDC:** Login page shows SSO button if OIDC is configured. Button navigates to `/api/auth/oidc/authorize` (full-page redirect, backend handles the OAuth flow).

### Auth Config Discovery

The login page needs to know whether OIDC is available before the user authenticates. Proposed: `GET /api/auth/config` (unauthenticated endpoint) returns `{ oidc_enabled: boolean }`. This is a minor addition to the DD 0.4 endpoint schema (see Open Questions §21).

## 11. Component Organization

Feature-based structure with shared UI components:

```
src/
  api/                      ← API client layer (§8)
    schema.d.ts
    client.ts
    tickets.ts
    comments.ts
    components.ts
    milestones.ts
    users.ts
    auth.ts
    attachments.ts
  components/               ← shared UI components
    Button.tsx
    Table.tsx
    FilterBar.tsx
    InlineEdit.tsx
    MarkdownEditor.tsx
    MarkdownRenderer.tsx
    Modal.tsx
    StatusBadge.tsx
    PriorityBadge.tsx
    UserPill.tsx
    Spinner.tsx
  features/
    auth/                   ← LoginPage, ResetPasswordPage, AuthContext, useAuth
    tickets/                ← TicketListPage, TicketDetailPage, CreateTicketPage,
                              TicketRow, MetadataSidebar, CommentThread,
                              CommentItem, useTickets, useComments
    components/             ← ComponentTreePage, ComponentNode, useComponents
    milestones/             ← MilestoneListPage, MilestoneDetailPage, useMilestones
    admin/                  ← AdminLayout, UserMgmt, ComponentMgmt, Settings, useUsers
  hooks/                    ← useKeyboardNavigation, useSSE, useDebounce
  styles/                   ← tokens.css, reset.css, typography.css, globals.css
  App.tsx
  main.tsx
  router.tsx
```

### Naming Conventions

| Pattern             | Example                 | Purpose                  |
|---------------------|-------------------------|--------------------------|
| `*Page.tsx`         | `TicketListPage.tsx`    | Route-level components   |
| `use*.ts`           | `useTickets.ts`         | Custom hooks             |
| `*.module.css`      | `TicketRow.module.css`  | Component-scoped styles  |
| `*.tsx`             | `StatusBadge.tsx`       | Shared UI components     |

CSS Modules are co-located with their component files.

## 12. Markdown Rendering

`react-markdown` + `remark-gfm` + custom `remark-s9-references` plugin.

### Custom Plugin: remark-s9-references

Transforms micro-syntax in text nodes:

| Syntax              | Rendered As                           |
|---------------------|---------------------------------------|
| `#42`               | `<a href="/tickets/42">#42</a>`       |
| `@alex`             | `<span class="mention">@alex</span>` |
| `comment#3`         | `<a href="#comment-3">comment#3</a>`  |
| `#42/comment#3`     | `<a href="/tickets/42#comment-3">#42/comment#3</a>` |

The plugin operates on the remark AST (MDAST), splitting text nodes and inserting link/span nodes. No `dangerouslySetInnerHTML` — `react-markdown` renders React elements from the AST.

No syntax highlighting for code blocks in v1 — they render in monospace with the `--font-mono` font.

## 13. Markdown Editor

Plain `<textarea>` with:

- **Live preview toggle:** Side-by-side or below-editor preview using `MarkdownRenderer`.
- **Attachment upload:** Drag-drop and paste handlers. On upload completion, inserts `![name](url)` (images) or `[name](url)` (other files) at the cursor position.
- **Keyboard shortcuts:**
  - `Ctrl+B` — wrap selection in `**bold**`
  - `Ctrl+I` — wrap selection in `*italic*`
  - `Ctrl+K` — wrap selection in `[text](url)` link template

No WYSIWYG editor — the textarea operates on raw Markdown text.

## 14. Inline Editing

Reusable `InlineEdit` component pattern: display mode → click to enter edit mode → Enter saves / Escape cancels.

### Behavior

1. Display mode shows the current value as styled text.
2. Click triggers edit mode with the appropriate input type.
3. Enter (or blur) fires a PATCH request via `useMutation`.
4. Escape reverts to display mode without saving.
5. Optimistic update: `onMutate` sets the new value immediately, `onError` rolls back.

### Field-Specific Inputs

| Field        | Input Type              |
|--------------|-------------------------|
| Title        | Text input              |
| Estimation   | Text input              |
| Status       | Dropdown                |
| Priority     | Dropdown                |
| Type         | Dropdown                |
| Owner        | User autocomplete       |
| CC           | Multi-select user       |
| Milestones   | Multi-select            |
| Component    | Component path autocomplete |

## 15. Keyboard Navigation

`useKeyboardNavigation` hook attaches a global `keydown` listener. Shortcuts are disabled when focus is inside an `<input>`, `<textarea>`, or `[contenteditable]` element.

| Key      | Context       | Action                     |
|----------|---------------|----------------------------|
| `j`      | Ticket list   | Move selection down        |
| `k`      | Ticket list   | Move selection up          |
| `Enter`  | Ticket list   | Open selected ticket       |
| `c`      | Any page      | Navigate to create ticket  |
| `/`      | Any page      | Focus filter bar           |
| `Escape` | Modal/edit    | Cancel inline edit / close modal |

The hook maintains a `selectedIndex` state and scrolls the selected row into view.

## 16. Filter Bar

Text input with autocomplete dropdown positioned below the cursor.

### Behavior

1. User types in the filter bar.
2. On each keystroke, the component pattern-matches the cursor position to determine context:
   - At word start → suggest filter keys (`owner:`, `status:`, `priority:`, `type:`, `component:`, `milestone:`).
   - After `owner:` or `cc:` → suggest user logins via autocomplete endpoint.
   - After `status:` → suggest status values (`open`, `in_progress`, `resolved`, `closed`).
   - After `component:` → suggest component paths.
   - After `priority:` / `type:` / `milestone:` → suggest corresponding values.
3. Selected suggestion is inserted at cursor position.
4. Filter string is synced to URL search param `?q=` for bookmarkable/shareable state.
5. 300 ms debounce before the API call fires.

## 17. Forms

Controlled components, no form library. The app has few simple forms:

- **Login form:** email + password.
- **Create ticket:** title, description (Markdown), type, priority, component, owner, CC, milestones.
- **Admin CRUD:** user management, component management, system settings.

Client-side validation mirrors server rules (required fields, string lengths). Server 422 responses with `details` field map are displayed as inline error messages next to the corresponding form fields.

## 18. Testing

| Tool                   | Purpose                          | Scope           |
|------------------------|----------------------------------|-----------------|
| Vitest                 | Unit + integration test runner   | All tests       |
| React Testing Library  | Component behavior tests         | Component tests |
| MSW (Mock Service Worker) | API mocking at network level  | Integration tests |
| Playwright             | End-to-end browser tests         | Task 6.4        |

### Conventions

- Test files co-located: `Component.test.tsx` next to `Component.tsx`.
- API mocks via MSW handlers — tests hit the real TanStack Query hooks, MSW intercepts `fetch`.
- Test user interactions, not implementation details. Prefer `getByRole` / `getByText` over `getByTestId`.
- E2E tests (Playwright) are separate from unit/component tests, defined in task 6.4.

## 19. Build Configuration

Informs DD 0.8 (Build Pipeline).

- **Bundler:** Vite with `@vitejs/plugin-react`.
- **Output:** `dist/` directory (static files for `rust-embed`).
- **Dev proxy:** `/api/*` requests proxied to the Rust backend (e.g., `localhost:3000`).
- **Code splitting:** `React.lazy()` per route — each page is a separate chunk.
- **Asset hashing:** Content-hash filenames for cache busting (`index-[hash].js`).
- **Source maps:** Excluded from production builds.
- **TypeScript:** `strict: true`, paths alias `@/` → `src/`.

## 20. Dependency Summary

| Package                | Size (gzipped) | Purpose                 |
|------------------------|----------------|-------------------------|
| react                  | ~6 KB          | UI library              |
| react-dom              | ~40 KB         | DOM renderer            |
| react-router           | ~14 KB         | Client-side routing     |
| @tanstack/react-query  | ~13 KB         | Server state management |
| react-markdown         | ~6 KB          | Markdown → React        |
| remark-gfm             | ~2 KB          | GFM tables/lists/etc.   |
| **Total**              | **~81 KB**     |                         |

Dev-only dependencies (not in runtime bundle): `openapi-typescript`, `vitest`, `@testing-library/react`, `msw`, `playwright`, `typescript`, `eslint`, `prettier`.

## 21. Open Questions

1. **Auth config endpoint.** The login page needs to know if OIDC is available before the user authenticates. Proposal: `GET /api/auth/config` returning `{ oidc_enabled: boolean }` as a public (unauthenticated) endpoint. This is a minor addition to DD 0.4 — to be resolved when implementing task 3.4.

2. **User autocomplete for non-admins.** `GET /api/users` is admin-only (DD 0.4), but the filter bar `owner:` key and the CC field need user lookup for all authenticated users. Proposal: `GET /api/users/autocomplete?q=` returning `[{ login, display_name }]` for all authenticated users. Limited to 10 results.

3. **Optimistic updates scope.** Start with optimistic updates for dropdown fields (status, priority, type) where the change is local and fast to roll back. Use pessimistic updates for relational fields (owner, CC, milestones) where the server may reject the change. Revisit based on UX feedback.

4. **Bundle size budget.** Enforce <200 KB gzipped initial JavaScript in CI (via `vite-plugin-inspect` or a post-build size check script). The ~81 KB runtime estimate leaves headroom for application code.

5. **Accessibility baseline.** Not targeting full WCAG audit, but enforcing: semantic HTML elements, ARIA labels on icon-only buttons, visible focus indicators, keyboard operability for all interactive elements. This is a discipline, not a separate task.
