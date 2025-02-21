//! Implementation of partial content responses.
//!
//! See <https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests>

use std::io::SeekFrom;
use std::sync::LazyLock;

use axum::body::Body;
use axum::http::header::CONTENT_LENGTH;
use axum::http::header::CONTENT_RANGE;
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use axum_extra::headers::Range;
use axum_extra::TypedHeader;
use mime::APPLICATION_OCTET_STREAM;
use tokio::fs::File;
use tokio::io::AsyncReadExt as _;
use tokio::io::AsyncSeekExt as _;
use tokio_util::io::ReaderStream;

static CONTENT_TYPE_APPLICATION_OCTET_STREAM: LazyLock<HeaderValue> = LazyLock::new(|| {
    HeaderValue::from_str(APPLICATION_OCTET_STREAM.as_ref())
        .expect("A valid application/octet-stream header value")
});

/// Process the range header and return the start and end of the range.
///
/// Only one part is supported.
///
/// # Errors
///
/// Will return an error if the range is not satisfiable. This error can be served directly
/// as a response.
fn process_range(content_length: u64, range: &TypedHeader<Range>) -> Result<(u64, u64), Response> {
    fn err() -> Result<(u64, u64), Response> {
        Err((
            StatusCode::RANGE_NOT_SATISFIABLE,
            "Requested Range Not Satisfiable",
        )
            .into_response())
    }

    if let Some((start, end)) = range.satisfiable_ranges(content_length).next() {
        let start = match start {
            std::ops::Bound::Included(start) => start,
            std::ops::Bound::Excluded(start) => start + 1,
            std::ops::Bound::Unbounded => 0,
        };

        let end = match end {
            std::ops::Bound::Included(end) => end,
            std::ops::Bound::Excluded(end) => end.saturating_sub(1),
            std::ops::Bound::Unbounded => content_length.saturating_sub(1),
        };

        tracing::trace!("Range response start: {}, end: {}", start, end);

        if start > content_length {
            return err();
        }

        if end < start {
            return err();
        }

        if end >= content_length {
            return err();
        }

        return Ok((start, end));
    }

    err()
}

/// Create a `Content-Range` header value.
fn content_range_range(start: u64, end: u64, content_length: u64) -> HeaderValue {
    HeaderValue::from_str(&format!("bytes {start}-{end}/{content_length}")).unwrap()
}

/// Apply the headers needed for a content range response.
fn apply_content_range_headers(headers: &mut HeaderMap, start: u64, end: u64, content_length: u64) {
    headers.insert(CONTENT_TYPE, CONTENT_TYPE_APPLICATION_OCTET_STREAM.clone());

    headers.insert(
        CONTENT_RANGE,
        content_range_range(start, end, content_length),
    );

    // overwrite the content length header, this should be the actual content that is sent
    headers.insert(CONTENT_LENGTH, (end - start + 1).into());
}

/// Serve a partial response from a cached response.
pub async fn serve_partial_cached_response(
    mut headers: HeaderMap,
    content: &[u8],
    content_length: u64,
    range: &TypedHeader<Range>,
) -> Response {
    match process_range(content_length, range) {
        Ok((start, end)) => {
            apply_content_range_headers(&mut headers, start, end, content_length);

            // the cached content is never bigger than `file_cache::FILE_SYSTEM_THRESHOLD`,
            // the start and end can be safely converted to usize
            let start = usize::try_from(start).expect("Start is a valid usize");
            let end = usize::try_from(end).expect("End is a valid usize");

            let body = content[start..=end].to_vec();

            (StatusCode::PARTIAL_CONTENT, headers, body).into_response()
        }
        Err(response) => response,
    }
}

/// Serve a partial response from a file.
pub async fn serve_partial_file_response(
    mut headers: HeaderMap,
    mut file: File,
    content_length: u64,
    range: &TypedHeader<Range>,
) -> Response {
    match process_range(content_length, range) {
        Ok((start, end)) => {
            apply_content_range_headers(&mut headers, start, end, content_length);

            // Seek and take the range of the file
            file.seek(SeekFrom::Start(start)).await.unwrap();
            let stream = ReaderStream::new(file.take(end - start + 1));

            (
                StatusCode::PARTIAL_CONTENT,
                headers,
                Body::from_stream(stream),
            )
                .into_response()
        }

        Err(response) => response,
    }
}
