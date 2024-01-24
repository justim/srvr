use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

use axum::http::Uri;

use crate::encoding::ClientEncodingSupport;
use crate::encoding::Encoding;

fn append_to_path(path: impl Into<OsString>, suffix: impl AsRef<OsStr>) -> PathBuf {
    let mut path = path.into();
    path.push(suffix);
    path.into()
}

#[derive(Debug)]
pub struct PathToTry {
    path: PathBuf,
    encoding: Option<Encoding>,
    cache_control: Option<&'static str>,
}

impl PathToTry {
    #[inline]
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn content_path(&self) -> PathBuf {
        if let Some(encoding) = self.encoding {
            append_to_path(&self.path, encoding.get_extension())
        } else {
            self.path()
        }
    }

    #[inline]
    pub const fn encoding(&self) -> Option<Encoding> {
        self.encoding
    }

    #[inline]
    pub const fn cache_control(&self) -> Option<&'static str> {
        self.cache_control
    }
}

pub fn collect_paths_to_try(
    client_encoding_support: &ClientEncodingSupport,
    base_dir: &Path,
    fallback_path: &Path,
    uri: &Uri,
    initial_path: PathBuf,
) -> Vec<PathToTry> {
    let mut paths_to_try = vec![];

    // empty request will never match anything, skip to fallback
    if uri.path() != "/" {
        let path = base_dir.join(initial_path);

        for encoding in client_encoding_support.supported_encodings() {
            paths_to_try.push(PathToTry {
                path: path.clone(),
                encoding: Some(*encoding),
                cache_control: None,
            });
        }

        paths_to_try.push(PathToTry {
            path,
            encoding: None,
            cache_control: None,
        });
    }

    for encoding in client_encoding_support.supported_encodings() {
        paths_to_try.push(PathToTry {
            path: fallback_path.to_path_buf(),
            encoding: Some(*encoding),
            cache_control: Some("no-cache"),
        });
    }

    paths_to_try.push(PathToTry {
        path: fallback_path.to_path_buf(),
        encoding: None,
        cache_control: None,
    });

    paths_to_try
}
