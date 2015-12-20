# cargo-tree

[![Build Status](https://travis-ci.org/sfackler/cargo-tree.svg?branch=master)](https://travis-ci.org/sfackler/cargo-tree) [![Latest Version](https://img.shields.io/crates/v/cargo-tree.svg)](https://crates.io/crates/cargo-tree)

`cargo tree` is a Cargo subcommand that visualizes a crate's dependency graph
in a tree-like format.

Install it with Cargo:

```
$ cargo install cargo-tree
```

In its default mode, `cargo tree` will print the dependency graph from the
local crate outwards:

```
$ cargo tree
chrono v0.2.17 (file:///Volumes/git/rust/rust-chrono)
├── num v0.1.29
│  ├── rand v0.3.12
│  │  ├── advapi32-sys v0.1.2
│  │  │  ├── winapi v0.2.5
│  │  │  └── winapi-build v0.1.1
│  │  ├── libc v0.2.4
│  │  └── winapi v0.2.5 (*)
│  └── rustc-serialize v0.3.16
├── rustc-serialize v0.3.16 (*)
├── serde v0.6.6
│  └── num v0.1.29 (*)
├── serde_json v0.6.0
│  ├── num v0.1.29 (*)
│  └── serde v0.6.6 (*)
└── time v0.1.34
   ├── kernel32-sys v0.2.1
   │  ├── winapi v0.2.5 (*)
   │  └── winapi-build v0.1.1 (*)
   ├── libc v0.2.4 (*)
   └── winapi v0.2.5 (*)
```

Crates will only have their dependencies displayed the first time they are
shown - further copies will have a `(*)` appended to indicate that their output
has been truncated.

`cargo tree` can also operate in an "inverse" mode where the dependency tree is
walked backwards. This is most often useful when trying to determine where
a certain crate is coming from:

```
cargo tree -p libc -i
libc v0.2.4
├── rand v0.3.12
│  └── num v0.1.29
│     ├── chrono v0.2.17 (file:///Volumes/git/rust/rust-chrono)
│     ├── serde v0.6.6
│     │  ├── chrono v0.2.17 (file:///Volumes/git/rust/rust-chrono) (*)
│     │  └── serde_json v0.6.0
│     │     └── chrono v0.2.17 (file:///Volumes/git/rust/rust-chrono) (*)
│     └── serde_json v0.6.0 (*)
└── time v0.1.34
   └── chrono v0.2.17 (file:///Volumes/git/rust/rust-chrono) (*)
```
