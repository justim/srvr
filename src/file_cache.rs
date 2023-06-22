use std::collections::HashMap;
use std::fs::Metadata;
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::HeaderValue;
use httpdate::HttpDate;
use tokio::sync::RwLock;

#[derive(Clone)]
pub enum FileCacheEntry {
    Found {
        content: Arc<Vec<u8>>,
        content_type: HeaderValue,
        last_modified: HttpDate,
    },

    NotFound,
}

#[derive(Default)]
pub struct FileCache {
    files: RwLock<HashMap<PathBuf, FileCacheEntry>>,
}

impl FileCache {
    pub async fn get(&self, path: &PathBuf) -> Option<FileCacheEntry> {
        self.files.read().await.get(path).map(Clone::clone)
    }

    async fn set(&self, path: PathBuf, entry: FileCacheEntry) -> FileCacheEntry {
        let mut files = self.files.write().await;
        files.insert(path.clone(), entry);

        files.get(&path).unwrap().clone()
    }

    pub async fn read_file(
        &self,
        meta: Metadata,
        content_path: PathBuf,
        content_type_path: &PathBuf,
    ) -> FileCacheEntry {
        match tokio::fs::read(&content_path).await {
            Ok(content) => {
                let mime = mime_guess::from_path(content_type_path)
                    .first_raw()
                    .map_or_else(
                        || HeaderValue::from_str(mime::APPLICATION_OCTET_STREAM.as_ref()).unwrap(),
                        HeaderValue::from_static,
                    );

                let entry = FileCacheEntry::Found {
                    content: Arc::new(content),
                    content_type: mime,
                    last_modified: HttpDate::from(meta.modified().unwrap()),
                };

                self.set(content_path, entry).await
            }

            Err(err) => {
                tracing::warn!("File error ({content_path:?}): {err}");

                let entry = FileCacheEntry::NotFound;

                self.set(content_path, entry).await
            }
        }
    }
}
