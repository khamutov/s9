# S9 Deployment Guide

## Overview

S9 ships as a single binary with the React frontend embedded. It uses SQLite for storage and the local filesystem for attachments — no external database required.

## Quick Start (Docker)

```bash
docker build -t s9 .

docker run -d \
  --name s9 \
  -p 8080:8080 \
  -v s9-data:/data \
  s9

# Create the initial admin user
docker exec s9 s9 create-admin --login admin --password <your-password>
```

Open `http://localhost:8080` and log in.

## Quick Start (Binary)

```bash
# Build
task build

# Run (creates ./data/ for DB and attachments)
./target/release/s9 serve

# Create admin user
./target/release/s9 create-admin --login admin --password <your-password>
```

## Configuration

All settings are configured via environment variables or CLI flags. CLI flags take precedence over env vars.

### Core Settings

| Env Var | CLI Flag | Default | Description |
|---------|----------|---------|-------------|
| `S9_DATA_DIR` | `--data-dir` | `./data` | Directory for SQLite DB and attachments |
| `S9_LISTEN` | `--listen` | `127.0.0.1:8080` | Bind address and port |

### OIDC / SSO (Optional)

Set all three to enable OIDC login:

| Env Var | CLI Flag | Default | Description |
|---------|----------|---------|-------------|
| `S9_OIDC_ISSUER_URL` | `--oidc-issuer-url` | — | OIDC issuer URL (e.g. `https://idp.example.com/realm`) |
| `S9_OIDC_CLIENT_ID` | `--oidc-client-id` | — | Client ID registered with the IdP |
| `S9_OIDC_CLIENT_SECRET` | `--oidc-client-secret` | — | Client secret |
| `S9_OIDC_DISPLAY_NAME` | `--oidc-display-name` | `SSO` | Label for the SSO login button |

### Email Notifications (Optional)

Set `S9_SMTP_HOST` and `S9_SMTP_FROM` to enable email notifications. When unset, all other features work normally — emails are simply not sent.

| Env Var | CLI Flag | Default | Description |
|---------|----------|---------|-------------|
| `S9_SMTP_HOST` | `--smtp-host` | — | SMTP server hostname |
| `S9_SMTP_PORT` | `--smtp-port` | `587` | SMTP server port |
| `S9_SMTP_USERNAME` | `--smtp-username` | — | SMTP auth username |
| `S9_SMTP_PASSWORD` | `--smtp-password` | — | SMTP auth password |
| `S9_SMTP_TLS` | `--smtp-tls` | `starttls` | TLS mode: `none`, `starttls`, or `tls` |
| `S9_SMTP_FROM` | `--smtp-from` | — | Sender email address |
| `S9_BASE_URL` | `--base-url` | — | Base URL for links in emails (e.g. `https://bugs.example.com`) |
| `S9_NOTIFICATION_DELAY` | `--notification-delay` | `120` | Batching window in seconds before sending |

## CLI Subcommands

| Subcommand | Description |
|------------|-------------|
| `serve` | Start the HTTP server (default when no subcommand given) |
| `migrate` | Run pending database migrations |
| `create-admin` | Create an admin user (`--login` and `--password` required) |

## Data Directory Layout

```
$S9_DATA_DIR/
  s9.db          # SQLite database
  s9.db-wal      # WAL journal (auto-created)
  attachments/   # Content-addressed file storage (SHA-256)
    ab/
      abcdef1234...  # Files stored by hash prefix sharding
  tmp/           # Temporary upload staging (cleaned on startup)
```

## Docker Compose Example

```yaml
services:
  s9:
    build: .
    ports:
      - "8080:8080"
    volumes:
      - s9-data:/data
    environment:
      # OIDC (optional)
      S9_OIDC_ISSUER_URL: https://idp.example.com/realm
      S9_OIDC_CLIENT_ID: s9
      S9_OIDC_CLIENT_SECRET: secret
      # Email (optional)
      S9_SMTP_HOST: smtp.example.com
      S9_SMTP_FROM: s9@example.com
      S9_SMTP_USERNAME: s9
      S9_SMTP_PASSWORD: secret
      S9_BASE_URL: https://bugs.example.com

volumes:
  s9-data:
```

## Reverse Proxy

S9 binds to `0.0.0.0:8080` in Docker. Place it behind a reverse proxy for TLS termination.

### nginx

```nginx
server {
    listen 443 ssl;
    server_name bugs.example.com;

    ssl_certificate     /etc/ssl/certs/bugs.pem;
    ssl_certificate_key /etc/ssl/private/bugs.key;

    client_max_body_size 20m;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # SSE requires no buffering
    location /api/events {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 3600s;
    }
}
```

## Backup and Restore

SQLite with WAL mode. To back up safely:

```bash
# Online backup (recommended)
sqlite3 /data/s9.db ".backup /backup/s9-$(date +%Y%m%d).db"

# Also back up the attachments directory
rsync -a /data/attachments/ /backup/attachments/
```

Restore by stopping S9, replacing the DB file and attachments directory, then restarting.

## Upgrading

S9 runs migrations automatically on startup. To upgrade:

1. Pull the new image or build the new binary
2. Stop the running instance
3. Back up the data directory
4. Start the new version

Migrations are forward-only and run automatically — no manual `migrate` step needed for normal upgrades.
