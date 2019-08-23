geiger ☢️ 
=========

[![Safety Dance](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)

This crate provides some library parts used by [cargo-geiger] that are decoupled
from [cargo].

For more details please see the `README.md` in [cargo-geiger].

Changelog
---------

### 0.3.3
 - Updated dependencies.

### 0.3.2
 - Updated dependencies.

### 0.3.1
 - Bugfix for attributes [#57].

### 0.3.0
 - Added a public function to scan source code strings. [#55]

### 0.2.0
 - Scan for `#![forbid(unsafe_code)]`. [#52]

### 0.1.0
 - Parts of `cargo-geiger` has been moved to this crate.

[#52]: https://github.com/anderejd/cargo-geiger/pull/52
[#55]: https://github.com/anderejd/cargo-geiger/pull/55
[#57]: https://github.com/anderejd/cargo-geiger/pull/57
[cargo-geiger]: https://crates.io/crates/cargo-geiger
[cargo]: https://crates.io/crates/cargo

