use cargo_geiger_serde::SafetyReport;
use std::fs;

#[test]
fn generated_rust_is_counted_and_resource_inputs_are_not_rust() {
    let dir = tempfile::tempdir().unwrap();
    let foreign = tempfile::tempdir().unwrap();
    fs::create_dir(foreign.path().join("src")).unwrap();
    fs::write(
        foreign.path().join("Cargo.toml"),
        "[package]\nname = \"foreign-input-proof\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    ).unwrap();
    fs::write(
        foreign.path().join("src/lib.rs"),
        "pub unsafe fn foreign() {}",
    )
    .unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("Cargo.toml"), format!(
        "[package]\nname = \"generated-input-proof\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[workspace]\n[target.'cfg(target_os = \"none\")'.dependencies]\nforeign-input-proof = {{ path = {:?} }}\n",
        foreign.path().to_string_lossy().replace('\\', "/"),
    )).unwrap();
    fs::write(dir.path().join("build.rs"), r#"fn main() {
        std::fs::write(std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("generated.rs"), "pub unsafe fn generated() {}\n").unwrap();
    }"#).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        r#"include!(concat!(env!("OUT_DIR"), "/generated.rs"));
        pub const DOC: &str = include_str!("../README.md");
        pub const DATA: &[u8] = include_bytes!("../sample.data");"#,
    )
    .unwrap();
    fs::write(dir.path().join("README.md"), "Documentation, not Rust.")
        .unwrap();
    fs::write(dir.path().join("sample.data"), [0xff, 0x00]).unwrap();
    fs::write(
        dir.path().join("src/uncompiled.rs"),
        "pub unsafe fn uncompiled() {}",
    )
    .unwrap();
    let target_dir = tempfile::tempdir().unwrap();
    let deps_dir = target_dir.path().join("debug/deps");
    fs::create_dir_all(&deps_dir).unwrap();
    fs::write(
        deps_dir.join("unrelated.d"),
        "unrelated: missing-from-another-build.rs\n",
    )
    .unwrap();
    for format in ["Ascii", "Json"] {
        let output = assert_cmd::cargo_bin_cmd!("cargo-geiger")
            .args([
                "geiger",
                "--all-targets",
                "--color=never",
                "--output-format",
                format,
            ])
            .arg("--manifest-path")
            .arg(dir.path().join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", target_dir.path())
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stderr)
            .contains("Failed to parse"));
        assert!(!String::from_utf8_lossy(&output.stderr)
            .contains("Failed to format Package"));
        if format == "Ascii" {
            assert!(String::from_utf8_lossy(&output.stdout)
                .contains("foreign-input-proof 0.1.0"));
        }
        if format == "Json" {
            let report: SafetyReport =
                serde_json::from_slice(&output.stdout).unwrap();
            assert!(
                report.used_but_not_scanned_files.is_empty(),
                "{:?}",
                report
            );
            assert!(report.packages_without_metrics.is_empty(), "{:?}", report);
            let root = report
                .packages
                .values()
                .find(|entry| entry.package.id.name == "generated-input-proof")
                .unwrap();
            assert_eq!(root.unsafety.used.functions.unsafe_, 1);
            assert_eq!(root.unsafety.unused.functions.unsafe_, 1);
            let foreign = report
                .packages
                .values()
                .find(|entry| entry.package.id.name == "foreign-input-proof")
                .unwrap();
            assert_eq!(foreign.unsafety.used.functions.unsafe_, 0);
            assert_eq!(foreign.unsafety.unused.functions.unsafe_, 1);
        }
    }
}
