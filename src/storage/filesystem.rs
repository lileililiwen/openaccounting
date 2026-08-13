use crate::error::AppError;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct FilesystemStore {
    root: PathBuf,
}

impl FilesystemStore {
    pub async fn new(root: impl Into<PathBuf>) -> Result<Self, AppError> {
        let root = root.into();
        fs::create_dir_all(&root).await?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the absolute path where the file should be stored. The caller
    /// is responsible for actually writing the bytes.
    pub fn allocate_path(&self, transaction_id: Uuid, original: &str) -> Result<PathBuf, AppError> {
        let safe = sanitize_filename::sanitize(original);
        if safe.is_empty() {
            return Err(AppError::Validation("invalid filename".into()));
        }
        let tx_dir = self.root.join(transaction_id.to_string());
        let stored = format!("{}-{}", Uuid::new_v4(), safe);
        Ok(tx_dir.join(stored))
    }

    pub async fn ensure_dir(&self, path: &Path) -> Result<(), AppError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        Ok(())
    }

    pub async fn read(&self, path: &Path) -> Result<Vec<u8>, AppError> {
        Ok(fs::read(path).await?)
    }

    pub async fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), AppError> {
        self.ensure_dir(path).await?;
        fs::write(path, bytes).await?;
        Ok(())
    }

    pub async fn delete(&self, path: &Path) -> Result<(), AppError> {
        if path.exists() {
            fs::remove_file(path).await?;
            if let Some(parent) = path.parent() {
                let _ = fs::remove_dir(parent).await; // best effort
            }
        }
        Ok(())
    }
}
