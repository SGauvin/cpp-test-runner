use serde::Serialize;
use skim::{ItemPreview, PreviewPosition, SkimItem};
use std::{
    borrow::Cow,
    io::{BufRead, Cursor},
    path::PathBuf,
    sync::LazyLock,
};
use syntect::{
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
    util::as_24_bit_terminal_escaped,
};

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

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME: LazyLock<Theme> = LazyLock::new(|| {
    let theme = include_str!("Catppuccin Macchiato.tmTheme");
    let mut reader = Cursor::new(theme);
    ThemeSet::load_from_reader(&mut reader).unwrap()
});

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

    fn preview(&self, _context: skim::prelude::PreviewContext) -> ItemPreview {
        if let Some(file) = &self.file {
            let mut highlighter =
                syntect::easy::HighlightFile::new(file, &SYNTAX_SET, &THEME).unwrap();

            let mut content = String::default();
            let mut line = String::default();
            while highlighter.reader.read_line(&mut line).unwrap_or(0) > 0 {
                let regions: Vec<_> = highlighter
                    .highlight_lines
                    .highlight_line(&line, &SYNTAX_SET)
                    .unwrap();

                content.push_str(&as_24_bit_terminal_escaped(&regions[..], false));
                line.clear();
            }

            ItemPreview::AnsiWithPos(
                content,
                PreviewPosition {
                    v_scroll: tuikit::prelude::Size::Fixed(self.line.unwrap_or(0) as usize),
                    ..Default::default()
                },
            )
        } else {
            ItemPreview::Global
        }
    }
}

impl Test {
    pub fn clone_with_index(&self, index: usize) -> Self {
        let mut clone = self.clone();
        clone.set_index(index);
        clone
    }
}
