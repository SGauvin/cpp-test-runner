mod executable_finder;
mod gtest_parser;
mod test_runner;
mod types;
mod vscode_launch_json_formatter;

use std::{ops::Deref, path::PathBuf};

use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use crossbeam::thread;
use executable_finder::{
    find_gtest_executables, find_test_dir, parse_gtest_executable, validate_executables,
};
use faccess::PathExt;
use gtest_parser::get_all_tests_from_executables;
use ignore::WalkBuilder;
use test_runner::run_all;
use types::Test;
use vscode_launch_json_formatter::format_tests_to_vscode_launch_json;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    fn common_flags(&self) -> &CommonFlags {
        self.command.common_flags()
    }

    fn elf_metadata(&self) -> bool {
        match &self.command {
            Command::List(cmd) => cmd.elf_metadata,
            _ => false,
        }
    }
}

#[derive(Default, Debug, Parser)]
struct CommonFlags {
    #[clap(flatten)]
    input: Option<Input>,

    /// Don't look up in parent directories when searching for the test directory.
    #[arg(long)]
    no_parent: bool,

    /// Limit the number of threads used by the application.
    #[arg(short, long)]
    jobs: Option<usize>,

    /// If set to true, the individual tests won't be parsed from the executables.
    #[arg(long)]
    executables_only: bool,

    /// Extra arguments to pass to the test executables.
    #[arg(long, value_delimiter = ',')]
    extra_args: Vec<String>,
}

#[derive(Args, Default, Debug)]
#[group(multiple = false)]
struct Input {
    /// The directory where to search for gtest executables.
    /// By default, if the path is relative, this program will search up the parent directories
    /// until it finds the test directory. Mutually exclusive with --executables. [default: .]
    #[arg(long)]
    test_dir: Option<String>,

    /// List all executables instead of searching them. Mutually exclusive with --test-dir.
    #[arg(long, value_delimiter = ',')]
    executables: Vec<PathBuf>,
}

#[derive(ValueEnum, Debug, Clone, Default)]
enum ColorOption {
    #[default]
    Auto,
    Yes,
    No,
}

#[derive(ValueEnum, Debug, Clone, Default)]
enum OutputFormat {
    Plain,
    #[default]
    Json,
    PrettyJson,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Find and list all tests and their executables.
    List(ListCommand),

    /// Print a vscode-compatible launch.json.
    LaunchJson(LaunchJsonCommand),

    /// Run tests.
    Run(RunCommand),
}

impl Command {
    fn common_flags(&self) -> &CommonFlags {
        match self {
            Self::List(cmd) => &cmd.common_flags,
            Self::Run(cmd) => &cmd.common_flags,
            Self::LaunchJson(cmd) => &cmd.common_flags,
        }
    }
}

#[derive(Default, Debug, Args)]
struct ListCommand {
    #[clap(flatten)]
    common_flags: CommonFlags,

    /// Include elf metadata of the binary files.
    #[arg(long)]
    elf_metadata: bool,

    /// Choose the output format of the list.
    #[arg(long, value_enum, default_value = "json")]
    output: OutputFormat,
}

#[derive(ValueEnum, Debug, Clone, Default)]
pub enum CwdRelativeTo {
    #[default]
    Executable,
    CppFile,
    None,
}

#[derive(Default, Debug, Args)]
struct LaunchJsonCommand {
    #[clap(flatten)]
    common_flags: CommonFlags,

    /// The type of debugger of the launch configuration.
    #[arg(long, default_value = "cppdbg")]
    launch_type: String,

    /// The request type of the launch configuration.
    #[arg(long, default_value = "launch")]
    launch_request: String,

    /// The cwd of the tests. Change launch-cwd-relative-to to modify to what the cwd is relative to.
    #[arg(long, value_enum, default_value = ".")]
    launch_cwd: PathBuf,

    /// Controls to what the cwd is relative to.
    #[arg(long, value_enum, default_value = "executable")]
    launch_cwd_relative_to: CwdRelativeTo,

    /// Appends the executable path to the test name. Useful for distinguishing between tests with duplicate names.
    #[arg(long)]
    add_exec_path_to_name: bool,

    /// Only print the list of configurations.
    #[arg(long)]
    configurations_only: bool,

    /// Add the stopAtEntry option to the config.
    #[arg(long)]
    stop_at_entry: bool,

    /// Enable pretty printing in the debugger.
    #[arg(long)]
    pretty_printing: bool,
}

#[derive(Default, Debug, Args)]
struct RunCommand {
    #[clap(flatten)]
    common_flags: CommonFlags,

    /// Enable or disable colored output.
    #[arg(long, value_enum, default_value = "auto")]
    color: ColorOption,
}

fn main() -> Result<()> {
    let walker = WalkBuilder::new(".")
        .hidden(false)
        .ignore(false)
        .parents(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .follow_links(false)
        .threads(0)
        .build_parallel();

    let (tx, rx) = crossbeam::channel::bounded::<PathBuf>(100);

    let mut vec = Vec::<PathBuf>::default();

    let a = thread::scope(|scope| {
        scope.spawn(|_| loop {
            match rx.recv() {
                Ok(test) => {
                    if let Ok(Some(_)) = parse_gtest_executable(&test, false) {
                        vec.push(test);
                    }
                }
                Err(_) => {
                    return;
                }
            }
        });

        walker.run(|| {
            let tx = tx.clone();
            Box::new(move |result| {
                let path = result.as_ref().unwrap().path();
                if path.is_file() && path.executable() {
                    tx.send(path.to_path_buf()).unwrap();
                }
                ignore::WalkState::Continue
            })
        });

        drop(tx);
    });

    // Ok(vec)
    println!("{vec:#?}");

    // let args = Cli::parse();
    //
    // if let Some(jobs) = &args.common_flags().jobs {
    //     rayon::ThreadPoolBuilder::new()
    //         .num_threads(*jobs)
    //         .build_global()?;
    // }
    //
    // let input = args.common_flags().input.as_ref();
    // let executables = {
    //     let cli_executables = input
    //         .map(|input| input.executables.clone())
    //         .unwrap_or(Default::default());
    //
    //     if !cli_executables.is_empty() {
    //         validate_executables(&cli_executables, args.common_flags().no_parent)
    //     } else {
    //         let test_dir = input
    //             .and_then(|input| input.test_dir.clone())
    //             .unwrap_or_else(|| String::from("."));
    //
    //         let Some(test_dir) = find_test_dir(&test_dir, args.common_flags().no_parent)? else {
    //             bail!("test_dir {test_dir} not found");
    //         };
    //
    //         find_gtest_executables(&test_dir, args.elf_metadata())
    //     }
    // }?;
    //
    // let extra_args = args
    //     .common_flags()
    //     .extra_args
    //     .iter()
    //     .map(|x| {
    //         if x.starts_with("-") {
    //             x.clone()
    //         } else {
    //             format!("--{x}")
    //         }
    //     })
    //     .collect::<Vec<_>>();
    //
    // let tests = if args.common_flags().executables_only {
    //     executables
    //         .into_iter()
    //         .map(|executable| Test {
    //             name: executable.path.to_string_lossy().deref().to_string(),
    //             file: None,
    //             line: None,
    //             executable,
    //             arguments: extra_args.clone(),
    //         })
    //         .collect::<Vec<_>>()
    // } else {
    //     get_all_tests_from_executables(&executables, &extra_args)?
    // };
    //
    // match args.command {
    //     Command::List(command) => match command.output {
    //         OutputFormat::Json => {
    //             println!("{}", serde_json::to_string(&tests)?);
    //         }
    //         OutputFormat::PrettyJson => {
    //             println!("{}", serde_json::to_string_pretty(&tests)?);
    //         }
    //         OutputFormat::Plain => {
    //             let all_test_names =
    //                 tests
    //                     .iter()
    //                     .map(|test| &test.name)
    //                     .fold(String::new(), |mut list, name| {
    //                         list.push_str(&format!("{name}\n"));
    //                         list
    //                     });
    //             print!("{all_test_names}");
    //         }
    //     },
    //     Command::LaunchJson(command) => {
    //         println!("{}", format_tests_to_vscode_launch_json(&tests, &command));
    //     }
    //     Command::Run(command) => {
    //         let use_color = match command.color {
    //             ColorOption::No => false,
    //             ColorOption::Yes => true,
    //             ColorOption::Auto => atty::is(atty::Stream::Stdout),
    //         };
    //
    //         run_all(&tests, use_color)?;
    //     }
    // }

    Ok(())
}
