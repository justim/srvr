use std::path::Component;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use axum::extract::State;
use axum::http::header::CACHE_CONTROL;
use axum::http::header::CONTENT_ENCODING;
use axum::http::header::CONTENT_LENGTH;
use axum::http::header::CONTENT_TYPE;
use axum::http::header::LAST_MODIFIED;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::Method;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::Router;
use axum_extra::body::AsyncReadBody;
use axum_extra::headers::IfModifiedSince;
use axum_extra::TypedHeader;
use httpdate::HttpDate;
use humantime::format_duration;
use percent_encoding::percent_decode_str;
use tokio::fs::File;
use tower_http::timeout::RequestBodyTimeoutLayer;
use tower_http::timeout::ResponseBodyTimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::config::Config;
use crate::encoding::ClientEncodingSupport;
use crate::file_cache::FileCache;
use crate::file_cache::FileCacheEntry;
use crate::file_cache::FileCacheEntryContent;
use crate::paths::collect_paths_to_try;
use crate::paths::PathToTry;

const DEFAULT_FALLBACK_PATH: &str = "index.html";

#[derive(Clone)]
pub struct ServerState {
    config: Config,
    fallback_path: PathBuf,
    file_cache: Arc<FileCache>,
}

impl ServerState {
    pub fn from_config(config: Config) -> Self {
        let fallback_path = config.fallback_path.as_ref().map_or_else(
            || config.base_dir.join(DEFAULT_FALLBACK_PATH),
            PathBuf::from,
        );

        Self {
            config,
            fallback_path,
            file_cache: Arc::default(),
        }
    }
}

pub fn app(state: ServerState) -> Router {
    Router::new()
        .fallback(root)
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    tracing::info_span!(
                        "req",
                        status = tracing::field::Empty,
                        path = &tracing::field::display(request.uri()),
                        latency = tracing::field::Empty,
                    )
                })
                .on_request(|_request: &Request<_>, _span: &Span| {
                    tracing::debug!("Incoming request");
                })
                .on_response(|response: &Response, latency: Duration, span: &Span| {
                    span.record("status", &tracing::field::display(response.status()));
                    span.record("latency", format_duration(latency).to_string());

                    tracing::info!("Finished request");
                }),
        )
        .layer(RequestBodyTimeoutLayer::new(Duration::from_secs(5)))
        .layer(ResponseBodyTimeoutLayer::new(Duration::from_secs(5)))
}

enum ServeFileResponse {
    Found {
        headers: HeaderMap,
        content: FileCacheEntryContent,
    },
    NotModified {
        headers: HeaderMap,
    },
    NotFound,
}

async fn serve_file(
    file_cache: &FileCache,
    path_to_try: &PathToTry,
    if_modified_since: &Option<TypedHeader<IfModifiedSince>>,
) -> ServeFileResponse {
    let content_type_path = path_to_try.path();
    let content_path = path_to_try.content_path();

    let Ok(meta) = tokio::fs::metadata(&content_path).await else {
        return ServeFileResponse::NotFound;
    };

    let entry = if let Some(entry) = file_cache.get(&content_path).await {
        tracing::trace!("Cache hit, serving from cache");

        let file_last_modified = HttpDate::from(meta.modified().unwrap_or_else(|_| {
            // there is no way to known when the file was modified, just assume it is
            // fresh -- not ideal, but better than a crash :)
            SystemTime::now()
        }));

        match entry {
            FileCacheEntry::Found { last_modified, .. } => {
                if file_last_modified > last_modified {
                    tracing::trace!("Newer file on disk, reloading");

                    file_cache
                        .read_file(meta, content_path, &content_type_path)
                        .await
                } else {
                    entry
                }
            }

            FileCacheEntry::NotFound => entry,
        }
    } else {
        tracing::trace!("Cache miss, going to file system");

        file_cache
            .read_file(meta, content_path, &content_type_path)
            .await
    };

    match entry {
        FileCacheEntry::Found {
            content,
            content_type,
            content_length,
            last_modified,
        } => {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, content_type);

            match HeaderValue::from_str(&last_modified.to_string()) {
                Ok(last_modified) => {
                    headers.insert(LAST_MODIFIED, last_modified);
                }

                Err(err) => {
                    tracing::warn!("Could not set last modified header: {err}");
                }
            };

            if let Some(if_modified_since) = if_modified_since {
                if !if_modified_since.is_modified(last_modified.into()) {
                    tracing::trace!("Client has latest version");
                    headers.insert(CONTENT_LENGTH, 0.into());
                    return ServeFileResponse::NotModified { headers };
                }
            }

            headers.insert(CONTENT_LENGTH, content_length.into());

            ServeFileResponse::Found { headers, content }
        }

        FileCacheEntry::NotFound => ServeFileResponse::NotFound,
    }
}

async fn root(
    state: State<ServerState>,
    method: Method,
    uri: Uri,
    client_encoding_support: ClientEncodingSupport,
    if_modified_since: Option<TypedHeader<IfModifiedSince>>,
) -> Response {
    let path = uri.path().trim_start_matches('/');

    let Ok(path) = percent_decode_str(path).decode_utf8() else {
        // we received something funky, just bail
        return StatusCode::NOT_FOUND.into_response();
    };

    let path = PathBuf::from(&*path);

    // quick check to see if there are any weird path traversal tricks
    let is_valid = path
        .components()
        .all(|comp| matches!(comp, Component::Normal(_)));

    if !is_valid {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let paths_to_try = collect_paths_to_try(
        &client_encoding_support,
        &state.config.base_dir,
        &state.fallback_path,
        &uri,
        path,
    );

    for path_to_try in paths_to_try {
        tracing::trace!("Trying path: {path_to_try:?}");

        match serve_file(&state.file_cache, &path_to_try, &if_modified_since).await {
            ServeFileResponse::Found {
                mut headers,
                content,
            } => {
                if let Some(encoding) = path_to_try.encoding() {
                    headers.append(CONTENT_ENCODING, encoding.to_header_value());
                }

                if let Some(cache_control) = path_to_try.cache_control() {
                    headers.append(CACHE_CONTROL, HeaderValue::from_static(cache_control));
                }

                return if method == Method::HEAD {
                    // HEAD-method expects no content
                    (StatusCode::OK, headers).into_response()
                } else {
                    match content {
                        FileCacheEntryContent::Cached(content) => {
                            (StatusCode::OK, headers, content.to_vec()).into_response()
                        }

                        FileCacheEntryContent::File => {
                            match File::open(&path_to_try.content_path()).await {
                                Ok(file) => {
                                    let body = AsyncReadBody::new(file);
                                    (StatusCode::OK, headers, body).into_response()
                                }

                                Err(err) => {
                                    tracing::warn!("File is no longer available: {err}");
                                    StatusCode::NOT_FOUND.into_response()
                                }
                            }
                        }
                    }
                };
            }

            ServeFileResponse::NotModified { headers } => {
                return (StatusCode::NOT_MODIFIED, headers).into_response();
            }

            ServeFileResponse::NotFound => {
                // try the next path
            }
        }
    }

    StatusCode::NOT_FOUND.into_response()
}
