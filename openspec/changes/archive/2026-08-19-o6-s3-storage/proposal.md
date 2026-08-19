# Pluggable Storage: S3-Compatible Backend

## Why

`src/storage/filesystem.rs` is the only backend. Deployments on
ephemeral VMs or containers lose documents on restart. Hetzner, AWS,
Cloudflare R2, MinIO all support the S3 API. Firefly III supports S3
out of the box.

## What Changes

- New `Storage` trait with `put`, `get`, `delete`, `exists`.
- `FilesystemStore` implements it (existing).
- New `S3Store` implements it using `aws-sdk-s3`.
- `AppConfig.storage_backend` selects; default filesystem.
- The migration of existing documents is NOT in scope (separate change).

## Capabilities

### New Capabilities

- `s3-storage`: S3-compatible document storage.

## Impact

**New files:**
- `src/storage/s3.rs`.
- `tests/integration/s3_store.rs` (uses LocalStack in CI).

**Modified files:**
- `src/storage/mod.rs` — trait.
- `src/lib.rs::AppState` — generic over backend.
- `Cargo.toml` — `aws-sdk-s3`, `aws-config`.
