use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize, Clone)]
pub struct Executable {
    pub path: PathBuf,
    pub modified: u128,
    pub executable_type: ExecutableType,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub enum ExecutableType {
    Gtest,
    Catch2,
}

#[derive(Debug, Serialize, Clone)]
pub struct Test {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    pub executable: Executable,
    pub arguments: Vec<String>,
}
