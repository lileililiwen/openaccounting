# Production Deployment Guide

## Prerequisites

- PostgreSQL 14+ (16 recommended)
- Rust stable toolchain (for building)
- Reverse proxy with TLS termination (nginx, Caddy, Traefik)
- 1GB+ RAM for the application process

## Required Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `APP_SECRET` | Yes | Random string ≥ 64 characters. Used for session signing and TOTP encryption. |
| `DATABASE_URL` | Yes | PostgreSQL connection string. Must use `postgres://` or `postgresql://` scheme. |
| `APP_ENV` | Yes | Set to `production`. Enables strict security checks. |
| `DOCUMENTS_DIR` | Yes | Absolute path for uploaded document storage. |
| `APP_HOST` | No | Bind address. Default: `0.0.0.0`. |
| `APP_PORT` | No | Listen port. Default: `3000`. |
| `RUST_LOG` | No | Log level. Default: `info`. Recommended: `info,openaccounting=debug,sqlx=warn`. |
| `METRICS_ENABLED` | No | Enable `/metrics` endpoint. Default: `true`. |
| `UPLOAD_MAX_BYTES` | No | Max upload size in bytes. Default: 26214400 (25 MiB). |

## Generating APP_SECRET

```bash
# Generate a 64-character random secret
openssl rand -base64 48
```

## TLS Boundary

OpenAccounting does NOT terminate TLS. Place a reverse proxy
in front:

```
Internet → [TLS Termination] → OpenAccounting (HTTP)
```

Recommended proxy configurations:
- **nginx**: `proxy_pass http://127.0.0.1:3000;`
- **Caddy**: `reverse_proxy localhost:3000`
- **Traefik**: `services.openaccounting.loadbalancer.server.port=3000`

## Database Setup

```bash
# Create the database
createdb openaccounting

# Create a dedicated user (DO NOT use superuser in production)
psql -c "CREATE USER oa_app WITH PASSWORD 'strong_password_here';"
psql -c "GRANT ALL PRIVILEGES ON DATABASE openaccounting TO oa_app;"
psql -c "ALTER DATABASE openaccounting OWNER TO oa_app;"

# Apply migrations
cargo sqlx migrate run
```

## Logging

Structured logging via `RUST_LOG`:
- `info` — startup, requests, errors
- `debug` — SQL queries, handler details
- `warn` — slow queries, deprecation notices

## Metrics

When `METRICS_ENABLED=true`, the `/metrics` endpoint exposes
Prometheus-format metrics:
- Request count and latency histograms
- Database connection pool stats
- Active session count

## Health Checks

- `GET /healthz` — Liveness probe (200 = alive)
- `GET /readyz` — Readiness probe (200 = DB + disk healthy)

Both endpoints are anonymous (no authentication required).

## Migrations

All migrations are reversible. To apply:
```bash
cargo sqlx migrate run
```

To rollback:
```bash
cargo sqlx migrate revert
```

## Backup

See `docs/backup-restore.md` for the complete backup and
restore procedure.

## Rollback

If a new version introduces issues:

1. Stop the application
2. Restore the previous version binary
3. Run `cargo sqlx migrate revert` if new migrations were applied
4. Restart with the previous version

## Upgrade Procedure

1. Back up the database and document storage
2. Stop the application
3. Run `cargo sqlx migrate run` for any new migrations
4. Deploy the new binary
5. Verify `/healthz` and `/readyz` return 200
6. Monitor logs for errors
