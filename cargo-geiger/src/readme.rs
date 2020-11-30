use cargo::{CliError, CliResult};
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Name of README FILE
pub const README_FILENAME: &str = "README.md";
/// Safety report section
const CARGO_GEIGER_SAFETY_REPORT_SECTION_HEADER: &str =
    "## Cargo Geiger Safety Report";

/// Taking a `PathBuf` pointing to the README location, and a `&Vec<String>` containing the result
/// of a scan, either create a section containing the scan result if one does not exist, or replace
/// the section if it already exists
pub fn create_or_replace_section_in_readme(
    readme_file_path: PathBuf,
    scan_output_lines: &Vec<String>,
) -> CliResult {
    if !readme_file_path.exists() {
        eprintln!(
            "File {} does not exist. To construct a Cargo Geiger Safety Report section, please first create a README.",
            readme_file_path.to_str().unwrap()
        );
        return CliResult::Err(CliError::code(1));
    }

    let mut readme_content =
        BufReader::new(File::open(readme_file_path.clone()).unwrap())
            .lines()
            .map(|l| l.unwrap())
            .collect::<Vec<String>>();

    update_readme_content(&mut readme_content, scan_output_lines);

    let mut readme_file = File::create(readme_file_path.clone()).unwrap();

    for line in readme_content {
        writeln!(readme_file, "{}", line).unwrap();
    }

    Ok(())
}

/// For a `&Vec<String` find the index of the first and last lines of a Safety Report Section. If
/// the Section is not present, -1 is returned for both values, and if the Section is the last
/// section present, then the last index is -1
fn find_start_and_end_lines_of_safety_report_section(
    readme_content: &Vec<String>,
) -> (i32, i32) {
    let mut start_line_number = -1;
    let mut end_line_number = -1;

    let start_line_pattern =
        Regex::new("#+\\sCargo\\sGeiger\\sSafety\\sReport\\s*").unwrap();

    let end_line_pattern = Regex::new("#+.*").unwrap();

    for (line_number, line) in readme_content.iter().enumerate() {
        if start_line_pattern.is_match(line) {
            start_line_number = line_number as i32;
            continue;
        }

        if start_line_number != -1 && end_line_pattern.is_match(line) {
            end_line_number = line_number as i32;
            break;
        }
    }

    (start_line_number, end_line_number)
}

/// Update the content of a README.md with a Scan Result
fn update_readme_content(
    readme_content: &mut Vec<String>,
    scan_result: &Vec<String>,
) {
    let (start_line_number, end_line_number) =
        find_start_and_end_lines_of_safety_report_section(&readme_content);

    if start_line_number == -1 {
        // When Cargo Geiger Safety Report isn't present in README, add an
        // h2 headed section at the end of the README.md containing the report
        readme_content
            .push(CARGO_GEIGER_SAFETY_REPORT_SECTION_HEADER.to_string());
        for scan_result_line in scan_result {
            readme_content.push(scan_result_line.to_string())
        }
    } else {
        // When Cargo Geiger Safety Report is present in README, remove the
        // section and and replace, preserving header level (h1/h2/h3)
        for _ in start_line_number + 1..end_line_number {
            readme_content.remove((start_line_number + 1) as usize);
        }

        let mut running_scan_line_index = start_line_number + 1;

        for scan_result_line in scan_result {
            readme_content.insert(
                running_scan_line_index as usize,
                scan_result_line.to_string(),
            );
            running_scan_line_index += 1;
        }
    }
}

#[cfg(test)]
mod readme_tests {
    use super::*;

    use rstest::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[rstest]
    fn create_or_replace_section_test_readme_doesnt_exist() {
        let temp_dir = tempdir().unwrap();
        let readme_file_path = temp_dir.path().join("README.md");
        let scan_result = vec![];

        let result =
            create_or_replace_section_in_readme(readme_file_path, &scan_result);

        assert!(result.is_err());
    }

    #[rstest]
    fn create_or_replace_section_test_reademe_doesnt_contain_section() {
        let temp_dir = tempdir().unwrap();
        let readme_file_path = temp_dir.path().join("README.md");
        let mut readme_file = File::create(readme_file_path.clone()).unwrap();
        let scan_result = vec![
            String::from("First safety report line"),
            String::from("Second safety report line"),
            String::from("Third safety report line"),
        ];

        writeln!(
            readme_file,
            "# Readme Header\nSome text\nAnother line\n## Another header\nMore text"
        ).unwrap();

        let result = create_or_replace_section_in_readme(
            readme_file_path.clone(),
            &scan_result,
        );

        assert!(result.is_ok());

        let updated_file_content =
            BufReader::new(File::open(readme_file_path.clone()).unwrap())
                .lines()
                .map(|l| l.unwrap())
                .collect::<Vec<String>>();

        let expected_readme_content = vec![
            String::from("# Readme Header"),
            String::from("Some text"),
            String::from("Another line"),
            String::from("## Another header"),
            String::from("More text"),
            CARGO_GEIGER_SAFETY_REPORT_SECTION_HEADER.to_string(),
            String::from("First safety report line"),
            String::from("Second safety report line"),
            String::from("Third safety report line"),
        ];

        assert_eq!(updated_file_content, expected_readme_content)
    }

    #[rstest(
        input_readme_content,
        expected_start_line_number,
        expected_end_line_number,
        case(
            vec![
                String::from("## Cargo Geiger Safety Report"),
                String::from("First line"),
                String::from("Second line")
            ],
            0,
            -1
        ),
        case(
            vec![
                String::from("# Cargo Geiger Safety Report"),
                String::from("First line"),
                String::from("Second line")
            ],
            0,
            -1
        ),
        case(
            vec![
                String::from("First line"),
                String::from("## Cargo Geiger Safety Report"),
                String::from("Second line"),
                String::from("Third line")
            ],
            1,
            -1
        ),
        case(
            vec![
                String::from("# Another header"),
                String::from("First line"),
                String::from("## Cargo Geiger Safety Report"),
                String::from("Second line"),
                String::from("Third line")
            ],
            2,
            -1
        ),
        case(
            vec![
                String::from("# Another header"),
                String::from("First line"),
                String::from("## Cargo Geiger Safety Report"),
                String::from("Second line"),
                String::from("Third line"),
                String::from("# Next header"),
                String::from("Fourth line")
            ],
            2,
            5
        )
    )]
    fn find_start_and_end_lines_of_safety_report_section_test(
        input_readme_content: Vec<String>,
        expected_start_line_number: i32,
        expected_end_line_number: i32,
    ) {
        let (start_line_number, end_line_number) =
            find_start_and_end_lines_of_safety_report_section(
                &input_readme_content,
            );

        assert_eq!(start_line_number, expected_start_line_number);
        assert_eq!(end_line_number, expected_end_line_number);
    }

    #[rstest]
    fn update_readme_content_test_no_safety_report_present() {
        let mut readme_content = vec![
            String::from("# readme header"),
            String::from("line of text"),
            String::from("another line of text"),
            String::from("## another header"),
        ];

        let scan_result = vec![
            String::from("first line of scan result"),
            String::from("second line of scan result"),
            String::from("third line of scan result"),
        ];

        update_readme_content(&mut readme_content, &scan_result);

        let expected_readme_content = vec![
            String::from("# readme header"),
            String::from("line of text"),
            String::from("another line of text"),
            String::from("## another header"),
            CARGO_GEIGER_SAFETY_REPORT_SECTION_HEADER.to_string(),
            String::from("first line of scan result"),
            String::from("second line of scan result"),
            String::from("third line of scan result"),
        ];

        assert_eq!(readme_content, expected_readme_content);
    }

    #[rstest]
    fn update_readme_content_test_safety_report_present_in_middle_of_readme() {
        let mut readme_content = vec![
            String::from("# readme header"),
            String::from("line of text"),
            String::from("another line of text"),
            CARGO_GEIGER_SAFETY_REPORT_SECTION_HEADER.to_string(),
            String::from("first line of old scan result"),
            String::from("second line of old scan result"),
            String::from("# another header"),
            String::from("line of text"),
        ];

        let scan_result = vec![
            String::from("first line of scan result"),
            String::from("second line of scan result"),
            String::from("third line of scan result"),
        ];

        update_readme_content(&mut readme_content, &scan_result);

        let expected_readme_content = vec![
            String::from("# readme header"),
            String::from("line of text"),
            String::from("another line of text"),
            CARGO_GEIGER_SAFETY_REPORT_SECTION_HEADER.to_string(),
            String::from("first line of scan result"),
            String::from("second line of scan result"),
            String::from("third line of scan result"),
            String::from("# another header"),
            String::from("line of text"),
        ];

        assert_eq!(readme_content, expected_readme_content);
    }
}
