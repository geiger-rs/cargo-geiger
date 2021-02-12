use crate::args::ReadmeArgs;

use cargo::{CliError, CliResult};
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader, Error, Write};
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
    readme_args: &ReadmeArgs,
    scan_output_lines: &[String],
) -> CliResult {
    let readme_path_buf =
        get_readme_path_buf_from_arguments_or_default(readme_args);

    if !readme_path_buf.exists() {
        eprintln!(
            "File: {} does not exist. To construct a Cargo Geiger Safety Report section, please first create a README.",
            readme_path_buf.to_str().unwrap()
        );
        return CliResult::Err(CliError::code(1));
    }

    let mut readme_content =
        read_file_contents(&readme_path_buf).map_err(|e| {
            eprintln!(
                "Failed to read contents from file: {}",
                readme_path_buf.to_str().unwrap()
            );
            anyhow::Error::from(e)
        })?;

    update_readme_content(readme_args, &mut readme_content, scan_output_lines);

    write_lines_to_file(&readme_content, &readme_path_buf).map_err(|e| {
        eprintln!(
            "Failed to write lines to file: {}",
            readme_path_buf.to_str().unwrap()
        );
        anyhow::Error::from(e)
    })?;

    Ok(())
}

/// For a `&Vec<String` find the index of the first and last lines of a Safety Report Section. If
/// the Section is not present, -1 is returned for both values, and if the Section is the last
/// section present, then the last index is -1
fn find_start_and_end_lines_of_safety_report_section(
    readme_args: &ReadmeArgs,
    readme_content: &[String],
) -> (i32, i32) {
    let mut start_line_number = -1;
    let mut end_line_number = -1;

    let start_line_pattern =
        construct_regex_expression_for_section_header(readme_args);

    let end_line_pattern = Regex::new("^#+.*").unwrap();

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

/// Constructs a regex expression for the Section Name if provided as an argument,
/// otherwise returns a regex expression for the default Section Name
fn construct_regex_expression_for_section_header(
    readme_args: &ReadmeArgs,
) -> Regex {
    match &readme_args.section_name {
        Some(section_name) => {
            let mut regex_string = String::from("^#+\\s");
            regex_string.push_str(&section_name.replace(' ', "\\s"));
            regex_string.push_str("\\s*");

            Regex::new(&regex_string).unwrap()
        }
        None => {
            Regex::new("^#+\\sCargo\\sGeiger\\sSafety\\sReport\\s*").unwrap()
        }
    }
}

/// Returns the `PathBuf` passed in as an argument value if one exists, otherwise
/// returns the `PathBuf` to a file `README.md` in the current directory
fn get_readme_path_buf_from_arguments_or_default(
    readme_args: &ReadmeArgs,
) -> PathBuf {
    match &readme_args.readme_path {
        Some(readme_path) => readme_path.to_path_buf(),
        None => {
            let mut current_dir_path_buf = std::env::current_dir().unwrap();
            current_dir_path_buf.push(README_FILENAME);
            current_dir_path_buf
        }
    }
}

/// Read the contents of a file line by line.
fn read_file_contents(path_buf: &PathBuf) -> Result<Vec<String>, Error> {
    let file = File::open(path_buf)?;
    let buf_reader = BufReader::new(file);

    Ok(buf_reader
        .lines()
        .filter_map(|l| l.ok())
        .collect::<Vec<String>>())
}

/// Update the content of a README.md with a Scan Result. When the section doesn't exist, it will
/// be created with an `h2` level header, otherwise it will preserve the level of the existing
/// header
fn update_readme_content(
    readme_args: &ReadmeArgs,
    readme_content: &mut Vec<String>,
    scan_result: &[String],
) {
    let (start_line_number, mut end_line_number) =
        find_start_and_end_lines_of_safety_report_section(
            readme_args,
            &readme_content,
        );

    if start_line_number == -1 {
        // When Cargo Geiger Safety Report isn't present in README, add an
        // h2 headed section at the end of the README.md containing the report
        match &readme_args.section_name {
            Some(section_name) => {
                let mut section_name_string = String::from("## ");
                section_name_string.push_str(section_name);

                readme_content.push(section_name_string);
            }
            None => {
                readme_content.push(
                    CARGO_GEIGER_SAFETY_REPORT_SECTION_HEADER.to_string(),
                );
            }
        }
        readme_content.push(String::from("```"));
        for scan_result_line in scan_result {
            readme_content.push(scan_result_line.to_string())
        }
        readme_content.push(String::from("```"));
    } else {
        if end_line_number == -1 {
            end_line_number = readme_content.len() as i32;
        }

        // When Cargo Geiger Safety Report is present in README, remove the
        // section and and replace, preserving header level (h1/h2/h3)
        for _ in start_line_number + 1..end_line_number {
            readme_content.remove((start_line_number + 1) as usize);
        }

        let mut running_scan_line_index = start_line_number + 1;

        readme_content
            .insert(running_scan_line_index as usize, String::from("```"));
        running_scan_line_index += 1;

        for scan_result_line in scan_result {
            readme_content.insert(
                running_scan_line_index as usize,
                scan_result_line.to_string(),
            );
            running_scan_line_index += 1;
        }

        readme_content
            .insert(running_scan_line_index as usize, String::from("```"));
    }
}

/// Write a Vec<String> line by line to a file, overwriting the current file, if it exists.
fn write_lines_to_file(
    lines: &[String],
    path_buf: &PathBuf,
) -> Result<(), Error> {
    let mut readme_file = File::create(path_buf)?;

    for line in lines {
        writeln!(readme_file, "{}", line)?
    }

    Ok(())
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
        let readme_path = temp_dir.path().join("README.md");

        let readme_args = ReadmeArgs {
            readme_path: Some(readme_path),
            ..Default::default()
        };

        let scan_result = vec![];

        let result =
            create_or_replace_section_in_readme(&readme_args, &scan_result);

        assert!(result.is_err());
    }

    #[rstest]
    fn create_or_replace_section_test_readme_doesnt_contain_section() {
        let temp_dir = tempdir().unwrap();
        let readme_path = temp_dir.path().join("README.md");

        let readme_args = ReadmeArgs {
            readme_path: Some(readme_path.clone()),
            ..Default::default()
        };

        let mut readme_file = File::create(readme_path.clone()).unwrap();
        let scan_result = vec![
            String::from("First safety report line"),
            String::from("Second safety report line"),
            String::from("Third safety report line"),
        ];

        writeln!(
            readme_file,
            "# Readme Header\nSome text\nAnother line\n## Another header\nMore text"
        ).unwrap();

        let result =
            create_or_replace_section_in_readme(&readme_args, &scan_result);

        assert!(result.is_ok());

        let updated_file_content =
            BufReader::new(File::open(readme_path).unwrap())
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
            String::from("```"),
            String::from("First safety report line"),
            String::from("Second safety report line"),
            String::from("Third safety report line"),
            String::from("```"),
        ];

        assert_eq!(updated_file_content, expected_readme_content)
    }

    #[rstest(
        input_readme_args,
        expected_regex_expression,
        case(
            ReadmeArgs{
                section_name: None,
                ..Default::default()
            },
            Regex::new("^#+\\sCargo\\sGeiger\\sSafety\\sReport\\s*").unwrap()
        ),
        case(
            ReadmeArgs{
                section_name: Some(String::from("Test Section Name")),
                ..Default::default()
            },
            Regex::new("^#+\\sTest\\sSection\\sName\\s*").unwrap()
        )
    )]
    fn construct_regex_expression_for_section_header_test(
        input_readme_args: ReadmeArgs,
        expected_regex_expression: Regex,
    ) {
        let regex_expression =
            construct_regex_expression_for_section_header(&input_readme_args);

        assert_eq!(
            regex_expression.as_str(),
            expected_regex_expression.as_str()
        );
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
        let readme_args = ReadmeArgs::default();

        let (start_line_number, end_line_number) =
            find_start_and_end_lines_of_safety_report_section(
                &readme_args,
                &input_readme_content,
            );

        assert_eq!(start_line_number, expected_start_line_number);
        assert_eq!(end_line_number, expected_end_line_number);
    }

    #[rstest]
    fn get_readme_path_buf_from_arguments_or_default_test_none() {
        let mut path_buf = std::env::current_dir().unwrap();
        path_buf.push(README_FILENAME);

        let readme_args = ReadmeArgs {
            readme_path: None,
            ..Default::default()
        };

        let readme_path_buf =
            get_readme_path_buf_from_arguments_or_default(&readme_args);

        assert_eq!(readme_path_buf, path_buf)
    }

    #[rstest]
    fn get_readme_path_buf_from_arguments_or_default_test_some() {
        let path_buf = PathBuf::from("/test/path");

        let readme_args = ReadmeArgs {
            readme_path: Some(path_buf.clone()),
            ..Default::default()
        };

        let readme_path_buf =
            get_readme_path_buf_from_arguments_or_default(&readme_args);

        assert_eq!(readme_path_buf, path_buf);
    }

    #[rstest(
        input_readme_args,
        expected_section_header,
        case(
            ReadmeArgs{
                section_name: None,
                ..Default::default()
            },
            CARGO_GEIGER_SAFETY_REPORT_SECTION_HEADER.to_string()
        ),
        case(
            ReadmeArgs{
                section_name: Some(String::from("Test Section Name")),
                ..Default::default()
            },
            String::from("## Test Section Name")
        )
    )]
    fn update_readme_content_test_no_safety_report_present(
        input_readme_args: ReadmeArgs,
        expected_section_header: String,
    ) {
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

        update_readme_content(
            &input_readme_args,
            &mut readme_content,
            &scan_result,
        );

        let expected_readme_content = vec![
            String::from("# readme header"),
            String::from("line of text"),
            String::from("another line of text"),
            String::from("## another header"),
            expected_section_header,
            String::from("```"),
            String::from("first line of scan result"),
            String::from("second line of scan result"),
            String::from("third line of scan result"),
            String::from("```"),
        ];

        assert_eq!(readme_content, expected_readme_content);
    }

    #[rstest]
    fn update_readme_content_test_safety_report_present_in_middle_of_readme() {
        let readme_args = ReadmeArgs::default();

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

        update_readme_content(&readme_args, &mut readme_content, &scan_result);

        let expected_readme_content = vec![
            String::from("# readme header"),
            String::from("line of text"),
            String::from("another line of text"),
            CARGO_GEIGER_SAFETY_REPORT_SECTION_HEADER.to_string(),
            String::from("```"),
            String::from("first line of scan result"),
            String::from("second line of scan result"),
            String::from("third line of scan result"),
            String::from("```"),
            String::from("# another header"),
            String::from("line of text"),
        ];

        assert_eq!(readme_content, expected_readme_content);
    }
}
