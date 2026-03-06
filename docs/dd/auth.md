# Design Document: Authentication & Sessions

| Field        | Value                        |
|--------------|------------------------------|
| Status       | Draft                        |
| Author       | khamutov, Claude co-authored |
| Last updated | 2026-03-06                   |
| PRD ref      | 1. Initial PRD, §6           |
| Depends on   | DD 0.1 (Database Schema)     |

---

## 1. Context and Scope

S9 is a Rust/axum + React bug tracker shipped as a single embedded binary. The database DD (0.1) defined the `users` and `sessions` tables and left open question #2: "DB sessions vs signed cookies?" The API contract DD chose HTTP-only/Secure/SameSite=Lax session cookies and CSRF via Content-Type check.

This document specifies the full authentication and authorization design: session management, password hashing, login/logout flow, OIDC integration, auth middleware, the authorization model, and password reset. It unblocks:

- **0.4** DD: Endpoint Schema (needs auth endpoints and middleware behavior)
- **3.1** Password hashing implementation
- **3.2** Session management implementation
- **3.3** Auth middleware implementation
- **3.4** Login/logout endpoints
- **3.5** OIDC authentication flow

## 2. Problem Statement

Before writing any auth code we need to decide:

- How sessions are stored and managed (resolves DD 0.1 open question #2).
- Password hashing algorithm and parameters.
- The exact login, logout, and session lifecycle.
- How OIDC external authentication integrates with local user records.
- What the auth middleware checks and injects into request handlers.
- The authorization model: who can do what.
- How password reset works.
- How the first admin user is bootstrapped.

## 3. Goals

- Provide a session mechanism that supports server-side revocation (logout, user deactivation).
- Use a modern, memory-hard password hashing algorithm with OWASP-recommended parameters.
- Support both internal (password) and external (OIDC) authentication, coexisting.
- Define a simple role-based authorization model matching PRD §6.3.
- Prevent user enumeration, timing attacks, and other common auth vulnerabilities.

## 4. Non-goals

- Multi-factor authentication (MFA).
- JWT or stateless tokens (we chose server-side sessions).
- Fine-grained permissions beyond admin/user roles.
- Self-registration (users are created by admins or provisioned via OIDC).
- Rate limiting (handled at the HTTP middleware layer, orthogonal to auth design).

## 5. Options Considered

### Option A: Signed cookies (stateless sessions) `[rejected]`

Store session data (user ID, role, expiry) in a signed, encrypted cookie. No server-side session table.

**Pros:**
- No database lookup on every request.
- Simpler implementation — no session table management, no cleanup.

**Cons:**
- **No server-side revocation.** Logging out only clears the client cookie; a captured cookie remains valid until expiry. Deactivating a user does not immediately invalidate existing sessions.
- Cookie size grows with payload (user metadata, CSRF tokens).
- Key rotation requires careful handling to avoid mass session invalidation.
- The `sessions` table already exists in the schema (DD 0.1 §7.2).

### Option B: Database-backed sessions `[selected]`

Store a random session token in the database. The cookie contains only the token. Session validity is checked on every request via a DB lookup.

**Pros:**
- **Server-side revocation.** Logout deletes the session row. Deactivating a user deletes all their sessions immediately.
- Token is opaque — no user data in the cookie.
- Schema already exists (`sessions` table in DD 0.1 §7.2).
- SQLite point-lookup by primary key is ~0.1ms — negligible overhead.

**Cons:**
- One DB read per authenticated request (mitigated by SQLite's in-process speed — no network round-trip).

## 6. Decision

**Option B — Database-backed sessions.**

Server-side revocation is a hard requirement for logout and user deactivation. The `sessions` table already exists in the schema. SQLite point-lookups are sub-millisecond with no network overhead since it runs in-process. This resolves DD 0.1 open question #2.

## 7. Session Design

### 7.1 Token generation

- 32 cryptographically random bytes via `rand::rngs::OsRng`.
- Hex-encoded to 64 characters.
- Stored as `sessions.id` (TEXT PRIMARY KEY).

The token is the session identifier — there is no separate session ID. This avoids the need for a secondary lookup column.

### 7.2 Cookie configuration

| Attribute  | Value            | Rationale                                              |
|------------|------------------|--------------------------------------------------------|
| Name       | `s9_session`     | Namespaced to avoid collisions.                        |
| Value      | 64-char hex token| The session token.                                     |
| HttpOnly   | Yes              | Prevents JavaScript access (XSS mitigation).           |
| Secure     | Yes              | Cookie only sent over HTTPS. Disabled in dev mode.     |
| SameSite   | Lax              | Prevents CSRF on state-changing requests while allowing top-level navigations. |
| Path       | `/`              | Available to all routes.                               |
| Max-Age    | 30 days          | Matches server-side expiry.                            |

Per API contract DD §8: session cookies are HTTP-only, Secure, SameSite=Lax.

### 7.3 Session lifetime and sliding expiration

- **Initial TTL:** 30 days from creation.
- **Sliding expiry:** If the session has less than 15 days remaining when validated, extend `expires_at` to 30 days from now. This avoids a DB write on every single request while keeping active sessions alive indefinitely.
- **Max absolute lifetime:** None. Active users stay logged in. Inactive sessions expire after 30 days.

### 7.4 Session cleanup

- **Lazy:** On every session lookup, check `expires_at`. If expired, treat as invalid (return 401) but do not delete inline — let the cleanup job handle it.
- **Periodic:** A background task runs every hour and deletes all rows where `expires_at < now`. This keeps the table lean without impacting request latency.

```sql
DELETE FROM sessions WHERE expires_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
```

### 7.5 User deactivation

When a user is deactivated (`is_active = 0`), immediately delete all their sessions:

```sql
DELETE FROM sessions WHERE user_id = :user_id;
```

This ensures deactivated users are logged out within the current request cycle, not at next session validation.

## 8. Password Hashing

### 8.1 Algorithm: argon2id

Argon2id is the recommended password hashing algorithm (OWASP, RFC 9106). It combines resistance to both side-channel attacks (argon2i) and GPU/ASIC attacks (argon2d).

**Rust crate:** `argon2` (from RustCrypto).

### 8.2 Parameters

| Parameter | Value     | OWASP recommendation |
|-----------|-----------|----------------------|
| Memory    | 19456 KiB | 19 MiB (first recommendation) |
| Iterations| 2         | 2                    |
| Parallelism| 1        | 1                    |

These parameters produce a ~300ms hash time on modern hardware, which is acceptable for login operations.

### 8.3 Storage format

Hashes are stored in PHC string format in `users.password_hash`:

```
$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>
```

The PHC format embeds algorithm, version, and parameters — making future parameter upgrades transparent. The `argon2` crate handles PHC encoding/decoding natively.

### 8.4 Password policy

- Minimum 8 characters.
- No complexity requirements (uppercase, special chars, etc.) — these annoy users without meaningfully improving security.
- No maximum length (the hash is constant-size regardless of input).

### 8.5 Timing attack mitigation

When a login attempt specifies an unknown username, perform a dummy argon2id hash before returning an error. This ensures the response time is indistinguishable from a valid-user-wrong-password attempt, preventing timing-based user enumeration.

```rust
// Pseudocode
let user = find_user_by_login(login).await;
match user {
    Some(u) => verify_password(password, &u.password_hash),
    None => {
        // Dummy hash to equalize timing
        argon2id_hash(password, &dummy_salt);
        Err(AuthError::InvalidCredentials)
    }
}
```

## 9. Login/Logout Flow

### 9.1 Login: `POST /api/auth/login`

**Request:**
```json
{
  "login": "alex",
  "password": "hunter2"
}
```

**Steps:**
1. Look up user by `login`.
2. If not found, perform dummy hash (§8.5), return 401.
3. If found but `is_active = 0`, return 401 (same error as invalid credentials).
4. Verify password against `password_hash`.
5. If mismatch, return 401.
6. Generate 32-byte random session token, hex-encode.
7. Insert session row: `(id=token, user_id, expires_at=now+30d)`.
8. Set `s9_session` cookie with the token.
9. Return 200 with user info.

**Success response (200):**
```json
{
  "id": 1,
  "login": "alex",
  "display_name": "Alex Kim",
  "email": "alex@example.com",
  "role": "user"
}
```

**Error response (401):**
```json
{
  "error": "invalid_credentials",
  "message": "Invalid login or password."
}
```

A single generic error message for all failure modes (unknown user, wrong password, deactivated user) prevents user enumeration.

### 9.2 Logout: `POST /api/auth/logout`

**Steps:**
1. Extract session token from `s9_session` cookie.
2. Delete the session row from the database.
3. Clear the cookie (set Max-Age=0).
4. Return 204 No Content.

Logout always returns 204, even if the session was already expired or missing. This is idempotent and avoids leaking session state.

### 9.3 Current user: `GET /api/auth/me`

Returns the currently authenticated user. The frontend calls this on page load to check auth state.

**Authenticated (200):**
```json
{
  "id": 1,
  "login": "alex",
  "display_name": "Alex Kim",
  "email": "alex@example.com",
  "role": "user"
}
```

**Not authenticated (401):**
```json
{
  "error": "unauthorized",
  "message": "Authentication required."
}
```

## 10. OIDC Integration

### 10.1 Configuration

OIDC is configured via environment variables:

| Variable              | Required | Description                              |
|-----------------------|----------|------------------------------------------|
| `S9_OIDC_ISSUER_URL`  | Yes      | Issuer URL (e.g. `https://idp.example.com/realm`) |
| `S9_OIDC_CLIENT_ID`   | Yes      | Client ID registered with the IdP        |
| `S9_OIDC_CLIENT_SECRET`| Yes     | Client secret                            |
| `S9_OIDC_DISPLAY_NAME`| No       | Button label on login page (default: "SSO") |

When `S9_OIDC_ISSUER_URL` is not set, OIDC is disabled and the OIDC endpoints return 404.

**Rust crate:** `openidconnect`. Provider metadata is discovered automatically via `{issuer_url}/.well-known/openid-configuration`.

### 10.2 Authorization Code Flow

**Step 1: Initiate — `GET /api/auth/oidc/authorize`**

1. Generate a random `state` parameter (32 bytes, hex-encoded).
2. Generate a random `nonce` parameter (32 bytes, hex-encoded).
3. Store `(state, nonce)` in a short-lived, HttpOnly cookie (`s9_oidc_state`, Max-Age=10 minutes).
4. Construct the authorization URL with `response_type=code`, `scope=openid profile email`, `redirect_uri={origin}/api/auth/oidc/callback`, and the `state`/`nonce`.
5. Return 302 redirect to the IdP authorization endpoint.

**Step 2: Callback — `GET /api/auth/oidc/callback?code=...&state=...`**

1. Verify the `state` parameter matches the value in the `s9_oidc_state` cookie.
2. Clear the `s9_oidc_state` cookie.
3. Exchange the authorization code for tokens at the IdP's token endpoint.
4. Validate the ID token: signature (via IdP JWKS), `iss`, `aud`, `exp`, and `nonce` (matches the stored nonce).
5. Extract claims from the ID token.
6. Find or create the local user (§10.3).
7. Create a session (same as password login: random token, insert row, set cookie).
8. Redirect to `/` (or a `redirect_uri` from the original state, if implemented later).

### 10.3 User provisioning

**Claim mapping:**

| ID Token Claim      | User Field      |
|----------------------|-----------------|
| `sub`                | `oidc_sub`      |
| `preferred_username` | `login`         |
| `email`              | `email`         |
| `name`               | `display_name`  |

**Find-or-create logic:**

1. Look up user by `oidc_sub` (the IdP's stable subject identifier).
2. If found: update `login`, `email`, `display_name` from latest claims. Create session.
3. If not found: create a new user with `password_hash = NULL`, `role = 'user'`, `is_active = 1`, and the mapped fields. Create session.

**No auto-linking by email.** If a local user with the same email already exists but has no `oidc_sub`, the OIDC flow creates a separate user. This prevents account takeover if an attacker controls an email claim at the IdP. An admin can manually merge accounts if needed.

### 10.4 OIDC-only users

Users provisioned via OIDC have `password_hash = NULL`. They cannot log in via the password form. The login page shows both the password form and the OIDC button (when configured).

## 11. Auth Middleware

### 11.1 Scope

The auth middleware runs on all `/api/` routes **except**:

- `POST /api/auth/login`
- `POST /api/auth/logout`
- `GET /api/auth/oidc/authorize`
- `GET /api/auth/oidc/callback`
- `GET /api/auth/me` (allowed unauthenticated — returns 401 with error body)

Correction: `GET /api/auth/me` does go through the middleware but returns a structured 401 rather than being excluded. The middleware passes through all requests to auth endpoints listed above.

### 11.2 Middleware logic

```
1. Extract `s9_session` cookie value.
   → Missing cookie → 401
2. Look up session by token (JOIN users).
   → No matching session → 401
3. Check `sessions.expires_at > now`.
   → Expired → 401 (do not delete; let cleanup handle it)
4. Check `users.is_active = 1`.
   → Deactivated → 401
5. Sliding expiry: if expires_at - now < 15 days, update expires_at = now + 30 days.
6. Inject `AuthUser` into the request extensions.
```

The `AuthUser` struct available to all handlers:

```rust
pub struct AuthUser {
    pub id: i64,
    pub login: String,
    pub display_name: String,
    pub email: String,
    pub role: String,       // "admin" or "user"
    pub session_id: String, // for logout
}
```

### 11.3 Error response

All auth failures return the same response to prevent information leakage:

```json
HTTP/1.1 401 Unauthorized
Content-Type: application/json

{
  "error": "unauthorized",
  "message": "Authentication required."
}
```

No distinction between missing token, expired session, deactivated user, or invalid token.

### 11.4 SSE authentication

Per API contract DD §8: the SSE `GET /api/events` endpoint authenticates via the session cookie (same as all other API endpoints). No token in the query string. The `EventSource` API sends cookies automatically for same-origin requests.

## 12. Authorization Model

### 12.1 Roles

Two roles as defined in PRD §6.3:

| Role    | Scope |
|---------|-------|
| `admin` | Everything a `user` can do, plus: manage users, manage components, manage milestones, delete comments, system configuration. |
| `user`  | Create tickets in any component, comment on any ticket, edit own tickets/comments, change ticket metadata. |

### 12.2 Implementation: `RequireAdmin` extractor

Admin-only routes use a `RequireAdmin` axum extractor that wraps `AuthUser`:

```rust
pub struct RequireAdmin(pub AuthUser);
```

If the `AuthUser.role` is not `admin`, the extractor rejects with 403 Forbidden:

```json
{
  "error": "forbidden",
  "message": "Administrator access required."
}
```

### 12.3 Ownership checks

Some actions have ownership rules enforced at the handler level (not middleware):

| Action | Rule |
|--------|------|
| Edit comment body | Author of the comment, or admin |
| Delete comment | Admin only |
| Edit ticket title | Ticket creator, or admin |
| Edit ticket description (comment #0) | Ticket creator, or admin |
| Change ticket metadata (status, priority, owner, CC, milestone, component) | Any authenticated user |
| Create ticket | Any authenticated user |
| Add comment | Any authenticated user |

### 12.4 User deactivation effects

When a user is deactivated:
- All their sessions are deleted immediately (§7.5).
- They cannot log in (password login checks `is_active`, §9.1 step 3).
- Their OIDC login also fails (middleware checks `is_active`, §11.2 step 4).
- Their existing tickets and comments remain visible (soft deactivation, not deletion).

## 13. Password Reset Flow

### 13.1 Schema addition

A new `password_resets` table (not in DD 0.1's original schema):

```sql
CREATE TABLE password_resets (
    id         INTEGER PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id),
    token      TEXT    NOT NULL UNIQUE,  -- SHA-256 of actual token
    expires_at TEXT    NOT NULL,
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_password_resets_token ON password_resets(token);
CREATE INDEX idx_password_resets_user_id ON password_resets(user_id);
```

The token stored in the database is the SHA-256 hash of the actual token sent to the user. This way, if the database is compromised, the attacker cannot use stored tokens to reset passwords.

### 13.2 Request reset: `POST /api/auth/password-reset/request`

**Request:**
```json
{
  "email": "alex@example.com"
}
```

**Steps:**
1. Look up user by email.
2. If not found, or if user has no `password_hash` (OIDC-only), return 200 anyway (no enumeration).
3. Generate 32-byte random token, hex-encode.
4. Store SHA-256 hash of the token in `password_resets` with `expires_at = now + 1 hour`.
5. Delete any existing reset tokens for this user (single active token per user).
6. Send email with reset link: `{origin}/reset-password?token={raw_token}`.
7. Return 200.

**Response (always 200):**
```json
{
  "message": "If an account with that email exists, a reset link has been sent."
}
```

### 13.3 Confirm reset: `POST /api/auth/password-reset/confirm`

**Request:**
```json
{
  "token": "a1b2c3...hex...",
  "new_password": "newsecurepassword"
}
```

**Steps:**
1. Compute SHA-256 of the provided token.
2. Look up `password_resets` by the hash.
3. If not found or expired, return 400 (generic "invalid or expired token").
4. Validate new password (minimum 8 characters).
5. Hash new password with argon2id.
6. Update `users.password_hash`.
7. Delete the reset token row.
8. Delete all sessions for this user (force re-login everywhere).
9. Return 200.

**Success (200):**
```json
{
  "message": "Password has been reset. Please log in."
}
```

**Error (400):**
```json
{
  "error": "invalid_token",
  "message": "Reset token is invalid or has expired."
}
```

## 14. First Admin Bootstrap

The system needs at least one admin user to function. Since there is no self-registration, a CLI command creates the initial admin.

**Command:**
```
s9 create-admin --login admin --password <password>
```

**Behavior:**
1. Run pending database migrations (ensures schema exists).
2. Check that the login does not already exist.
3. Hash the password with argon2id.
4. Insert a user with `role = 'admin'`, `is_active = 1`.
5. Print confirmation and exit (no web server started).

This is documented here for completeness. Implementation is task 6.7.

## 15. Security Considerations

### 15.1 CSRF protection

Per API contract DD §8: the `Content-Type: application/json` header requirement on all mutation endpoints acts as a CSRF guard (browsers cannot send JSON Content-Type via form submissions). The `SameSite=Lax` cookie attribute provides a second layer — cookies are not sent on cross-origin POST requests.

The multipart attachment upload endpoint (`POST /api/attachments`) cannot rely on Content-Type checking. It is protected by `SameSite=Lax` cookies and requires an authenticated session.

### 15.2 Token entropy

Session tokens: 32 random bytes = 256 bits of entropy. Brute-force infeasible.

Password reset tokens: 32 random bytes = 256 bits. Stored as SHA-256 hash, so database compromise does not leak usable tokens.

OIDC state/nonce: 32 random bytes each, single-use, short-lived (10 minutes).

### 15.3 User enumeration prevention

- Login: same error for unknown user, wrong password, deactivated user.
- Login: dummy hash on unknown user to equalize timing.
- Password reset: same 200 response whether email exists or not.
- No endpoint lists users publicly (user list is admin-only).

### 15.4 OIDC security

- `state` parameter prevents CSRF on the authorization flow.
- `nonce` in the ID token prevents replay attacks.
- ID token signature verified against IdP's JWKS (fetched from discovery endpoint).
- No auto-linking by email prevents account takeover via compromised email claims.
- Client secret is stored as an environment variable, not in the database.

### 15.5 Session security

- Tokens are generated from a CSPRNG (`OsRng`), not a PRNG.
- Tokens are never logged or included in error responses.
- Session cookie is HttpOnly (no JS access), Secure (HTTPS only), SameSite=Lax.
- Deactivation immediately invalidates all sessions.

## 16. Schema Additions

Beyond the existing DD 0.1 schema, this document adds the `password_resets` table (§13.1). The DDL is included in the initial migration alongside the existing tables.

**Summary of all auth-related tables** (existing + new):

| Table             | Defined in | Purpose                          |
|-------------------|------------|----------------------------------|
| `users`           | DD 0.1 §7.1 | User accounts                  |
| `sessions`        | DD 0.1 §7.2 | Active sessions                |
| `password_resets`  | This DD §13.1 | Password reset tokens         |

## 17. Open Questions

1. **Account merging.** If an OIDC user and a local user share the same email, should an admin UI be provided to merge them? Recommendation: defer to admin panel design (task 5.17).
2. **Remember-me checkbox.** Should the login form offer a "remember me" option with different session durations (e.g. 24 hours vs 30 days)? Recommendation: defer — 30 days with sliding expiry is a reasonable default for all users.
3. **Password reset email template.** The exact email template (subject, body, styling) is deferred to DD 0.6 (Email Notifications).
