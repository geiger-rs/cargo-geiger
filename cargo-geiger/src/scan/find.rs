use crate::format::print_config::PrintConfig;
use crate::mapping::{CargoMetadataParameters, GetPackageRoot};
use crate::scan::rs_file::{
    into_is_entry_point_and_path_buf, into_rs_code_file, into_target_kind,
    is_file_with_ext, RsFile, RsFileMetricsWrapper,
};
use crate::scan::PackageMetrics;

use super::{GeigerContext, ScanMode};

use cargo::{CargoResult, CliError, GlobalContext};
use geiger::find::find_unsafe_in_file;
use geiger::{IncludeTests, RsFileMetrics, ScanFileError};
use krates::cm::PackageId;
use rayon::{in_place_scope, prelude::*};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

pub fn find_unsafe(
    cargo_metadata_parameters: &CargoMetadataParameters,
    gctx: &GlobalContext,
    mode: ScanMode,
    print_config: &PrintConfig,
) -> Result<GeigerContext, CliError> {
    let mut progress = cargo::util::Progress::new("Scanning", gctx);
    let geiger_context = find_unsafe_in_packages_with_progress(
        print_config.allow_partial_results,
        cargo_metadata_parameters,
        print_config.include_tests,
        mode,
        |progress_count, count| {
            progress.tick(progress_count, count, "find_unsafe_tick")
        },
    );
    progress.clear();
    gctx.shell().status("Scanning", "done")?;
    Ok(geiger_context)
}

fn find_unsafe_in_packages_with_progress<F>(
    allow_partial_results: bool,
    cargo_metadata_parameters: &CargoMetadataParameters,
    include_tests: IncludeTests,
    mode: ScanMode,
    mut progress_fn: F,
) -> GeigerContext
where
    F: FnMut(usize, usize) -> CargoResult<()>,
{
    let mut res: Option<GeigerContext> = None;
    let (progress_sender, progress_receiver) = sync_channel(0);
    let on_processed = move |count_processed, count| {
        progress_sender.send((count_processed, count)).unwrap();
    };
    in_place_scope(|s| {
        s.spawn(|_| {
            res = Some(find_unsafe_in_packages(
                allow_partial_results,
                cargo_metadata_parameters,
                include_tests,
                mode,
                Some(on_processed),
            ))
        });

        while let Ok((progress_counter, count)) = progress_receiver.recv() {
            let _ = progress_fn(progress_counter, count);
        }
    });
    res.unwrap()
}

fn find_unsafe_in_packages<F>(
    allow_partial_results: bool,
    cargo_metadata_parameters: &CargoMetadataParameters,
    include_tests: IncludeTests,
    mode: ScanMode,
    on_processed: Option<F>,
) -> GeigerContext
where
    F: Fn(usize, usize) + Send + Sync,
{
    let package_id_to_metrics = Arc::new(Mutex::new(HashMap::new()));
    let ignored = Arc::new(Mutex::new(HashSet::new()));
    let packages = cargo_metadata_parameters.metadata.packages.to_vec();
    let package_code_files: Vec<_> =
        find_rs_files_in_packages(&packages).collect();
    let package_code_file_count = package_code_files.len();
    let processed_count = AtomicUsize::new(0);
    package_code_files.into_par_iter().for_each_with(
        (package_id_to_metrics.clone(), ignored.clone()),
        |(package_id_to_metrics, ignored), (package_id, rs_code_file)| {
            if let RsFile::CustomBuildRoot(path_buf) = rs_code_file {
                let mut ignored = ignored.lock().unwrap();
                ignored.insert(path_buf);
                return;
            }
            let (is_entry_point, path_buf) =
                into_is_entry_point_and_path_buf(rs_code_file);
            if let (false, ScanMode::EntryPointsOnly) = (is_entry_point, &mode)
            {
                return;
            }
            match find_unsafe_in_file(&path_buf, include_tests) {
                Err(error) => {
                    handle_unsafe_in_file_error(
                        allow_partial_results,
                        error,
                        &path_buf,
                    );
                }
                Ok(rs_file_metrics) => {
                    let package_id_to_metrics =
                        &mut package_id_to_metrics.lock().unwrap();
                    update_package_id_to_metrics_with_rs_file_metrics(
                        is_entry_point,
                        package_id,
                        package_id_to_metrics,
                        path_buf,
                        rs_file_metrics,
                    );
                }
            }

            if let Some(on_processed) = &on_processed {
                on_processed(
                    processed_count.fetch_add(1, Ordering::Relaxed),
                    package_code_file_count,
                );
            }
        },
    );

    let cargo_core_package_metrics = package_id_to_metrics
        .lock()
        .unwrap()
        .iter()
        .map(|(cargo_metadata_package_id, package_metrics)| {
            (cargo_metadata_package_id.clone(), package_metrics.clone())
        })
        .collect::<HashMap<PackageId, PackageMetrics>>();

    GeigerContext {
        package_id_to_metrics: cargo_core_package_metrics,
        ignored_paths: Arc::try_unwrap(ignored).unwrap().into_inner().unwrap(),
    }
}

fn find_rs_files_in_dir(dir: &Path) -> impl Iterator<Item = PathBuf> {
    let walker = WalkDir::new(dir).into_iter();
    walker.filter_map(|entry| {
        let entry = entry.expect("walkdir error."); // TODO: Return result.
        if !is_file_with_ext(&entry, "rs") {
            return None;
        }
        Some(
            entry
                .path()
                .canonicalize()
                .expect("Error converting to canonical path"),
        ) // TODO: Return result.
    })
}

fn find_rs_files_in_package(package: &krates::cm::Package) -> Vec<RsFile> {
    // Find all build target entry point source files.
    let mut canon_targets = HashMap::new();
    for target in &package.targets {
        let path = target.src_path.as_path();
        /*let path = match path {
            None => continue,
            Some(p) => p,
        };*/
        if !path.exists() {
            // A package published to crates.io is not required to include
            // everything. We have to skip this build target.
            continue;
        }
        let canon = path
            .canonicalize() // will Err on non-existing paths.
            .expect("canonicalize for build target path failed."); // FIXME
        let targets = canon_targets.entry(canon).or_insert_with(Vec::new);
        targets.push(target);
    }
    let mut rs_files = Vec::new();

    if let Some(root_path) = package.clone().get_root() {
        for path_bufs in find_rs_files_in_dir(root_path.as_path()) {
            if !canon_targets.contains_key(&path_bufs) {
                rs_files.push(RsFile::Other(path_bufs));
            }
        }
    }

    for (path_buf, targets) in canon_targets.into_iter() {
        for target in targets {
            let target_kind = into_target_kind(&target.kind);
            rs_files.push(into_rs_code_file(&target_kind, path_buf.clone()));
        }
    }

    rs_files
}

fn find_rs_files_in_packages(
    packages: &[krates::cm::Package],
) -> impl Iterator<Item = (PackageId, RsFile)> + '_ {
    packages.iter().flat_map(|package| {
        find_rs_files_in_package(package)
            .into_iter()
            .map(move |p| (package.id.clone(), p))
    })
}

fn handle_unsafe_in_file_error(
    allow_partial_results: bool,
    error: ScanFileError,
    path_buf: &Path,
) {
    if allow_partial_results {
        eprintln!("Failed to parse file: {}, {:?} ", path_buf.display(), error);
    } else {
        panic!("Failed to parse file: {}, {:?} ", path_buf.display(), error);
    }
}

fn update_package_id_to_metrics_with_rs_file_metrics(
    is_entry_point: bool,
    package_id: PackageId,
    package_id_to_metrics: &mut HashMap<PackageId, PackageMetrics>,
    path_buf: PathBuf,
    rs_file_metrics: RsFileMetrics,
) {
    let package_metrics = package_id_to_metrics
        .entry(package_id)
        .or_insert_with(PackageMetrics::default);
    let wrapper = package_metrics
        .rs_path_to_metrics
        .entry(path_buf)
        .or_insert_with(RsFileMetricsWrapper::default);
    wrapper.metrics = rs_file_metrics;
    wrapper.is_crate_entry_point = is_entry_point;
}

#[cfg(test)]
mod find_tests {
    use super::*;

    use krates::cm::{CargoOpt, MetadataCommand};
    use rstest::*;
    use std::fs::File;
    use std::io;
    use std::io::ErrorKind;
    use tempfile::tempdir;

    #[rstest]
    fn find_rs_files_in_dir_test() {
        let temp_dir = tempdir().unwrap();

        let mut rs_file_names =
            vec!["rs_file_1.rs", "rs_file_2.rs", "rs_file_3.rs"];

        for file_name in &rs_file_names {
            let file_path = temp_dir.path().join(file_name);
            File::create(file_path).unwrap();
        }

        let non_rs_file_names =
            vec!["non_rs_file_1.txt", "non_rs_file_2.ext", "non_rs_file"];

        for file_name in &non_rs_file_names {
            let file_path = temp_dir.path().join(file_name);
            File::create(file_path).unwrap();
        }

        let actual_rs_files = find_rs_files_in_dir(temp_dir.path());

        let mut actual_rs_file_names = actual_rs_files
            .into_iter()
            .map(|f| {
                String::from(f.as_path().file_name().unwrap().to_str().unwrap())
            })
            .collect::<Vec<String>>();

        rs_file_names.sort_unstable();
        actual_rs_file_names.sort();

        assert_eq!(actual_rs_file_names, rs_file_names);
    }

    #[rstest]
    fn find_rs_file_in_package() {
        let package = get_current_workspace_package();
        let rs_files_in_package = find_rs_files_in_package(&package);

        let path_bufs_in_package = rs_files_in_package
            .iter()
            .map(|f| match f {
                RsFile::BinRoot(path_buf) => path_buf,
                RsFile::CustomBuildRoot(path_buf) => path_buf,
                RsFile::LibRoot(path_buf) => path_buf,
                RsFile::Other(path_buf) => path_buf,
            })
            .collect::<Vec<&PathBuf>>();

        for path_buf in &path_bufs_in_package {
            assert_eq!(path_buf.extension().unwrap().to_str().unwrap(), "rs");
        }
    }

    #[rstest]
    fn handle_unsafe_in_file_error_doesnt_panic_when_allow_partial_results_is_true(
    ) {
        let path_buf = PathBuf::from("test_path");
        handle_unsafe_in_file_error(
            true,
            ScanFileError::Io(
                io::Error::new(ErrorKind::Other, "test"),
                path_buf.clone(),
            ),
            &path_buf,
        );
    }

    #[rstest]
    #[should_panic]
    fn handle_unsafe_in_file_error_panics_when_allow_partial_results_is_false()
    {
        let path_buf = PathBuf::from("test_path");
        handle_unsafe_in_file_error(
            false,
            ScanFileError::Io(
                io::Error::new(ErrorKind::Other, "test"),
                path_buf.clone(),
            ),
            &path_buf,
        );
    }

    #[rstest(
        input_is_entry_point,
        expected_is_crate_entry_point,
        package,
        case(true, true, get_current_workspace_package()),
        case(false, false, get_current_workspace_package())
    )]
    fn update_package_id_to_metrics_with_rs_file_metrics_test(
        input_is_entry_point: bool,
        expected_is_crate_entry_point: bool,
        package: krates::cm::Package,
    ) {
        let mut package_id_to_metrics =
            HashMap::<PackageId, PackageMetrics>::new();

        let mut rs_files_in_package = find_rs_files_in_package(&package);
        let rs_file = rs_files_in_package.pop().unwrap();
        let (_, path_buf) = into_is_entry_point_and_path_buf(rs_file);

        let rs_file_metrics =
            find_unsafe_in_file(path_buf.as_path(), IncludeTests::Yes).unwrap();

        update_package_id_to_metrics_with_rs_file_metrics(
            input_is_entry_point,
            package.id.clone(),
            &mut package_id_to_metrics,
            package.manifest_path.clone().into(),
            rs_file_metrics.clone(),
        );

        assert!(package_id_to_metrics.contains_key(&package.id));
        let package_metrics = package_id_to_metrics.get(&package.id).unwrap();

        let manifest_path: &Path = package.manifest_path.as_path().as_ref();
        let wrapper = package_metrics
            .rs_path_to_metrics
            .get(manifest_path)
            .unwrap();

        assert_eq!(wrapper.metrics, rs_file_metrics);
        assert_eq!(wrapper.is_crate_entry_point, expected_is_crate_entry_point);
    }

    #[fixture]
    fn get_current_workspace_package() -> krates::cm::Package {
        let metadata = MetadataCommand::new()
            .manifest_path("./Cargo.toml")
            .features(CargoOpt::AllFeatures)
            .exec()
            .unwrap();

        metadata.root_package().unwrap().clone()
    }
}
