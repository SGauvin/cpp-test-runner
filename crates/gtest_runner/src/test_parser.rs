use crate::types::{Executable, ExecutableType, Test};
use anyhow::{bail, Result};
use serde::Deserialize;
use std::{ops::Deref, path::PathBuf, process::Command};

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
    file: PathBuf,
    line: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Catch2Json {
    // version: u32,
    listings: Catch2Listings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Catch2Listings {
    tests: Vec<Catch2Test>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Catch2Test {
    name: String,
    // class_name: String,
    // tags: Vec<String>,
    source_location: Catch2SourceLocation,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Catch2SourceLocation {
    filename: PathBuf,
    line: u32,
}

pub fn get_tests_from_executables(
    executables: &[Executable],
    exectuables_only: bool,
    extra_args: &[String],
) -> Vec<Test> {
    executables
        .iter()
        .filter_map(|exec| get_tests_from_executable(exec, exectuables_only, extra_args).ok())
        .flatten()
        .collect::<Vec<Test>>()
}

pub fn get_tests_from_executable(
    executable: &Executable,
    exectuables_only: bool,
    extra_args: &[String],
) -> Result<Vec<Test>> {
    match executable.executable_type {
        ExecutableType::Gtest => {
            get_tests_from_gtest_executable(executable, exectuables_only, extra_args)
        }
        ExecutableType::Catch2 => {
            get_tests_from_catch2_executable(executable, exectuables_only, extra_args)
        }
    }
}

pub fn get_tests_from_gtest_executable(
    executable: &Executable,
    executable_only: bool,
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
        bail!("{} is not a gtest executable!", executable.path.display());
    }

    let Ok::<GtestJson, serde_json::Error>(json) =
        serde_json::from_str(&String::from_utf8_lossy(&output.stderr))
    else {
        bail!("{} Failed to parse gtest json", executable.path.display());
    };

    if executable_only {
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
            test_suite.testsuite.iter().map(|test| {
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
        })
        .collect::<Vec<_>>())
}

pub fn get_tests_from_catch2_executable(
    executable: &Executable,
    executable_only: bool,
    extra_args: &[String],
) -> Result<Vec<Test>> {
    let is_catch2_executable = {
        let output = Command::new(&executable.path)
            .arg("--libidentify")
            .output()?;

        if !output.status.success() {
            false
        } else {
            String::from_utf8_lossy(&output.stdout).lines().any(|line| {
                line.split_once(':')
                    .map(|(key, value)| key.trim() == "framework" && value.trim() == "Catch2")
                    .unwrap_or(false)
            })
        }
    };

    if !is_catch2_executable {
        return Ok(Vec::new());
    }

    if executable_only {
        return Ok(vec![Test {
            name: executable.path.to_string_lossy().deref().to_string(),
            file: None,
            line: None,
            executable: executable.clone(),
            arguments: extra_args.to_vec(),
        }]);
    }

    let output = Command::new(&executable.path)
        .args(["--list-tests", "--reporter=JSON"])
        .output()?;

    if !output.status.success() {
        bail!("{} is not a catch2 executable!", executable.path.display());
    }

    let Ok::<Catch2Json, serde_json::Error>(json) =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout))
    else {
        bail!("{} Failed to parse catch2 json", executable.path.display());
    };

    Ok(json
        .listings
        .tests
        .iter()
        .map(|test| Test {
            name: test.name.clone(),
            file: Some(test.source_location.filename.clone()),
            line: Some(test.source_location.line),
            executable: executable.clone(),
            arguments: vec![test.name.clone()],
        })
        .collect::<Vec<_>>())
}
