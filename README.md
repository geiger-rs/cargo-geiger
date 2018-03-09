# cargo-osha

A Cargo plugin to list unsafe code in a Rust project.  Right now
mostly a proof of concept.

Not actually a cargo plugin yet.

# Example

Example from `winit`, a crate that does a large amount of
platform-specific FFI:

```
> cargo run -- winit/src/**.rs
...
Unsafe functions: 20/85
Unsafe expressions: 222/16847
Unsafe traits: 0/14
Unsafe methods: 14/460
Unsafe impls: 21/136
```

Example from `ggez`, a crate that mostly uses dependencies that
provide safe wrappers, and so needs little unsafe code itself:

```
> cargo run -- ggez/src/**.rs
...
Unsafe functions: 0/101
Unsafe expressions: 0/6786
Unsafe traits: 0/7
Unsafe methods: 0/312
Unsafe impls: 1/122
```

# Usage

Right now it only provides a simple command line program that reads in
N Rust source files.  Doesn't walk dependencies or anything, you have
to provide all files on the command line.  It has no command line
flags worth speaking of.

The code itself is nigh trivial, just read the source.  It uses `syn`
to walk a Rust source code file and counts up the occurances of
`unsafe` in various places.  There may be some it misses, just open an
issue or whatever.

Should be split into a library someday maybe.
