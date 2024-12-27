use crate::{types::Test, CwdRelativeTo, LaunchJsonCommand};
use serde::Serialize;
use std::{ops::Deref, path::PathBuf};

// Needed for skipping serializing false bools
fn is_false(b: &bool) -> bool {
    !b
}

#[derive(Debug, Clone, Serialize)]
struct VscodeLaunchJson {
    version: String,
    configurations: Vec<Configuration>,
}

#[derive(Debug, Clone, Serialize)]
struct SetupCommand {
    text: String,
    description: String,
    ignore_failures: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Configuration {
    name: String,
    r#type: String,
    request: String,
    program: String,
    args: Vec<String>,
    cwd: PathBuf,
    #[serde(skip_serializing_if = "is_false")]
    stop_at_entry: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    setup_commands: Vec<SetupCommand>,
}

pub fn format_tests_to_vscode_launch_json(tests: &[Test], command: &LaunchJsonCommand) -> String {
    let configurations = tests
        .iter()
        .map(|test| {
            let name = if command.add_exec_path_to_name {
                format!("{}:{}", test.name, test.executable.path.display())
            } else {
                test.name.clone()
            };

            let cwd = match command.launch_cwd_relative_to {
                CwdRelativeTo::Executable => {
                    let executable_directory = test
                        .executable
                        .path
                        .parent()
                        .unwrap_or_else(|| &test.executable.path);

                    executable_directory.join(&command.launch_cwd)
                }
                CwdRelativeTo::CppFile => {
                    let cpp_file_path = if let Some(file) = &test.file {
                        PathBuf::from(file)
                    } else {
                        test.executable.path.to_path_buf()
                    };
                    let cpp_file_directory =
                        cpp_file_path.parent().unwrap_or_else(|| &cpp_file_path);

                    cpp_file_directory.join(&command.launch_cwd)
                }
                CwdRelativeTo::None => command.launch_cwd.to_path_buf(),
            }
            .canonicalize()
            .unwrap();

            let setup_commands = if command.pretty_printing {
                vec![SetupCommand {
                    text: String::from("-enable-pretty-printing"),
                    description: String::from("Enable pretty printing"),
                    ignore_failures: false,
                }]
            } else {
                Vec::default()
            };

            Configuration {
                name,
                r#type: command.launch_type.to_string(),
                request: command.launch_request.to_string(),
                program: test.executable.path.to_string_lossy().deref().to_string(),
                args: test.arguments.clone(),
                stop_at_entry: command.stop_at_entry,
                cwd,
                setup_commands,
            }
        })
        .collect::<Vec<_>>();

    if command.configurations_only {
        serde_json::to_string_pretty(&configurations).unwrap()
    } else {
        let launch_json = VscodeLaunchJson {
            version: String::from("0.2.0"),
            configurations,
        };
        serde_json::to_string_pretty(&launch_json).unwrap()
    }
}
