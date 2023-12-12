cargo-geiger ☢️ 
===============

[![CI](https://github.com/geiger-rs/cargo-geiger/actions/workflows/ci.yml/badge.svg)](https://github.com/geiger-rs/cargo-geiger/actions/workflows/ci.yml)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![crates.io](https://img.shields.io/crates/v/cargo-geiger.svg)](https://crates.io/crates/cargo-geiger)
[![Crates.io](https://img.shields.io/crates/d/cargo-geiger?label=cargo%20installs)](https://crates.io/crates/cargo-geiger)

A tool that lists statistics related to the usage of unsafe Rust code in a Rust
crate and all its dependencies.

This cargo plugin was originally based on the code from two other projects:
* <https://github.com/icefoxen/cargo-osha> and
* <https://github.com/sfackler/cargo-tree>

Installation
------------

Try to find and use a system-wide installed OpenSSL library:

```bash
cargo install --locked cargo-geiger
```

Or, build and statically link OpenSSL as part of the cargo-geiger executable:

```bash
cargo install --locked cargo-geiger --features vendored-openssl
```

Alternatively pre-built binary releases are available.

Usage
-----

1. Navigate to the same directory as the `Cargo.toml` you want to analyze.
2. `cargo geiger`

Intended Use
------------

This tool is not meant to advise directly whether the code ultimately is truly insecure or not.

The purpose of cargo-geiger is to provide statistical input to auditing e.g. with:

- [cargo-crev](https://crates.io/crates/cargo-crev)
- [safety-dance](https://github.com/rust-secure-code/safety-dance)

The use of unsafe is nuanced and necessary in some cases and any motivation to use it is outside the scope of cargo-geiger.

It is important that any reporting is handled with care:

- [Reddit: The Stigma around Unsafe](https://www.reddit.com/r/rust/comments/y1u068/the_stigma_around_unsafe/)
- [YouTube: Rust NYC: Jon Gjengset - Demystifying unsafe code](https://youtu.be/QAz-maaH0KM)
- [Rust-lang: WG Unsafe Code Guidelines](https://github.com/rust-lang/unsafe-code-guidelines)

Output example
--------------

![Example output](https://user-images.githubusercontent.com/3704611/53132247-845f7080-356f-11e9-9c76-a9498d4a744b.png)

Known issues
------------

 - See the [issue tracker](https://github.com/rust-secure-code/cargo-geiger/issues).

Libraries
---------

Cargo Geiger exposes three libraries:

 - `cargo-geiger` - Unversioned and highly unstable library exposing the internals of the `cargo-geiger` binary. As such, any function contained within this library may be subject to change.
 - `cargo-geiger-serde` - A library containing the serializable report types
 - `geiger` - A library containing a few decoupled [cargo] components used by [cargo-geiger]

Changelog
---------

See the [changelog].

[cargo]: https://crates.io/crates/cargo
[cargo-geiger]: https://crates.io/crates/cargo-geiger
[changelog]: https://github.com/rust-secure-code/cargo-geiger/blob/master/CHANGELOG.md

Why the name?
-------------

<https://en.wikipedia.org/wiki/Geiger_counter>

Unsafe code, like ionizing radiation, is unavoidable in some situations and should be safely contained!

