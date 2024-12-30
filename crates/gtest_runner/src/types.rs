use serde::Serialize;
use skim::SkimItem;
use std::{borrow::Cow, path::PathBuf};

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

    #[serde(skip_serializing)]
    pub index: Option<usize>,
}

impl SkimItem for Test {
    fn text(&self) -> Cow<str> {
        Cow::Borrowed(&self.name)
    }

    fn get_index(&self) -> usize {
        self.index.unwrap_or_default()
    }

    fn set_index(&mut self, index: usize) {
        self.index = Some(index);
    }
}

impl Test {
    pub fn clone_with_index(&self, index: usize) -> Self {
        let mut clone = self.clone();
        clone.set_index(index);
        clone
    }
}
