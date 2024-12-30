use crate::types::{Executable, ExecutableType};
use anyhow::{anyhow, bail, Result};
use elf_parser::{Elf, Section, SectionHeaders};
use faccess::PathExt;
use ignore::WalkBuilder;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    path::{Path, PathBuf},
    thread,
    time::UNIX_EPOCH,
};

pub fn find_test_dir(cli_path: &str, cli_no_parent: bool) -> Result<Option<PathBuf>> {
    let cli_path = PathBuf::from(cli_path);

    let mut current_dir = std::env::current_dir()?;

    loop {
        let test_dir = if cli_path.is_absolute() {
            cli_path.clone()
        } else {
            current_dir.join(&cli_path)
        };

        if test_dir.is_dir() && test_dir.exists() {
            break Ok(Some(test_dir.canonicalize()?));
        }

        if cli_no_parent || cli_path.is_absolute() {
            break Ok(None);
        }

        let Some(parent_dir) = current_dir.parent() else {
            break Ok(None);
        };

        current_dir = parent_dir.to_path_buf();
    }
}

pub fn validate_executables(executables: &[PathBuf]) -> Result<Vec<Executable>> {
    executables
        .par_iter()
        .map(|path| {
            let Ok(Some(gtest_executable)) = parse_test_executable(path, true, true) else {
                return Err(anyhow!(format!(
                    "{} is not a test executable",
                    path.display()
                )));
            };
            Ok(gtest_executable)
        })
        .collect::<Result<Vec<_>>>()
}

pub fn find_test_executables(
    path: &Path,
    jobs: Option<usize>,
    executable_types: &[ExecutableType],
) -> Result<Vec<Executable>> {
    let walker = WalkBuilder::new(path)
        .hidden(false)
        .ignore(false)
        .parents(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .follow_links(false)
        .threads(jobs.unwrap_or_default())
        .build_parallel();

    let (tx, rx) = crossbeam::channel::bounded::<Executable>(100);

    let is_gtest_enabled = executable_types.contains(&ExecutableType::Gtest);
    let is_catch2_enabled = executable_types.contains(&ExecutableType::Catch2);

    let mut tests = Vec::<Executable>::default();

    thread::scope(|scope| {
        scope.spawn(|| loop {
            match rx.recv() {
                Ok(test) => {
                    tests.push(test);
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
                    if let Ok(Some(executable)) =
                        parse_test_executable(path, is_gtest_enabled, is_catch2_enabled)
                    {
                        tx.send(executable).unwrap();
                    }
                }
                ignore::WalkState::Continue
            })
        });

        drop(tx);
    });

    Ok(tests)
}

pub fn parse_test_executable(
    path: &Path,
    is_gtest_enabled: bool,
    is_catch2_enabled: bool,
) -> Result<Option<Executable>> {
    let elf = Elf::new(path)?;

    let elf_type = elf.header.e_type();
    if elf_type != 0x02 && elf_type != 0x03 {
        return Ok(None);
    }

    let all_section_headers: SectionHeaders = elf.get_all_section_headers()?;
    let Some(symbol_table_header) = all_section_headers.find_symbol_table_header() else {
        return Ok(None);
    };

    let Some(string_table_header) = all_section_headers
        .headers
        .get(symbol_table_header.sh_link() as usize)
    else {
        bail!("Invalid ELF");
    };

    let Section::Symbols(symbols) = elf.get_section(symbol_table_header)? else {
        bail!("Invalid ELF");
    };

    let Section::Strings(strings) = elf.get_section(string_table_header)? else {
        bail!("Invalid ELF");
    };

    let test_executable_type = symbols.iter().find_map(|symbol| {
        strings
            .get_symbol_name(symbol)
            .map(|symbol_cstr| symbol_cstr.to_string_lossy())
            .and_then(|symbol| {
                if is_gtest_enabled && symbol.contains("InitGoogleTest") {
                    Some(ExecutableType::Gtest)
                } else if is_catch2_enabled && symbol.contains("Catch2") {
                    Some(ExecutableType::Catch2)
                } else {
                    None
                }
            })
    });

    let gtest_executable = test_executable_type.map(|test_executable_type| Executable {
        path: path.to_path_buf(),
        modified: path
            .metadata()
            .unwrap()
            .created()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        executable_type: test_executable_type,
    });

    Ok(gtest_executable)
}
