use std::path::Component;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::header::CACHE_CONTROL;
use axum::http::header::CONTENT_ENCODING;
use axum::http::header::CONTENT_LENGTH;
use axum::http::header::CONTENT_TYPE;
use axum::http::header::IF_MODIFIED_SINCE;
use axum::http::header::LAST_MODIFIED;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::Method;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::response::Response;
use axum::Router;
use httpdate::HttpDate;
use humantime::format_duration;
use percent_encoding::percent_decode_str;
use tower_http::trace::TraceLayer;
use tracing::Span;

use crate::config::Config;
use crate::encoding::ClientEncodingSupport;
use crate::file_cache::FileCache;
use crate::file_cache::FileCacheEntry;
use crate::paths::collect_paths_to_try;
use crate::paths::PathToTry;

#[derive(Clone)]
pub struct ServerState {
    config: Config,
    file_cache: Arc<FileCache>,
}

impl ServerState {
    pub fn from_config(config: Config) -> Self {
        Self {
            config,
            file_cache: Arc::default(),
        }
    }
}

pub fn app(state: ServerState) -> Router {
    Router::new().fallback(root).with_state(state).layer(
        TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| {
                tracing::info_span!(
                    "req",
                    status = tracing::field::Empty,
                    path = &tracing::field::display(request.uri()),
                    latency = tracing::field::Empty,
                )
            })
            .on_request(|_request: &Request<Body>, _span: &Span| {
                tracing::debug!("Incoming request");
            })
            .on_response(|response: &Response, latency: Duration, span: &Span| {
                span.record("status", &tracing::field::display(response.status()));
                span.record("latency", format_duration(latency).to_string());

                tracing::info!("Finished request");
            }),
    )
}

enum ServeFileResponse {
    Found {
        headers: HeaderMap,
        content: Vec<u8>,
    },
    NotModified {
        headers: HeaderMap,
    },
    NotFound,
}

async fn serve_file(
    file_cache: &FileCache,
    path_to_try: &PathToTry,
    if_modified_since: &Option<HttpDate>,
) -> ServeFileResponse {
    let content_type_path = path_to_try.path();
    let content_path = path_to_try.content_path();

    let Ok(meta) = tokio::fs::metadata(&content_path).await else {
        return ServeFileResponse::NotFound;
    };

    let entry = if let Some(entry) = file_cache.get(&content_path).await {
        tracing::trace!("Cache hit, serving from cache");

        let file_last_modified = HttpDate::from(meta.modified().unwrap());

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
            last_modified,
        } => {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, content_type);
            headers.insert(
                LAST_MODIFIED,
                HeaderValue::from_str(&last_modified.to_string()).unwrap(),
            );

            if let Some(if_modified_since) = if_modified_since {
                if last_modified <= *if_modified_since {
                    tracing::trace!("Client has latest version");
                    headers.insert(CONTENT_LENGTH, 0.into());
                    return ServeFileResponse::NotModified { headers };
                }
            }

            headers.insert(CONTENT_LENGTH, content.len().into());

            ServeFileResponse::Found {
                headers,
                content: content.to_vec(),
            }
        }

        FileCacheEntry::NotFound => ServeFileResponse::NotFound,
    }
}

async fn root(
    state: State<ServerState>,
    method: Method,
    uri: Uri,
    incoming_headers: HeaderMap,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let path = uri.path().trim_start_matches('/');
    let path = percent_decode_str(path).decode_utf8().ok().unwrap();
    let path = PathBuf::from(&*path);

    // quick check to see if there are any weird path traversal tricks
    let is_valid = path
        .components()
        .all(|comp| matches!(comp, Component::Normal(_)));

    if !is_valid {
        return (StatusCode::FOUND, HeaderMap::new(), vec![]);
    }

    let client_encoding_support = ClientEncodingSupport::from_header_map(&incoming_headers);

    let if_modified_since = incoming_headers
        .get(IF_MODIFIED_SINCE)
        .and_then(|if_modified_since| if_modified_since.to_str().ok())
        .and_then(|if_modified_since| HttpDate::from_str(if_modified_since).ok());

    let paths_to_try =
        collect_paths_to_try(&client_encoding_support, &state.config.base_dir, &uri, path);

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
                    (StatusCode::OK, headers, vec![])
                } else {
                    (StatusCode::OK, headers, content)
                };
            }

            ServeFileResponse::NotModified { headers } => {
                return (StatusCode::NOT_MODIFIED, headers, vec![]);
            }

            ServeFileResponse::NotFound => {
                // try the next path
            }
        }
    }

    (StatusCode::NOT_FOUND, HeaderMap::new(), vec![])
}
