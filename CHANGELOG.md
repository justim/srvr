# Changelog

## srvr

> So simple, even the vowels are not needed

## Version `0.1.2`

### Features

-   Add support for content range responses
-   Add support for Zstandard encoding
-   Use command line args to determine verbosity
-   Add `Vary` header to cache-bust different encodings

### Fixes

-   Upgrade to latest `tokio` (1.44.2) - Fixes GHSA-rr8g-9fpq-6wmg
-   Increase MSRV to 1.85

## Version `0.1.1`

### Fixes

-   Upgrade to latest h2 (0.4.4) — Fixes GHSA-q6cp-qfwq-4gcv
-   Upgrade to latest `mio` (0.8.11) — Fixes GHSA-r8w9-5wcg-vfj7

## Initial release (`0.1.0`)

## Features

-   Supports gzipped/brotlied files next to regular file
-   All files are kept in memory to reduce disk access
