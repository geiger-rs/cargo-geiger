# Changelog
---------

## 0.12.0
 - Upgraded from Cargo 0.75 to 0.86
 - Fix compilation with new rust versions - thanks @mleonhard [#518] and @justahero [#529] & others

## 0.11.7
 - Upgraded from Cargo 0.69 to 0.75
 - Instruct to use --locked for installation

## 0.11.6
 - Upgraded from Cargo 0.67 to 0.69
 - Fix panic with git dep without rev - thanks @ginger51011 [#462]

## 0.11.5
 - Upgraded from Cargo 0.63 to 0.67
 - Upgraded from rayon 1.5 to 1.6
 - Bump lockfile

## 0.11.4
 - Bump insta from 1.16 to 1.17 [#353], [#354]
 - Bump regex from 0.5 to 0.6 [#348]
 - Code clean-ups - thanks @jmcconnell26 [#333]
 - Bump pico-args from 0.4 to 0.5 [#328]
 - Upgraded from Cargo 0.62.0 to 0.63.0 [#345]
 - Upgraded from Cargo 0.60.0 to 0.62.0 - thanks @jmcconnell26 [#317]
 - Bump lockfile [#349]

## 0.11.3
 - Add threaded scanning [#268]
 - Upgraded dependencies including from Cargo 0.58.0 to 0.60.0 [#251], [#275]

## 0.11.2
 - Upgraded dependencies including from Cargo 0.52.0 to 0.58.0 [#230], [#225]

## 0.11.1
 - Removed a failing test case that depended on the crate version.

## 0.11.0
 - Explore the dependency graph using cargo_metadata [#16]
   - [#120], [#122], [#126], [#129], [#133], [#135], [#136], [#138], [#139], [#140], [#141], [#142], [#143], [#146], [#147], [#154]
 - Add build without lock file to CI and upgrade the cargo dependency to 0.50. [#183]
 - Feature: safety report in readme. [#151]
 - Make `--quiet` take no value. [#114]
 - Ability to generate a JSON report. [#115]
 - Fix tree vine on dependency group line. [#118]
 - `cargo-geiger-serde`, a crate with types for report serialization using serde. [#121]
 - Replace links that points to the old repository. [#124]
 - Move report types to lib (`cargo-geiger-serde`). [#125]
 - Add cargo tarpaulin step to CI [#127]
 - Add code coverage badge to readme [#128]
 - Add crates.io badges, current version, total downloads. [#130]
 - Use GitHub Actions / actions-rs to ensure code is well-formatted. [#131]
 - Add CONTRIBUTING.md file. [#132]
 - Fixed small errors in Changelog. [#134]
 - Add Dockerfile and use cargo chef to reduce docker build times locally. [#148]
 - Create lib.rs to allow documentation tests to be written. [#153]
 - `--update-readme` Writes output to README.md. Looks for a Safety Report
   section, replaces if found, adds if not. Throws an error if no README.md
   exists. [#156]
 - Refactor integration tests. [#157]
 - Refactoring geiger lib and adding further testing. [#158]
 - Accept Readme Path and Section Name as parameters. [#159]
 - Update version of syn package used in geiger. [#161]
 - Fix a bug where a report wasn't written if any warning. [#162]
 - Add GitHub markdown formatting. [#164]
 - Cleanup a trait only used in a unit test module. [#165]
 - Run `cargo audit` as part of CI builds. [#166]
 - Add new Ratio output type `--output-format=Ratio`. [#167]
 - Clean only packages. [#171]
 - Mark no_mangle functions as unsafe. [#173]
 - Improved `README.md` [#176]
 - Update graph module to use latest version of cargo_metadata. [#178]
 - Explicitly enable serde for semver. [#180]
 - Use DependencyKind from cargo_metadata. [#182]
 - Add canary build without lockfile. [#183]
 - Add cargo audit github action to run against head every day. [#184]
 - Clean up error handling, remove unwrap() calls, logging. [#188]
 - Update lint enforcement level based on issue. [#189]
 - Implement Display for FoundWarningsError instead of relying on Debug. [#191]
 - Add further testing. [#192]
 - Fix Args::parse_args for -p option. [#196]
 - Refactor mapping module to use traits. [#197]
 - Fix into target kind function logic. [#198]
 - Bump insta version. [#199]
 - Upgrade dependencies; use cargo 1.52.0 for the new resolver. [#201]

## 0.10.2
 - __Bugfix__: Avoid panic and log warnings on parse failure. [#105]
 - Upgraded all dependencies.

## 0.10.1
 - Expose the `cargo` crate feature: `vendored-openssl`. [#99]
 - Upgraded all dependencies.

## 0.10.0
 - Upgraded all dependencies. [#98]

## 0.9.1
 - __Bugfix__: Avoid counting the same crate multiple times. [#79]
 - Upgraded cargo to 0.41. [#85]
 - Upgraded all dependencies.

## 0.9.0
 - __Breaking change__: Replaced structopt & clap with [pico-args], to reduce 
   compile times [#77]. As a result the `-Z` flag now requires quotes around
   its list of sub arguments, other than that there should be no changes to 
   the CLI.

## 0.8.0
 - __Bugfix:__ Count all expressions in unsafe functions and nested unsafe
   scopes, in [geiger 0.4.1](geiger), [#72] & [#71].
 - __Bugfix:__ Properly account for possibly patched dependencies [#70].
 - Summary for each metrics column, [#76].
 - Now requires all entry points for a crate to declare
   `#[forbid(unsafe_code)]` for it to count as crate-wide.
 - New optional scan mode `--forbid-only`. This mode doesn't require any calls
   to `rustc` and only requires parsing the entry point `.rs` files, making it
   much faster than the normal mode.
 - Updated dependencies.

## 0.7.3
 - __Bugfix:__ Fix dependency collection for mixed workspaces [#66].
 - Updated dependencies.

## 0.7.2
 - Updated dependencies to fix [#59].

## 0.7.1
 - __Bugfix:__ related to attributes, in [geiger] [#57].
 - Updated all dependencies.

## 0.7.0
 - Updated all dependencies, [geiger] to 0.3.0.

## 0.6.1
 - A tiny readme fix.

## 0.6.0
 - There are now three crate scanning result variants [#52]:
   - 🔒 No unsafe usage found and all build target entry point `.rs` source
     files, used by the build, declare `#![forbid(unsafe_code)]`. Crates like
     this will be printed in green.
   - ❓ No unsafe usage found, but at least one build target entry point `.rs`
     file, used by the build, does not declare `#[forbid(unsafe_code)]`.  Crates
     like this will be printed in the default terminal foreground color.
   - ☢️  Unsafe usage found. Crates like this will be printed in red, same as in
     the previous version.

## 0.5.0
 - Moved reusable parts, decoupled from `cargo`, to the new crate
   [geiger]. Main github issue: [#30].
 - Some general refactoring and cleanup.
 - Merge pull request [#46] from alexmaco/dependency_kind_control. add options
   to filter dependencies by kind; defaults to Kind::Normal.
 - Merge pull request [#40] from jiminhsieh/rust-2018. Use Rust 2018 edition.

## 0.4.2
 - __Bugfix:__ Merge pull request [#33] from ajpaverd/windows_filepaths.
   Canonicalize file paths from walker.

 - Merge pull request [#38] from anderejd/updated-deps. Updated deps and fixed
   build errors.

## 0.4.1
 - Merge pull request [#28] from alexmaco/deps_upgrade. fix build on rust 1.30:
   upgrade petgraph to 0.4.13

 - __Bugfix:__ Merge pull request [#29] from alexmaco/invalid_utf8_source. fix 
   handling source files with invalid utf8: lossy conversion to string

## 0.4.0
 - Filters out tests by default. Tests can still be included by using
   `--include-tests`. The test code is filtered out by looking for the attribute
   `#[test]` on functions and `#[cfg(test)]` on modules.

## 0.3.1
 - __Bugfix:__ Some bugfixes related to cargo workspace path handling.
 - Slightly better error messages in some cases.

## 0.3.0
 - Intercepts `rustc` calls and reads the `.d` files generated by `rustc` to
   identify which `.rs` files are used by the build. This allows a crate that
   contains `.rs` files with unsafe code usage to pass as "green" if the unsafe
   code isn't used by the build.
 - Each metric is now printed as `x/y`, where `x` is the unsafe code used by the
   build and `y` is the total unsafe usage found in the crate.
 - Removed the `--compact` output format to avoid some code complexity. A new
   and better compact mode can be added later if requested.

## 0.2.0
 - Table based output format [#9].

## 0.1.x
 - Initial experimental versions.
 - Mostly README.md updates.

[#9]: https://github.com/rust-secure-code/cargo-geiger/pull/9
[#16]: https://github.com/rust-secure-code/cargo-geiger/issues/16
[#28]: https://github.com/rust-secure-code/cargo-geiger/issues/28
[#29]: https://github.com/rust-secure-code/cargo-geiger/issues/29
[#30]: https://github.com/rust-secure-code/cargo-geiger/issues/30
[#33]: https://github.com/rust-secure-code/cargo-geiger/issues/33
[#38]: https://github.com/rust-secure-code/cargo-geiger/issues/38
[#40]: https://github.com/rust-secure-code/cargo-geiger/issues/40
[#46]: https://github.com/rust-secure-code/cargo-geiger/issues/46
[#52]: https://github.com/rust-secure-code/cargo-geiger/issues/52
[#57]: https://github.com/rust-secure-code/cargo-geiger/issues/57
[#59]: https://github.com/rust-secure-code/cargo-geiger/issues/59
[#66]: https://github.com/rust-secure-code/cargo-geiger/issues/66
[#70]: https://github.com/rust-secure-code/cargo-geiger/pull/70
[#71]: https://github.com/rust-secure-code/cargo-geiger/issues/71
[#72]: https://github.com/rust-secure-code/cargo-geiger/pull/72
[#76]: https://github.com/rust-secure-code/cargo-geiger/pull/76
[#77]: https://github.com/rust-secure-code/cargo-geiger/pull/77
[#79]: https://github.com/rust-secure-code/cargo-geiger/issues/79
[#85]: https://github.com/rust-secure-code/cargo-geiger/pull/85
[#98]: https://github.com/rust-secure-code/cargo-geiger/pull/98
[#99]: https://github.com/rust-secure-code/cargo-geiger/pull/99
[#105]: https://github.com/rust-secure-code/cargo-geiger/issues/105
[#114]: https://github.com/rust-secure-code/cargo-geiger/pull/114
[#115]: https://github.com/rust-secure-code/cargo-geiger/pull/115
[#118]: https://github.com/rust-secure-code/cargo-geiger/pull/118
[#120]: https://github.com/rust-secure-code/cargo-geiger/pull/120
[#121]: https://github.com/rust-secure-code/cargo-geiger/pull/121
[#122]: https://github.com/rust-secure-code/cargo-geiger/pull/122
[#124]: https://github.com/rust-secure-code/cargo-geiger/pull/124
[#125]: https://github.com/rust-secure-code/cargo-geiger/pull/125
[#126]: https://github.com/rust-secure-code/cargo-geiger/pull/126
[#127]: https://github.com/rust-secure-code/cargo-geiger/pull/127
[#128]: https://github.com/rust-secure-code/cargo-geiger/pull/128
[#129]: https://github.com/rust-secure-code/cargo-geiger/pull/129
[#130]: https://github.com/rust-secure-code/cargo-geiger/pull/130
[#131]: https://github.com/rust-secure-code/cargo-geiger/pull/131
[#132]: https://github.com/rust-secure-code/cargo-geiger/pull/132
[#133]: https://github.com/rust-secure-code/cargo-geiger/pull/133
[#134]: https://github.com/rust-secure-code/cargo-geiger/pull/134
[#135]: https://github.com/rust-secure-code/cargo-geiger/pull/135
[#136]: https://github.com/rust-secure-code/cargo-geiger/pull/136
[#138]: https://github.com/rust-secure-code/cargo-geiger/pull/138
[#139]: https://github.com/rust-secure-code/cargo-geiger/pull/139
[#140]: https://github.com/rust-secure-code/cargo-geiger/pull/140
[#141]: https://github.com/rust-secure-code/cargo-geiger/pull/141
[#142]: https://github.com/rust-secure-code/cargo-geiger/pull/142
[#143]: https://github.com/rust-secure-code/cargo-geiger/pull/143
[#146]: https://github.com/rust-secure-code/cargo-geiger/pull/146
[#147]: https://github.com/rust-secure-code/cargo-geiger/pull/147
[#148]: https://github.com/rust-secure-code/cargo-geiger/pull/148
[#151]: https://github.com/rust-secure-code/cargo-geiger/issues/151
[#153]: https://github.com/rust-secure-code/cargo-geiger/pull/153
[#154]: https://github.com/rust-secure-code/cargo-geiger/pull/154
[#156]: https://github.com/rust-secure-code/cargo-geiger/pull/156
[#157]: https://github.com/rust-secure-code/cargo-geiger/pull/157
[#158]: https://github.com/rust-secure-code/cargo-geiger/pull/158
[#159]: https://github.com/rust-secure-code/cargo-geiger/pull/159
[#161]: https://github.com/rust-secure-code/cargo-geiger/pull/161
[#162]: https://github.com/rust-secure-code/cargo-geiger/pull/162
[#164]: https://github.com/rust-secure-code/cargo-geiger/pull/164
[#165]: https://github.com/rust-secure-code/cargo-geiger/pull/165
[#166]: https://github.com/rust-secure-code/cargo-geiger/issues/166
[#167]: https://github.com/rust-secure-code/cargo-geiger/pull/167
[#171]: https://github.com/rust-secure-code/cargo-geiger/pull/171
[#173]: https://github.com/rust-secure-code/cargo-geiger/pull/173
[#176]: https://github.com/rust-secure-code/cargo-geiger/pull/176
[#178]: https://github.com/rust-secure-code/cargo-geiger/pull/178
[#180]: https://github.com/rust-secure-code/cargo-geiger/pull/180
[#182]: https://github.com/rust-secure-code/cargo-geiger/pull/182
[#183]: https://github.com/rust-secure-code/cargo-geiger/pull/183
[#184]: https://github.com/rust-secure-code/cargo-geiger/pull/184
[#188]: https://github.com/rust-secure-code/cargo-geiger/pull/188
[#189]: https://github.com/rust-secure-code/cargo-geiger/pull/189
[#191]: https://github.com/rust-secure-code/cargo-geiger/pull/191
[#192]: https://github.com/rust-secure-code/cargo-geiger/pull/192
[#196]: https://github.com/rust-secure-code/cargo-geiger/pull/196
[#197]: https://github.com/rust-secure-code/cargo-geiger/pull/197
[#198]: https://github.com/rust-secure-code/cargo-geiger/pull/198
[#199]: https://github.com/rust-secure-code/cargo-geiger/pull/199
[#201]: https://github.com/rust-secure-code/cargo-geiger/pull/201
[geiger]: https://crates.io/crates/geiger
[pico-args]: https://crates.io/crates/pico-args

