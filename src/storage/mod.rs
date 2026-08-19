//! Document storage abstraction (`o6-s3-storage`).
//!
//! Two backends:
//!
//! - [`FilesystemStore`] — local disk. Default. The root is set
//!   via the `DOCUMENTS_DIR` env var (or `cfg.documents_dir`).
//! - [`S3Store`] — S3-compatible object storage. Optional,
//!   feature-gated behind `storage-s3`.
//!
//! Both implement the [`Storage`] trait. The application state
//! holds an `Arc<dyn Storage>` so handlers don't care which
//! backend is in use.
//!
//! ## Configuration
//!
//! `STORAGE_BACKEND=fs` (default) → `FilesystemStore` at
//! `DOCUMENTS_DIR`.
//!
//! `STORAGE_BACKEND=s3` (requires the `storage-s3` feature) →
//! `S3Store` configured from these env vars:
//!
//! | Variable            | Required | Example                                |
//! |---------------------|----------|----------------------------------------|
//! | `STORAGE_BACKEND`   | yes      | `s3`                                   |
//! | `S3_BUCKET`         | yes      | `my-bucket`                            |
//! | `S3_REGION`         | yes      | `us-east-1`                            |
//! | `S3_ENDPOINT_URL`   | no       | `http://minio:9000` (MinIO, R2, etc.)  |
//! | `S3_ACCESS_KEY_ID`  | yes      | `AKIA…`                                |
//! | `S3_SECRET_ACCESS_KEY` | yes   | `…`                                    |
//! | `S3_KEY_PREFIX`     | no       | `documents/` (default empty)           |

use crate::error::AppError;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

pub mod filesystem;
#[cfg(feature = "storage-s3")]
pub mod s3;

pub use filesystem::FilesystemStore;
#[cfg(feature = "storage-s3")]
pub use s3::S3Store;

/// Storage backend error wrapper. All backends funnel into this
/// so callers don't depend on a specific SDK.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("s3 error: {0}")]
    S3(String),
    #[error("not found")]
    NotFound,
    #[error("invalid filename")]
    InvalidFilename,
    #[error("other: {0}")]
    Other(String),
}

impl From<StorageError> for AppError {
    fn from(value: StorageError) -> Self {
        match value {
            StorageError::InvalidFilename => AppError::Validation("invalid filename".into()),
            other => AppError::Db(sqlx::Error::Decode(format!("{other}").into())),
        }
    }
}

/// The abstract document store. Every backend implements these
/// methods. Handlers MUST go through this trait — never reach
/// into a concrete backend.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Where a fresh object would live. Some backends return a
    /// logical key (S3), others a filesystem path. Used by the
    /// upload handler to commit the bytes.
    async fn allocate_path(
        &self,
        transaction_id: Uuid,
        original: &str,
    ) -> Result<StorageKey, StorageError>;

    /// Reconstruct a key from a stored_filename stored in the
    /// `documents` table (which is a relative path under the
    /// legacy FS layout, e.g. `{txn_id}/{uuid}-{name}`).
    fn key_from_stored(&self, stored: &str) -> Result<StorageKey, StorageError>;

    /// Backend-local root for stripping paths (FS only). Returns
    /// the empty path for S3.
    fn root_for(&self) -> PathBuf;

    /// Read the bytes for an object.
    async fn read(&self, key: &StorageKey) -> Result<Vec<u8>, StorageError>;

    /// Write the bytes for a freshly-allocated key.
    async fn write(&self, key: &StorageKey, bytes: &[u8]) -> Result<(), StorageError>;

    /// Delete the object (and best-effort any empty parent
    /// containers — only meaningful for the FS backend).
    async fn delete(&self, key: &StorageKey) -> Result<(), StorageError>;

    /// Returns a presigned URL valid for `ttl_secs` seconds. The
    /// FS backend returns `None` (the handler falls back to
    /// streaming the bytes). S3 always returns a presigned URL.
    async fn signed_url(
        &self,
        key: &StorageKey,
        ttl_secs: u32,
    ) -> Result<Option<String>, StorageError>;

    /// A short, human-readable label for the backend
    /// (`"filesystem"` or `"s3"`).
    fn backend_label(&self) -> &'static str;
}

/// Logical identifier for a stored object. Two variants so the
/// trait can stay backend-agnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageKey {
    Filesystem(PathBuf),
    S3 { bucket: String, key: String },
}

impl StorageKey {
    pub fn as_filesystem(&self) -> Option<&Path> {
        match self {
            StorageKey::Filesystem(p) => Some(p),
            _ => None,
        }
    }
}

/// `Arc<dyn Storage>` — used in `AppState`. The trait object
/// keeps callers (handlers) backend-agnostic while letting the
/// binary pick one backend at startup.
pub type SharedStorage = Arc<dyn Storage>;

/// Helper used by the FS backend's `allocate_path` to share the
/// sanitisation rule.
pub(crate) fn safe_stored_name(
    transaction_id: Uuid,
    original: &str,
) -> Result<String, StorageError> {
    let safe = sanitize_filename::sanitize(original);
    if safe.is_empty() {
        return Err(StorageError::InvalidFilename);
    }
    Ok(format!(
        "{}-{}",
        transaction_id,
        format!("{}-{}", Uuid::new_v4(), safe)
    ))
}

/// Backwards-compatible re-export so existing code that imports
/// `FilesystemStore` continues to work.
pub use filesystem::FilesystemStore as _FilesystemStore;

#[allow(dead_code)]
fn _silence_unused(_: &Path) {}
