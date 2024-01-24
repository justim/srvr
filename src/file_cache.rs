use std::collections::HashMap;
use std::fs::Metadata;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use axum::http::HeaderValue;
use httpdate::HttpDate;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::RwLock;

/// Threshold for which to start using the file system for serving files, ie _not_ to use the cache
const FILE_SYSTEM_THRESHOLD: u64 = 65_536;

#[derive(Clone)]
pub enum FileCacheEntryContent {
    Cached(Arc<Vec<u8>>),
    File,
}

#[derive(Clone)]
pub enum FileCacheEntry {
    Found {
        content: FileCacheEntryContent,
        content_type: HeaderValue,
        content_length: u64,
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

        files.get(&path).expect("Just inserted entry").clone()
    }

    pub async fn read_file(
        &self,
        meta: Metadata,
        content_path: PathBuf,
        content_type_path: &PathBuf,
    ) -> FileCacheEntry {
        match File::open(&content_path).await {
            Ok(mut file) => {
                let mime = mime_guess::from_path(content_type_path)
                    .first_raw()
                    .map_or_else(
                        || {
                            HeaderValue::from_str(mime::APPLICATION_OCTET_STREAM.as_ref())
                                .expect("A valid application/octet-stream header value")
                        },
                        HeaderValue::from_static,
                    );

                let content = if meta.len() > FILE_SYSTEM_THRESHOLD {
                    tracing::trace!("Using file system to serve file");

                    FileCacheEntryContent::File
                } else {
                    tracing::trace!("Using cache to serve file");

                    // SAFEFY: We check that the file size is below the threshold
                    let mut bytes = Vec::with_capacity(
                        usize::try_from(meta.len()).expect("Valid u64 -> usize conversion"),
                    );

                    if let Err(err) = file.read_to_end(&mut bytes).await {
                        tracing::warn!("Could not read file into cache ({content_path:?}): {err}");
                        return FileCacheEntry::NotFound;
                    }

                    FileCacheEntryContent::Cached(Arc::new(bytes))
                };

                let entry = FileCacheEntry::Found {
                    content,
                    content_type: mime,
                    content_length: meta.len(),
                    last_modified: HttpDate::from(
                        meta.modified().unwrap_or_else(|_| SystemTime::now()),
                    ),
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
