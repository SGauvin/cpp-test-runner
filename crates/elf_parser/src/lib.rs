use bytemuck::{Pod, Zeroable};
use std::{ffi::CStr, io, os::unix::fs::FileExt, path::Path};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Io error: {0}")]
    IoError(#[from] io::Error),

    #[error("File is not an ELF")]
    NotAnElf,

    #[error("Elf is not 64 bits")]
    Not64Bits,

    #[error("Elf is not little endian")]
    NotLittleEndian,
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait FetchInteger {
    fn is_little_endian(&self) -> bool;
    fn data(&self) -> &[u8];

    fn get_u64(&self, offset: u64) -> Option<u64> {
        let offset = offset as usize;
        let slice = self.data().get(offset..offset + 8)?;
        if self.is_little_endian() {
            Some(u64::from_le_bytes(slice.try_into().unwrap()))
        } else {
            Some(u64::from_be_bytes(slice.try_into().unwrap()))
        }
    }

    fn get_u32(&self, offset: u64) -> Option<u32> {
        let offset = offset as usize;
        let slice = self.data().get(offset..offset + 4)?;
        if self.is_little_endian() {
            Some(u32::from_le_bytes(slice.try_into().unwrap()))
        } else {
            Some(u32::from_be_bytes(slice.try_into().unwrap()))
        }
    }

    fn get_u16(&self, offset: u64) -> Option<u16> {
        let offset = offset as usize;
        let slice = self.data().get(offset..offset + 2)?;
        if self.is_little_endian() {
            Some(u16::from_le_bytes(slice.try_into().unwrap()))
        } else {
            Some(u16::from_be_bytes(slice.try_into().unwrap()))
        }
    }

    fn get_u8(&self, offset: u64) -> Option<u8> {
        let offset = offset as usize;
        let slice = self.data().get(offset..offset + 1)?;
        if self.is_little_endian() {
            Some(u8::from_le_bytes(slice.try_into().unwrap()))
        } else {
            Some(u8::from_be_bytes(slice.try_into().unwrap()))
        }
    }
}

#[derive(Debug)]
pub struct Elf {
    pub header: Header,
    file: std::fs::File,
}

impl Elf {
    pub fn new(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;

        let header_buffer = {
            let mut header_buffer = [0u8; 64];
            file.read_exact_at(&mut header_buffer, 0)?;
            header_buffer
        };

        // Check ELF magic numbers
        let is_elf = &header_buffer[0..4] == b"\x7FELF";
        if !is_elf {
            return Err(Error::NotAnElf);
        }

        let header = Header { header_buffer };

        let executable = Elf { file, header };

        // We only support 64 bits ELF files
        if !executable.header.e_type_is_64_bits() {
            return Err(Error::Not64Bits);
        }

        Ok(executable)
    }

    pub fn get_all_section_headers(&self) -> std::result::Result<SectionHeaders, io::Error> {
        let mut all_section_headers: Vec<SectionHeader> =
            std::iter::repeat(SectionHeader::zeroed())
                .take(self.header.e_shnum() as usize)
                .collect();

        let all_section_headers_bytes: &mut [u8] =
            bytemuck::cast_slice_mut(&mut all_section_headers);

        self.file
            .read_exact_at(all_section_headers_bytes, self.header.e_shoff())?;

        Ok(SectionHeaders {
            headers: all_section_headers,
        })
    }

    pub fn get_section(
        &self,
        section_header: &SectionHeader,
    ) -> std::result::Result<Section, io::Error> {
        let header_type = section_header.sh_type();
        Ok(match header_type {
            0x2 => {
                let mut symbols: Vec<Elf64Sym> = std::iter::repeat(Elf64Sym::zeroed())
                    .take(section_header.sh_size() as usize / std::mem::size_of::<Elf64Sym>())
                    .collect();

                self.file.read_exact_at(
                    bytemuck::cast_slice_mut(&mut symbols),
                    section_header.sh_offset(),
                )?;

                Section::Symbols(symbols)
            }
            0x3 => {
                let mut data: Vec<u8> = std::iter::repeat(0u8)
                    .take(section_header.sh_size() as usize)
                    .collect();

                self.file
                    .read_exact_at(&mut data, section_header.sh_offset())?;

                Section::Strings(StringTable { data })
            }
            _ => Section::NotImplemented,
        })
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Header {
    header_buffer: [u8; 64],
}

impl FetchInteger for Header {
    fn is_little_endian(&self) -> bool {
        u8::from_le_bytes(self.header_buffer[0x5..0x6].try_into().unwrap()) == 1
    }

    fn data(&self) -> &[u8] {
        &self.header_buffer
    }
}

impl Header {
    pub fn e_type_is_64_bits(&self) -> bool {
        self.get_u8(0x4).unwrap() == 2
    }

    pub fn e_type_version(&self) -> u8 {
        self.get_u8(0x6).unwrap()
    }

    pub fn e_type_os_abi(&self) -> u8 {
        self.get_u8(0x7).unwrap()
    }

    pub fn e_type_abi_version(&self) -> u8 {
        self.get_u8(0x8).unwrap()
    }

    pub fn e_type(&self) -> u16 {
        self.get_u16(0x10).unwrap()
    }

    pub fn e_machine(&self) -> u16 {
        self.get_u16(0x12).unwrap()
    }

    pub fn e_version(&self) -> u32 {
        self.get_u32(0x14).unwrap()
    }

    pub fn e_entry(&self) -> u64 {
        self.get_u64(0x18).unwrap()
    }

    pub fn e_phoff(&self) -> u64 {
        self.get_u64(0x20).unwrap()
    }

    pub fn e_shoff(&self) -> u64 {
        self.get_u64(0x28).unwrap()
    }

    pub fn e_flags(&self) -> u32 {
        self.get_u32(0x30).unwrap()
    }

    pub fn e_ehsize(&self) -> u16 {
        self.get_u16(0x34).unwrap()
    }

    pub fn e_phentsize(&self) -> u16 {
        self.get_u16(0x36).unwrap()
    }

    pub fn e_phnum(&self) -> u16 {
        self.get_u16(0x38).unwrap()
    }

    pub fn e_shentsize(&self) -> u16 {
        self.get_u16(0x3A).unwrap()
    }

    pub fn e_shnum(&self) -> u16 {
        self.get_u16(0x3C).unwrap()
    }

    pub fn e_shstrndx(&self) -> u16 {
        self.get_u16(0x3E).unwrap()
    }
}

pub struct SectionHeaders {
    pub headers: Vec<SectionHeader>,
}

impl SectionHeaders {
    pub fn find_symbol_table_header(&self) -> Option<&SectionHeader> {
        self.headers.iter().find(|section| section.sh_type() == 2)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SectionHeader {
    data: [u8; 64],
}

impl FetchInteger for SectionHeader {
    fn is_little_endian(&self) -> bool {
        true
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}

impl SectionHeader {
    pub fn sh_name(&self) -> u32 {
        self.get_u32(0x00).unwrap()
    }

    pub fn sh_type(&self) -> u32 {
        self.get_u32(0x04).unwrap()
    }

    pub fn sh_flags(&self) -> u64 {
        self.get_u64(0x08).unwrap()
    }

    pub fn sh_addr(&self) -> u64 {
        self.get_u64(0x10).unwrap()
    }

    pub fn sh_offset(&self) -> u64 {
        self.get_u64(0x18).unwrap()
    }

    pub fn sh_size(&self) -> u64 {
        self.get_u64(0x20).unwrap()
    }

    pub fn sh_link(&self) -> u32 {
        self.get_u32(0x28).unwrap()
    }

    pub fn sh_info(&self) -> u32 {
        self.get_u32(0x2C).unwrap()
    }

    pub fn sh_addralign(&self) -> u64 {
        self.get_u64(0x30).unwrap()
    }

    pub fn sh_entsize(&self) -> u64 {
        self.get_u64(0x38).unwrap()
    }
}

pub enum Section {
    Symbols(Vec<Elf64Sym>),
    Strings(StringTable),
    NotImplemented,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, Pod, Zeroable)]
pub struct Elf64Sym {
    pub st_name: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub st_value: u64,
    pub st_size: u64,
}

pub struct StringTable {
    pub data: Vec<u8>,
}

impl<'a> StringTable {
    pub fn get_symbol_name(&'a self, symbol: &Elf64Sym) -> Option<&'a CStr> {
        let symbol_string_index = symbol.st_name as usize;
        let data_slice = self.data.get(symbol_string_index..)?;
        CStr::from_bytes_until_nul(data_slice).ok()
    }
}
