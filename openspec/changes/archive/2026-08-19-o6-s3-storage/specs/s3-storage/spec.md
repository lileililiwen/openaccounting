# s3-storage Specification (delta)

## ADDED Requirements

### Requirement: Storage Trait

MUST define a `Storage` trait with `put`, `get`, `delete`, `exists`, `signed_url` methods.

#### Scenario: Round-trip

- **WHEN** an object is put and then get-ed
- **THEN** the bytes match.

### Requirement: Filesystem Backend

MUST retain the existing filesystem behavior when `STORAGE_BACKEND=fs` (default).

#### Scenario: Default

- **WHEN** STORAGE_BACKEND not set
- **THEN** filesystem behavior; existing tests pass.

### Requirement: S3 Backend

MUST use the S3 SDK to put, get, delete when `STORAGE_BACKEND=s3`; MUST support any S3-compatible endpoint via `S3_ENDPOINT_URL`.

#### Scenario: S3

- **WHEN** STORAGE_BACKEND=s3, S3_ENDPOINT_URL=http://minio:9000
- **THEN** uploads go to MinIO.

### Requirement: Signed URLs

MUST MUST, when the S3 backend is configured, expose signed download URLs to avoid proxying large files.

#### Scenario: Signed URL

- **WHEN** the user clicks download
- **THEN** the response is a 302 to a presigned S3 URL.

### Requirement: Atomicity

MUST be safe to use across concurrent requests (no in-process locking beyond what the SDK provides).

#### Scenario: Concurrent

- **WHEN** two concurrent uploads of different keys
- **THEN** both succeed.
