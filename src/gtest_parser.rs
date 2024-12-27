use crate::types::{GtestExecutable, Test};
use anyhow::Result;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct GtestJson {
    // tests: u32,
    // name: String,
    testsuites: Vec<GtestTestSuite>,
}

#[derive(Debug, Deserialize)]
struct GtestTestSuite {
    name: String,
    // tests: u32,
    testsuite: Vec<GtestTest>,
}

#[derive(Debug, Deserialize)]
struct GtestTest {
    name: String,
    file: String,
    line: u32,
}

pub fn get_all_tests_from_executables(
    executables: &[GtestExecutable],
    extra_args: &[String],
) -> Result<Vec<Test>> {
    executables
        .iter()
        .map(|exec| get_all_tests_from_executable(exec, extra_args))
        .collect::<Result<Vec<_>>>()
        .map(|x| x.into_iter().flatten().collect::<Vec<_>>())
}

pub fn get_all_tests_from_executable(
    executable: &GtestExecutable,
    extra_args: &[String],
) -> Result<Vec<Test>> {
    let args = {
        let mut args = vec![
            String::from("--gtest_list_tests"),
            String::from("--gtest_output=json:/dev/stderr"),
        ];
        args.extend(
            extra_args
                .iter()
                .filter(|argument| argument.starts_with("--gtest_filter="))
                .cloned(),
        );
        args
    };

    let output = Command::new(&executable.path).args(args).output()?;

    let json: GtestJson = serde_json::from_str(&String::from_utf8_lossy(&output.stderr))?;

    Ok(json
        .testsuites
        .iter()
        .flat_map(|test_suite| {
            test_suite
                .testsuite
                .iter()
                .map(|test| {
                    let name = test_suite.name.clone() + "." + &test.name;

                    let mut arguments = vec![
                        format!("--gtest_filter={name}"),
                        String::from("--gtest_also_run_disabled_tests"),
                    ];

                    // Don't add the gtest filter since they were already filtered when fetching
                    // them from the executable
                    arguments.extend(
                        extra_args
                            .iter()
                            .filter(|argument| !argument.starts_with("--gtest_filter="))
                            .cloned(),
                    );

                    Test {
                        name: name.clone(),
                        file: Some(test.file.clone()),
                        line: Some(test.line),
                        executable: executable.clone(),
                        arguments,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>())
}
