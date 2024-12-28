use crate::types::GtestExecutable;
use anyhow::{anyhow, Result};
use bytemuck::{Pod, Zeroable};
use faccess::PathExt;
use ignore::WalkBuilder;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    io::{Read, Seek, SeekFrom},
    os::{raw::c_uchar, unix::fs::FileExt},
    path::{Path, PathBuf},
    thread,
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

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Elf64Sym {
    pub st_name: u32,
    pub st_info: c_uchar,
    pub st_other: c_uchar,
    pub st_shndx: u16,
    pub st_value: u64,
    pub st_size: u64,
}

fn get_u64(data: &[u8], offset: u64, is_little_endian: bool) -> u64 {
    let offset = offset as usize;
    if is_little_endian {
        u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
    } else {
        u64::from_be_bytes(data[offset..offset + 8].try_into().unwrap())
    }
}

fn get_u32(data: &[u8], offset: u64, is_little_endian: bool) -> u32 {
    let offset = offset as usize;
    if is_little_endian {
        u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
    } else {
        u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap())
    }
}

fn get_u16(data: &[u8], offset: u64, is_little_endian: bool) -> u16 {
    let offset = offset as usize;
    if is_little_endian {
        u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap())
    } else {
        u16::from_be_bytes(data[offset..offset + 2].try_into().unwrap())
    }
}

fn get_u8(data: &[u8], offset: u64, is_little_endian: bool) -> u8 {
    let offset = offset as usize;
    if is_little_endian {
        u8::from_le_bytes(data[offset..offset + 1].try_into().unwrap())
    } else {
        u8::from_be_bytes(data[offset..offset + 1].try_into().unwrap())
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SectionHeader {
    data: [u8; 64],
}

impl Default for SectionHeader {
    fn default() -> Self {
        Self { data: [0u8; 64] }
    }
}

impl SectionHeader {
    fn sh_name(&self, is_little_endian: bool) -> u32 {
        get_u32(&self.data, 0x00, is_little_endian)
    }

    fn sh_type(&self, is_little_endian: bool) -> u32 {
        get_u32(&self.data, 0x04, is_little_endian)
    }

    fn sh_flags(&self, is_little_endian: bool) -> u64 {
        get_u64(&self.data, 0x08, is_little_endian)
    }

    fn sh_addr(&self, is_little_endian: bool) -> u64 {
        get_u64(&self.data, 0x10, is_little_endian)
    }

    fn sh_offset(&self, is_little_endian: bool) -> u64 {
        get_u64(&self.data, 0x18, is_little_endian)
    }

    fn sh_size(&self, is_little_endian: bool) -> u64 {
        get_u64(&self.data, 0x20, is_little_endian)
    }

    fn sh_link(&self, is_little_endian: bool) -> u32 {
        get_u32(&self.data, 0x28, is_little_endian)
    }

    fn sh_info(&self, is_little_endian: bool) -> u32 {
        get_u32(&self.data, 0x2C, is_little_endian)
    }

    fn sh_addralign(&self, is_little_endian: bool) -> u64 {
        get_u64(&self.data, 0x30, is_little_endian)
    }

    fn sh_entsize(&self, is_little_endian: bool) -> u64 {
        get_u64(&self.data, 0x38, is_little_endian)
    }
}

pub fn validate_executables(
    executables: &[PathBuf],
    read_elf_metadata: bool,
) -> Result<Vec<GtestExecutable>> {
    executables
        .par_iter()
        .map(|path| {
            let Ok(Some(gtest_executable)) = parse_gtest_executable(path, read_elf_metadata) else {
                return Err(anyhow!(format!(
                    "{} is not a gtest executable",
                    path.display()
                )));
            };
            Ok(gtest_executable)
        })
        .collect::<Result<Vec<_>>>()
}

pub fn find_gtest_executables(
    path: &Path,
    read_elf_metadata: bool,
) -> Result<Vec<GtestExecutable>> {
    let walker = WalkBuilder::new(path)
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

    let (tx, rx) = crossbeam::channel::bounded::<GtestExecutable>(100);

    let mut vec = Vec::<GtestExecutable>::default();

    thread::scope(|scope| {
        scope.spawn(|| loop {
            match rx.recv() {
                Ok(test) => {
                    vec.push(test);
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
                    if let Some(test) = parse_gtest_executable(path, read_elf_metadata)
                        .ok()
                        .flatten()
                    {
                        tx.send(test).unwrap();
                    }
                }
                ignore::WalkState::Continue
            })
        });

        drop(tx);
    });

    Ok(vec)
}

pub fn parse_gtest_executable(
    path: &Path,
    _read_elf_metadata: bool,
) -> Result<Option<GtestExecutable>> {
    let mut file = std::fs::File::open(path)?;

    let mut header_buffer = [0u8; 64]; // Read only the first 64 bytes
    file.read_exact(&mut header_buffer)?;

    let is_little_endian = { u8::from_le_bytes(header_buffer[0x5..0x6].try_into().unwrap()) == 1 };

    let is_64_bits_executable_elf = {
        let is_elf = &header_buffer[0..4] == b"\x7FELF";
        let is_executable = {
            let e_type = get_u16(&header_buffer, 0x10, is_little_endian);
            e_type == 2 || e_type == 3
        };
        let has_valid_entry_point = get_u64(&header_buffer, 0x18, is_little_endian) != 0;
        let section_header_entry_size_is_64 = get_u16(&header_buffer, 0x3A, is_little_endian) == 64;

        is_elf && is_executable && has_valid_entry_point && section_header_entry_size_is_64
    };

    if !is_64_bits_executable_elf {
        return Ok(None);
    }

    let section_header_offset = get_u64(&header_buffer, 0x28, is_little_endian);
    let section_header_count = get_u16(&header_buffer, 0x3C, is_little_endian);

    let all_section_headers = {
        let mut all_section_headers: Vec<SectionHeader> =
            std::iter::repeat(SectionHeader::default())
                .take(section_header_count as usize)
                .collect();

        let all_section_headers_bytes: &mut [u8] =
            bytemuck::try_cast_slice_mut(&mut all_section_headers).unwrap();

        file.read_exact_at(all_section_headers_bytes, section_header_offset)?;

        all_section_headers
    };

    let Some(symbol_table) = all_section_headers
        .iter()
        .find(|x| x.sh_type(is_little_endian) == 2)
    else {
        return Ok(None);
    };

    let Some(string_table) =
        all_section_headers.get(symbol_table.sh_link(is_little_endian) as usize)
    else {
        return Ok(None);
    };

    println!(
        "{}: {} && {}",
        path.display(),
        string_table.sh_type(is_little_endian),
        string_table.sh_flags(is_little_endian) & 0x20 != 0
    );

    Ok(None)
}
