use serde::Serialize;
use std::path::PathBuf;

#[derive(Default, Debug, Serialize, Clone)]
pub struct ElfMetaData {
    pub comments: Vec<String>,
    pub dynamic_libraries: Vec<String>,
}

#[derive(Default, Debug, Serialize, Clone)]
pub struct GtestExecutable {
    pub path: PathBuf,
    pub modified: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elf_metadata: Option<ElfMetaData>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Test {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    pub executable: GtestExecutable,
    pub arguments: Vec<String>,
}
