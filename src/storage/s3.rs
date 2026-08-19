//! S3-compatible [`Storage`] implementation (`o6-s3-storage`).
//!
//! Optional; compiled only with `--features storage-s3`. We use
//! the official `aws-sdk-s3` crate so any S3-compatible
//! endpoint works (AWS, MinIO, Cloudflare R2, Hetzner Object
//! Storage, etc.).

use super::{Storage, StorageError, StorageKey};
use async_trait::async_trait;
use aws_config::{BehaviorVersion, Region};
use aws_credential_types::Credentials;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

// Helper to build static credentials from raw strings without
// pulling in the full credentials chain.
fn static_credentials(
    access_key_id: String,
    secret_access_key: String,
) -> aws_credential_types::Credentials {
    aws_credential_types::Credentials::new(
        access_key_id,
        secret_access_key,
        None,
        None,
        "openaccounting-static",
    )
}

#[derive(Clone, Debug)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub endpoint_url: Option<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
    /// Optional key prefix (e.g. `documents/`). Default: empty.
    pub key_prefix: String,
}

impl S3Config {
    pub fn from_env() -> Result<Self, StorageError> {
        let bucket = std::env::var("S3_BUCKET").map_err(|_| {
            StorageError::Other("S3_BUCKET must be set when STORAGE_BACKEND=s3".into())
        })?;
        let region = std::env::var("S3_REGION").map_err(|_| {
            StorageError::Other("S3_REGION must be set when STORAGE_BACKEND=s3".into())
        })?;
        let access_key_id = std::env::var("S3_ACCESS_KEY_ID").map_err(|_| {
            StorageError::Other("S3_ACCESS_KEY_ID must be set when STORAGE_BACKEND=s3".into())
        })?;
        let secret_access_key = std::env::var("S3_SECRET_ACCESS_KEY").map_err(|_| {
            StorageError::Other("S3_SECRET_ACCESS_KEY must be set when STORAGE_BACKEND=s3".into())
        })?;
        Ok(Self {
            bucket,
            region,
            endpoint_url: std::env::var("S3_ENDPOINT_URL").ok(),
            access_key_id,
            secret_access_key,
            key_prefix: std::env::var("S3_KEY_PREFIX").unwrap_or_default(),
        })
    }
}

pub struct S3Store {
    cfg: Arc<S3Config>,
    client: Client,
}

impl std::fmt::Debug for S3Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Store")
            .field("bucket", &self.cfg.bucket)
            .field("region", &self.cfg.region)
            .field("endpoint_url", &self.cfg.endpoint_url)
            .field("key_prefix", &self.cfg.key_prefix)
            .finish()
    }
}

impl S3Store {
    pub async fn new(cfg: S3Config) -> Result<Self, StorageError> {
        let creds = static_credentials(cfg.access_key_id.clone(), cfg.secret_access_key.clone());
        let region = Region::new(cfg.region.clone());
        let mut loader = aws_config::defaults(BehaviorVersion::latest())
            .region(region)
            .credentials_provider(creds);
        if let Some(endpoint) = &cfg.endpoint_url {
            loader = loader.endpoint_url(endpoint);
        }
        let aws_cfg = loader.load().await;
        let mut s3_conf = aws_sdk_s3::config::Builder::from(&aws_cfg);
        if let Some(endpoint) = &cfg.endpoint_url {
            s3_conf = s3_conf.endpoint_url(endpoint);
        }
        let client = Client::from_conf(s3_conf.build());
        Ok(Self {
            cfg: Arc::new(cfg),
            client,
        })
    }

    fn make_key(
        &self,
        transaction_id: Uuid,
        original: &str,
    ) -> Result<(String, String), StorageError> {
        let safe = sanitize_filename::sanitize(original);
        if safe.is_empty() {
            return Err(StorageError::InvalidFilename);
        }
        let stem = format!("{}-{}", Uuid::new_v4(), safe);
        let full = if self.cfg.key_prefix.is_empty() {
            format!("{transaction_id}/{stem}")
        } else {
            format!("{}/{transaction_id}/{stem}", self.cfg.key_prefix)
        };
        Ok((self.cfg.bucket.clone(), full))
    }
}

#[async_trait]
impl Storage for S3Store {
    async fn allocate_path(
        &self,
        transaction_id: Uuid,
        original: &str,
    ) -> Result<StorageKey, StorageError> {
        let (bucket, key) = self.make_key(transaction_id, original)?;
        Ok(StorageKey::S3 { bucket, key })
    }

    fn key_from_stored(&self, stored: &str) -> Result<StorageKey, StorageError> {
        let full = if self.cfg.key_prefix.is_empty() {
            stored.to_string()
        } else {
            format!("{}/{}", self.cfg.key_prefix, stored)
        };
        Ok(StorageKey::S3 {
            bucket: self.cfg.bucket.clone(),
            key: full,
        })
    }

    fn root_for(&self) -> PathBuf {
        PathBuf::new()
    }

    async fn read(&self, key: &StorageKey) -> Result<Vec<u8>, StorageError> {
        let (bucket, key) = match key {
            StorageKey::S3 { bucket, key } => (bucket.clone(), key.clone()),
            _ => return Err(StorageError::NotFound),
        };
        let out = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::S3(format!("get_object: {e}")))?;
        let bytes = out
            .body
            .collect()
            .await
            .map_err(|e| StorageError::S3(format!("collect body: {e}")))?;
        Ok(bytes.into_bytes().to_vec())
    }

    async fn write(&self, key: &StorageKey, bytes: &[u8]) -> Result<(), StorageError> {
        let (bucket, key) = match key {
            StorageKey::S3 { bucket, key } => (bucket.clone(), key.clone()),
            _ => return Err(StorageError::NotFound),
        };
        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(ByteStream::from(bytes.to_vec()))
            .send()
            .await
            .map_err(|e| StorageError::S3(format!("put_object: {e}")))?;
        Ok(())
    }

    async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
        let (bucket, key) = match key {
            StorageKey::S3 { bucket, key } => (bucket.clone(), key.clone()),
            _ => return Err(StorageError::NotFound),
        };
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StorageError::S3(format!("delete_object: {e}")))?;
        Ok(())
    }

    async fn signed_url(
        &self,
        key: &StorageKey,
        ttl_secs: u32,
    ) -> Result<Option<String>, StorageError> {
        let (bucket, key) = match key {
            StorageKey::S3 { bucket, key } => (bucket.clone(), key.clone()),
            _ => return Ok(None),
        };
        let presign = PresigningConfig::expires_in(Duration::from_secs(ttl_secs as u64))
            .map_err(|e| StorageError::S3(format!("presign config: {e}")))?;
        let req = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .presigned(presign)
            .await
            .map_err(|e| StorageError::S3(format!("presign: {e}")))?;
        Ok(Some(req.uri().to_string()))
    }

    fn backend_label(&self) -> &'static str {
        "s3"
    }
}
