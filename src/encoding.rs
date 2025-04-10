//! Encoding (compression) support utilities

use std::convert::Infallible;

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

/// Zstandard encoding in `accept-encoding` header
///
/// See <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Accept-Encoding>
const ENCODING_ZSTD: &str = "zstd";

/// Extension for zstd encoded files
const ENCODING_ZSTD_EXTENSION: &str = ".zst";

/// Supported encodings
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// Brotili compression (br)
    Brotli,

    /// Gzip compression (gzip)
    Gzip,

    /// Zstandard compression (zstd)
    Zstandard,
}

impl Encoding {
    /// Convert encoding to `HeaderValue`
    #[inline]
    pub const fn to_header_value(self) -> HeaderValue {
        match self {
            Encoding::Brotli => HeaderValue::from_static(ENCODING_BR),
            Encoding::Gzip => HeaderValue::from_static(ENCODING_GZIP),
            Encoding::Zstandard => HeaderValue::from_static(ENCODING_ZSTD),
        }
    }

    /// Get extension for encoding
    #[inline]
    pub const fn get_extension(self) -> &'static str {
        match self {
            Encoding::Brotli => ENCODING_BR_EXTENSION,
            Encoding::Gzip => ENCODING_GZIP_EXTENSION,
            Encoding::Zstandard => ENCODING_ZSTD_EXTENSION,
        }
    }
}

/// Client encoding support
#[derive(Default)]
pub struct ClientEncodingSupport {
    /// Support for Brotli encoding
    has_brotli: bool,

    /// Support for Gzip encoding
    has_gzip: bool,

    /// Support for Zstandard encoding
    has_zstandard: bool,
}

impl ClientEncodingSupport {
    /// Create new `ClientEncodingSupport` from `HeaderMap`
    ///
    /// Will check for `accept-encoding` header and check if it contains
    /// `br` or `gzip` encoding
    fn from_header_map(incoming_headers: &HeaderMap) -> Self {
        let mut support = Self::default();

        let encodings = incoming_headers
            .get(ACCEPT_ENCODING)
            .and_then(|encoding| encoding.to_str().ok())
            .map(|encoding| encoding.split(',').map(str::trim).collect::<Vec<&str>>());

        if let Some(encodings) = encodings {
            support.has_brotli = Self::check_support(&encodings, ENCODING_BR);
            support.has_gzip = Self::check_support(&encodings, ENCODING_GZIP);
            support.has_zstandard = Self::check_support(&encodings, ENCODING_ZSTD);
        }

        support
    }

    /// Check if the client supports the given encoding
    ///
    /// This also checks the quality value of the encoding:
    /// - `Accept-Encoding: deflate, gzip;q=1.0, *;q=0.5` would match `"gzip"`
    ///
    /// See <https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Accept-Encoding>
    fn check_support(encodings: &[&str], encoding_name: &str) -> bool {
        encodings.iter().any(|encoding| {
            match encoding.split_once(';') {
                Some((encoding, _)) => encoding.trim(),
                None => encoding,
            }
            .eq_ignore_ascii_case(encoding_name)
        })
    }

    /// Get list of supported encodings
    ///
    /// List is in order of quality, highest quality first:
    ///
    /// - Brotli
    /// - Zstandard
    /// - Gzip
    ///
    /// Brotli has generally the best compression ratio, but takes the longest to compress.
    /// Our use case is loading pre-compressed files, so we want to serve the most compressed,
    /// regardless of how long it took to compress.
    #[inline]
    pub const fn supported_encodings(&self) -> &[Encoding] {
        match (self.has_brotli, self.has_gzip, self.has_zstandard) {
            (true, true, true) => &[Encoding::Brotli, Encoding::Zstandard, Encoding::Gzip],
            (true, true, false) => &[Encoding::Brotli, Encoding::Gzip],
            (true, false, true) => &[Encoding::Brotli, Encoding::Zstandard],
            (false, true, true) => &[Encoding::Zstandard, Encoding::Gzip],
            (true, false, false) => &[Encoding::Brotli],
            (false, false, true) => &[Encoding::Zstandard],
            (false, true, false) => &[Encoding::Gzip],
            (false, false, false) => &[],
        }
    }
}

impl<S> FromRequestParts<S> for ClientEncodingSupport
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(ClientEncodingSupport::from_header_map(&parts.headers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_support() {
        let encodings = &["gzip", "brotli"];

        assert!(ClientEncodingSupport::check_support(encodings, "gzip"));
        assert!(ClientEncodingSupport::check_support(encodings, "brotli"));
        assert!(!ClientEncodingSupport::check_support(encodings, "deflate"),);
    }

    #[test]
    fn test_check_support_with_quality() {
        let encodings = &["gzip;q=1.0", "brotli"];

        assert!(ClientEncodingSupport::check_support(encodings, "gzip"));
        assert!(ClientEncodingSupport::check_support(encodings, "brotli"));
        assert!(!ClientEncodingSupport::check_support(encodings, "deflate"),);
    }

    #[test]
    fn test_supported_encodings() {
        let mut support = ClientEncodingSupport {
            has_gzip: true,
            has_brotli: true,
            has_zstandard: true,
        };

        assert_eq!(
            support.supported_encodings(),
            &[Encoding::Brotli, Encoding::Zstandard, Encoding::Gzip],
        );

        support.has_gzip = false;
        assert_eq!(
            support.supported_encodings(),
            &[Encoding::Brotli, Encoding::Zstandard]
        );

        support.has_brotli = false;
        assert_eq!(support.supported_encodings(), &[Encoding::Zstandard]);

        support.has_zstandard = false;
        assert_eq!(support.supported_encodings(), &[]);

        support.has_gzip = true;
        assert_eq!(support.supported_encodings(), &[Encoding::Gzip]);
    }

    #[test]
    fn test_simple_header_map() {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, Br"));

        let support = ClientEncodingSupport::from_header_map(&headers);

        assert!(support.has_gzip);
        assert!(support.has_brotli);
        assert!(!support.has_zstandard);

        assert_eq!(
            &[Encoding::Brotli, Encoding::Gzip],
            support.supported_encodings(),
        );
    }

    #[test]
    fn test_header_map_with_quality() {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT_ENCODING,
            HeaderValue::from_static("gzip;q=1.0, br ;  0.5"),
        );

        let support = ClientEncodingSupport::from_header_map(&headers);

        assert!(support.has_gzip);
        assert!(support.has_brotli);
        assert!(!support.has_zstandard);

        assert_eq!(
            &[Encoding::Brotli, Encoding::Gzip],
            support.supported_encodings(),
        );
    }

    #[test]
    fn test_empty_header_map() {
        let headers = HeaderMap::new();

        let support = ClientEncodingSupport::from_header_map(&headers);

        assert!(!support.has_gzip);
        assert!(!support.has_brotli);

        assert!(support.supported_encodings().is_empty());
    }

    #[test]
    fn test_simple_header_map_with_deflate() {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("deflate"));

        let support = ClientEncodingSupport::from_header_map(&headers);

        assert!(!support.has_gzip);
        assert!(!support.has_brotli);

        assert!(support.supported_encodings().is_empty());
    }
}
