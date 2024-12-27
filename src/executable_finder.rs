use crate::types::{ElfMetaData, GtestExecutable};
use anyhow::Result;
use goblin::{
    elf::{Elf, SectionHeader},
    Object,
};
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::{
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
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

pub fn find_gtest_executables(
    path: &Path,
    read_elf_metadata: bool,
) -> Result<Vec<GtestExecutable>> {
    Ok(walkdir::WalkDir::new(path)
        .into_iter()
        .par_bridge()
        .flatten()
        .filter_map(|entry| {
            if entry.path().is_file()
                && entry
                    .metadata()
                    // check if executable
                    .map(|metadata| metadata.permissions().mode() & 0b001001001 != 0)
                    .unwrap_or(false)
            {
                parse_gtest_executable(entry.path(), read_elf_metadata)
                    .ok()
                    .flatten()
            } else {
                None
            }
        })
        .collect::<Vec<_>>())
}

fn get_section_data(elf: &Elf, buffer: &[u8], section: &str) -> Result<Vec<String>> {
    Ok(elf
        .section_headers
        .iter()
        .filter_map(|header: &SectionHeader| {
            if &elf.shdr_strtab[header.sh_name] == section {
                let start: usize = header.sh_offset as usize;
                let end: usize = start + header.sh_size as usize;
                let data = &buffer[start..end];
                Some(
                    data.split(|c| *c == 0)
                        .filter(|data| !data.is_empty())
                        .map(|data| String::from_utf8_lossy(data).as_ref().to_owned())
                        .collect::<Vec<_>>(),
                )
            } else {
                // println!("Section name: {}", &elf.shdr_strtab[header.sh_name]);
                None
            }
        })
        .flatten()
        .collect::<Vec<_>>())
}

fn parse_gtest_executable(path: &Path, read_elf_metadata: bool) -> Result<Option<GtestExecutable>> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let Object::Elf(elf) = Object::parse(&buffer)? else {
        return Ok(None);
    };

    let is_gtest = elf
        .strtab
        .to_vec()?
        .into_iter()
        .any(|symbol: &str| symbol.contains("InitGoogleTest"));

    if !is_gtest {
        return Ok(None);
    }

    let modified_time = std::fs::metadata(path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let elf_metadata = if read_elf_metadata {
        let elf_comments = get_section_data(&elf, &buffer, ".comment")?;
        let dynamic_libraries = elf
            .dynamic
            .map(|dynamic| {
                dynamic
                    .dyns
                    .iter()
                    .filter(|dyn_entry| dyn_entry.d_tag == goblin::elf::dynamic::DT_NEEDED)
                    .filter_map(|dyn_entry| elf.dynstrtab.get_at(dyn_entry.d_val as usize))
                    .map(|lib_str| lib_str.to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Some(ElfMetaData {
            comments: elf_comments,
            dynamic_libraries,
        })
    } else {
        None
    };

    Ok(Some(GtestExecutable {
        path: path.to_owned(),
        elf_metadata,
        modified: modified_time,
    }))
}
