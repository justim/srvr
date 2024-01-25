use std::convert::Infallible;

use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::header::ACCEPT_ENCODING;
use axum::http::request::Parts;
use axum::http::HeaderMap;
use axum::http::HeaderValue;

/// Brotli encoding in `accept-encoding` header
///
/// See <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Accept-Encoding>
const ENCODING_BR: &str = "br";

/// Extension for Brotli encoded files
const ENCODING_BR_EXTENSION: &str = ".br";

/// Gzip encoding in `accept-encoding` header
///
/// See <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Accept-Encoding>
const ENCODING_GZIP: &str = "gzip";

/// Extension for gzip encoded files
const ENCODING_GZIP_EXTENSION: &str = ".gz";

#[derive(Clone, Copy, Debug)]
pub enum Encoding {
    Brotli,
    Gzip,
}

impl Encoding {
    #[inline]
    pub const fn to_header_value(self) -> HeaderValue {
        match self {
            Encoding::Brotli => HeaderValue::from_static(ENCODING_BR),
            Encoding::Gzip => HeaderValue::from_static(ENCODING_GZIP),
        }
    }

    #[inline]
    pub const fn get_extension(self) -> &'static str {
        match self {
            Encoding::Brotli => ENCODING_BR_EXTENSION,
            Encoding::Gzip => ENCODING_GZIP_EXTENSION,
        }
    }
}

#[derive(Default)]
pub struct ClientEncodingSupport {
    has_brotli: bool,
    has_gzip: bool,
}

impl ClientEncodingSupport {
    fn from_header_map(incoming_headers: &HeaderMap) -> Self {
        let mut support = Self::default();

        let encodings = incoming_headers
            .get(ACCEPT_ENCODING)
            .and_then(|encoding| encoding.to_str().ok())
            .map(|encoding| encoding.split(',').map(str::trim).collect::<Vec<&str>>());

        support.has_brotli = Self::check_support(&encodings, ENCODING_BR);
        support.has_gzip = Self::check_support(&encodings, ENCODING_GZIP);

        support
    }

    fn check_support(encodings: &Option<Vec<&str>>, encoding_name: &str) -> bool {
        encodings.as_ref().map_or(false, |encodings| {
            encodings
                .iter()
                .any(|encoding| encoding.eq_ignore_ascii_case(encoding_name))
        })
    }

    #[inline]
    pub const fn supported_encodings(&self) -> &[Encoding] {
        match (self.has_brotli, self.has_gzip) {
            (true, true) => &[Encoding::Brotli, Encoding::Gzip],
            (true, false) => &[Encoding::Brotli],
            (false, true) => &[Encoding::Gzip],
            (false, false) => &[],
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for ClientEncodingSupport
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(ClientEncodingSupport::from_header_map(&parts.headers))
    }
}
