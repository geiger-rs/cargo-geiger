# cargo-tree

[![CircleCI](https://circleci.com/gh/sfackler/cargo-tree.svg?style=shield)](https://circleci.com/gh/sfackler/cargo-tree) [![Latest Version](https://img.shields.io/crates/v/cargo-tree.svg)](https://crates.io/crates/cargo-tree)

`cargo tree` is a Cargo subcommand that visualizes a crate's dependency graph
in a tree-like format.

Requirements: `cmake`.  OSX users can install via [homebrew](http://brew.sh/): `brew install cmake` .

Install it with Cargo:

```
$ cargo install cargo-tree
```

In its default mode, `cargo tree` will print the "normal" dependencies of the
local crate:

```
$ cargo tree
postgres v0.10.2 (file:///Volumes/git/rust/rust-postgres)
├── bufstream v0.1.1
├── byteorder v0.4.2
├── hex v0.1.0
├── log v0.3.4
│   └── libc v0.2.4
├── net2 v0.2.20
│   ├── cfg-if v0.1.0
│   ├── kernel32-sys v0.2.1
│   │   └── winapi v0.2.5
│   ├── libc v0.2.4 (*)
│   ├── winapi v0.2.5 (*)
│   └── ws2_32-sys v0.2.1
│       └── winapi v0.2.5 (*)
└── phf v0.7.9
    └── phf_shared v0.7.9
```

Crates will only have their dependencies displayed the first time they are
shown - further copies will have a `(*)` appended to indicate that their output
has been truncated.

Like other `cargo` subcommands, features can be enabled via the `--features`
flag:
```
$ cargo tree --features serde_json
postgres v0.10.2 (file:///Volumes/git/rust/rust-postgres)
├── bufstream v0.1.1
├── byteorder v0.4.2
├── hex v0.1.0
├── log v0.3.4
│   └── libc v0.2.4
├── net2 v0.2.20
│   ├── cfg-if v0.1.0
│   ├── kernel32-sys v0.2.1
│   │   └── winapi v0.2.5
│   ├── libc v0.2.4 (*)
│   ├── winapi v0.2.5 (*)
│   └── ws2_32-sys v0.2.1
│       └── winapi v0.2.5 (*)
├── phf v0.7.9
│   └── phf_shared v0.7.9
└── serde_json v0.6.0
    ├── num v0.1.29
    │   ├── rand v0.3.12
    │   │   ├── advapi32-sys v0.1.2
    │   │   │   └── winapi v0.2.5 (*)
    │   │   ├── libc v0.2.4 (*)
    │   │   └── winapi v0.2.5 (*)
    │   └── rustc-serialize v0.3.16
    └── serde v0.6.7
        └── num v0.1.29 (*)
```

`cargo tree` can also operate in an "inverse" mode where the dependency tree is
walked backwards. This is most often useful when trying to determine where
a certain crate is coming from. The `--package` or `-p` flag selects the crate
to use as the root of the tree and the `--invert` or `-i` flag inverts the
dependency graph traversal:

```
$ cargo tree --features serde_json -p libc -i
libc v0.2.4
├── log v0.3.4
│   └── postgres v0.10.2 (file:///Volumes/git/rust/rust-postgres)
├── net2 v0.2.20
│   └── postgres v0.10.2 (file:///Volumes/git/rust/rust-postgres) (*)
└── rand v0.3.12
    └── num v0.1.29
        ├── serde v0.6.7
        │   └── serde_json v0.6.0
        │       └── postgres v0.10.2 (file:///Volumes/git/rust/rust-postgres) (*)
        └── serde_json v0.6.0 (*)
```

More options are available - see the output of `cargo tree --help` for details.
