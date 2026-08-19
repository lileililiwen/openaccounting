# ## Context

Single backend; not portable.

## Goals / Non-Goals

**Goals:**
- Trait + two impls.

**Non-Goals:**
- Migration of existing data.
- Multipart upload for files > 100 MB (separate change).

## Decisions

- Trait lives in `src/storage/mod.rs`.
- Default backend stays filesystem.
- S3 backend is feature-gated on `storage-s3` to avoid pulling the SDK
  into every build.
