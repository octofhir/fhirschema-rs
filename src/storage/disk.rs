#[cfg(feature = "disk-storage")]
use std::path::{Path, PathBuf};
#[cfg(feature = "disk-storage")]
use tokio::fs;
use url::Url;

use super::SchemaStorage;
use crate::{FhirSchema, FhirSchemaError, Result};

#[cfg(feature = "disk-storage")]
#[derive(Debug, Clone)]
pub struct DiskStorageConfig {
    pub base_path: PathBuf,
    pub create_directories: bool,
    pub file_extension: String,
}

#[cfg(feature = "disk-storage")]
impl Default for DiskStorageConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("./schemas"),
            create_directories: true,
            file_extension: "json".to_string(),
        }
    }
}

#[cfg(feature = "disk-storage")]
#[derive(Debug)]
pub struct DiskStorage {
    config: DiskStorageConfig,
}

#[cfg(feature = "disk-storage")]
impl DiskStorage {
    pub fn new(config: DiskStorageConfig) -> Result<Self> {
        Ok(Self { config })
    }

    pub async fn initialize(&self) -> Result<()> {
        if self.config.create_directories {
            fs::create_dir_all(&self.config.base_path)
                .await
                .map_err(|e| FhirSchemaError::Storage {
                    message: format!("Failed to create storage directory: {e}"),
                })?;
        }
        Ok(())
    }

    fn url_to_path(&self, url: &Url) -> PathBuf {
        let host = url.host_str().unwrap_or("unknown");
        let path = url.path().trim_start_matches('/');
        let filename = format!(
            "{}.{}",
            path.replace(['/', ':'], "_"),
            self.config.file_extension
        );

        self.config.base_path.join(host).join(filename)
    }

    fn path_to_url(&self, path: &Path) -> Result<Url> {
        let relative =
            path.strip_prefix(&self.config.base_path)
                .map_err(|_| FhirSchemaError::Storage {
                    message: "Invalid path for URL conversion".to_string(),
                })?;

        let host = relative
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .ok_or_else(|| FhirSchemaError::Storage {
                message: "Cannot extract host from path".to_string(),
            })?;

        let filename = relative
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| FhirSchemaError::Storage {
                message: "Cannot extract filename from path".to_string(),
            })?;

        let url_path = filename.replace('_', "/");
        let url_str = format!("https://{host}/{url_path}");

        Url::parse(&url_str).map_err(|e| FhirSchemaError::Storage {
            message: format!("Failed to parse URL from path: {e}"),
        })
    }
}

#[cfg(feature = "disk-storage")]
#[async_trait::async_trait]
impl SchemaStorage for DiskStorage {
    async fn get(&self, url: &Url) -> Result<Option<FhirSchema>> {
        let path = self.url_to_path(url);

        match fs::read_to_string(&path).await {
            Ok(content) => {
                let schema: FhirSchema =
                    serde_json::from_str(&content).map_err(|e| FhirSchemaError::Storage {
                        message: format!("Failed to deserialize schema: {e}"),
                    })?;
                Ok(Some(schema))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(FhirSchemaError::Storage {
                message: format!("Failed to read schema file: {e}"),
            }),
        }
    }

    async fn put(&self, url: Url, schema: FhirSchema) -> Result<()> {
        let path = self.url_to_path(&url);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| FhirSchemaError::Storage {
                    message: format!("Failed to create directory: {e}"),
                })?;
        }

        let content =
            serde_json::to_string_pretty(&schema).map_err(|e| FhirSchemaError::Storage {
                message: format!("Failed to serialize schema: {e}"),
            })?;

        fs::write(&path, content)
            .await
            .map_err(|e| FhirSchemaError::Storage {
                message: format!("Failed to write schema file: {e}"),
            })?;

        Ok(())
    }

    async fn remove(&self, url: &Url) -> Result<bool> {
        let path = self.url_to_path(url);

        match fs::remove_file(&path).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(FhirSchemaError::Storage {
                message: format!("Failed to remove schema file: {e}"),
            }),
        }
    }

    async fn list(&self) -> Result<Vec<Url>> {
        let mut urls = Vec::new();
        let mut stack = vec![self.config.base_path.clone()];

        while let Some(dir) = stack.pop() {
            let mut entries = fs::read_dir(&dir)
                .await
                .map_err(|e| FhirSchemaError::Storage {
                    message: format!("Failed to read directory: {e}"),
                })?;

            while let Some(entry) =
                entries
                    .next_entry()
                    .await
                    .map_err(|e| FhirSchemaError::Storage {
                        message: format!("Failed to read directory entry: {e}"),
                    })?
            {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str())
                    == Some(&self.config.file_extension)
                {
                    if let Ok(url) = self.path_to_url(&path) {
                        urls.push(url);
                    }
                }
            }
        }

        Ok(urls)
    }

    async fn contains(&self, url: &Url) -> Result<bool> {
        let path = self.url_to_path(url);
        Ok(path.exists())
    }

    async fn clear(&self) -> Result<()> {
        if self.config.base_path.exists() {
            fs::remove_dir_all(&self.config.base_path)
                .await
                .map_err(|e| FhirSchemaError::Storage {
                    message: format!("Failed to clear storage directory: {e}"),
                })?;
        }

        if self.config.create_directories {
            self.initialize().await?;
        }

        Ok(())
    }

    async fn size(&self) -> Result<usize> {
        self.list().await.map(|urls| urls.len())
    }
}

#[cfg(not(feature = "disk-storage"))]
compile_error!("disk-storage feature is required for DiskStorage");
