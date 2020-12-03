use std::env;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use url::Url;

pub struct Context {
    _dir: TempDir,
    pub path: PathBuf,
}

impl Context {
    pub fn new() -> Self {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let src_path = Path::new(&manifest_dir).join("../test_crates");
        let dir = TempDir::new().unwrap();
        let copy_options = fs_extra::dir::CopyOptions {
            content_only: true,
            ..Default::default()
        };
        fs_extra::dir::copy(&src_path, dir.path(), &copy_options)
            .expect("Failed to copy tests");
        let path = dir
            .path()
            .canonicalize()
            .expect("Failed to canonicalize temporary path");
        // Canonicalizing on Windows returns a UNC path (starting with `\\?\`).
        // `cargo build` (as of 1.47.0) fails to use an overriding path dependency if the manifest
        // given to `cargo build` is a UNC path. Roudtripping to URL gets rid of the UNC prefix.
        let path = if cfg!(windows) {
            Url::from_file_path(path)
                .expect("URL from path must succeed")
                .to_file_path()
                .expect("Roundtripping path to URL must succeed")
        } else {
            path
        };
        let _dir = dir;
        Context { _dir, path }
    }

    pub fn crate_dir(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}
