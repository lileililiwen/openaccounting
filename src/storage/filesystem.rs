//! Filesystem-backed [`Storage`] implementation. Default backend.

use super::{safe_stored_name, Storage, StorageError, StorageKey};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct FilesystemStore {
    root: PathBuf,
}

impl FilesystemStore {
    pub async fn new(root: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let root = root.into();
        fs::create_dir_all(&root).await?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub async fn ensure_dir(&self, path: &Path) -> Result<(), StorageError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl Storage for FilesystemStore {
    async fn allocate_path(
        &self,
        transaction_id: Uuid,
        original: &str,
    ) -> Result<StorageKey, StorageError> {
        let stored = safe_stored_name(transaction_id, original)?;
        let tx_dir = self.root.join(transaction_id.to_string());
        Ok(StorageKey::Filesystem(tx_dir.join(stored)))
    }

    fn key_from_stored(&self, stored: &str) -> Result<StorageKey, StorageError> {
        // Legacy: stored_filename is relative to the store root.
        Ok(StorageKey::Filesystem(self.root.join(stored)))
    }

    fn root_for(&self) -> PathBuf {
        self.root.clone()
    }

    async fn read(&self, key: &StorageKey) -> Result<Vec<u8>, StorageError> {
        let path = key.as_filesystem().ok_or(StorageError::NotFound)?;
        Ok(fs::read(path).await?)
    }

    async fn write(&self, key: &StorageKey, bytes: &[u8]) -> Result<(), StorageError> {
        let path = key.as_filesystem().ok_or(StorageError::NotFound)?;
        self.ensure_dir(path).await?;
        fs::write(path, bytes).await?;
        Ok(())
    }

    async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
        let path = key.as_filesystem().ok_or(StorageError::NotFound)?;
        if path.exists() {
            fs::remove_file(path).await?;
            if let Some(parent) = path.parent() {
                let _ = fs::remove_dir(parent).await; // best effort
            }
        }
        Ok(())
    }

    async fn signed_url(
        &self,
        _key: &StorageKey,
        _ttl_secs: u32,
    ) -> Result<Option<String>, StorageError> {
        // Filesystem backend has no concept of presigned URLs;
        // callers should stream the bytes themselves.
        Ok(None)
    }

    fn backend_label(&self) -> &'static str {
        "filesystem"
    }
}
