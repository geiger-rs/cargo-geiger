#![forbid(unsafe_code)]
#![forbid(warnings)]

mod context;
mod run;

use self::run::run_geiger_with;

use insta::assert_snapshot;
use rstest::rstest;
use std::fs::read_to_string;

#[rstest(
    input_crate_name,
    input_arg_vec,
    input_readme_filename,
    case(
        "test1_package_with_no_deps",
        vec!["--update-readme"],
        "README.md"
    ),
    case(
        "test2_package_with_shallow_deps",
        vec!["--update-readme"],
        "README.md"
    ),
    case(
        "test3_package_with_nested_deps",
        vec!["--update-readme"],
        "README.md"
    ),
    case(
        "test4_workspace_with_top_level_package",
        vec!["--update-readme", "--section-name", "Test Section Name"],
        "README.md"
    ),
    case(
        "test6_cargo_lock_out_of_date",
        vec![
            "--update-readme",
            "--readme-path",
            "README_DIFFERENT_NAME.md"
        ],
        "README_DIFFERENT_NAME.md"
    ),
    case(
        "test7_package_with_patched_dep",
        vec![
            "--update-readme",
            "--section-name",
            "Test Section Name",
            "--readme-path",
            "README_DIFFERENT_NAME.md"
        ],
        "README_DIFFERENT_NAME.md"
    )
)]
fn test_package_update_readme(
    input_crate_name: &str,
    input_arg_vec: Vec<&str>,
    input_readme_filename: &str,
) {
    let (_, context) = run_geiger_with(input_crate_name, input_arg_vec);

    let readme_snapshot_filename = format!("{}.readme", input_crate_name);

    let crate_location = context.crate_dir(input_crate_name);
    let readme_location = crate_location.join(input_readme_filename);

    let readme_content = read_to_string(readme_location).unwrap();
    assert_snapshot!(readme_snapshot_filename, readme_content);
}
