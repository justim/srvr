use axum::http::header::ACCEPT_ENCODING;
use axum::http::HeaderMap;
use axum::http::HeaderValue;

#[derive(Clone, Copy, Debug)]
pub enum Encoding {
    Brotli,
    Gzip,
}

impl Encoding {
    #[inline]
    pub const fn to_header_value(self) -> HeaderValue {
        match self {
            Encoding::Brotli => HeaderValue::from_static("br"),
            Encoding::Gzip => HeaderValue::from_static("gzip"),
        }
    }

    #[inline]
    pub const fn get_extension(self) -> &'static str {
        match self {
            Encoding::Brotli => ".br",
            Encoding::Gzip => ".gz",
        }
    }
}

#[derive(Default)]
pub struct ClientEncodingSupport {
    has_brotli: bool,
    has_gzip: bool,
}

impl ClientEncodingSupport {
    pub fn from_header_map(incoming_headers: &HeaderMap) -> Self {
        let mut support = Self::default();

        let encodings = incoming_headers
            .get(ACCEPT_ENCODING)
            .and_then(|encoding| encoding.to_str().ok())
            .map(|encoding| encoding.split(',').map(str::trim).collect::<Vec<&str>>());

        support.has_brotli = Self::check_support(&encodings, "br");
        support.has_gzip = Self::check_support(&encodings, "gzip");

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
