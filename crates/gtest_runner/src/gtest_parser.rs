use crate::types::{GtestExecutable, Test};
use anyhow::{bail, Result};
use serde::Deserialize;
use std::{ops::Deref, process::Command};

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
    exectuables_only: bool,
    extra_args: &[String],
) -> Vec<Test> {
    executables
        .iter()
        .filter_map(|exec| get_all_tests_from_executable(exec, exectuables_only, extra_args).ok())
        .flatten()
        .collect::<Vec<Test>>()
}

pub fn get_all_tests_from_executable(
    executable: &GtestExecutable,
    exectuables_only: bool,
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
    if !output.status.success() {
        // Some executables link against gtest even if they aren't test executables (like AIDL).
        // This here is a fail-safe that prevent these kind of executables to show up as google
        // test executables.
        bail!("{} is not a gtest executable!", executable.path.display());
    }

    let json: GtestJson = serde_json::from_str(&String::from_utf8_lossy(&output.stderr))?;

    if exectuables_only {
        return Ok(vec![Test {
            name: executable.path.to_string_lossy().deref().to_string(),
            file: None,
            line: None,
            executable: executable.clone(),
            arguments: extra_args.to_vec(),
        }]);
    }

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
