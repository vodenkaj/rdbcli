use std::{io::Read, os::unix::prelude::FileExt, process::Command};

use once_cell::sync::Lazy;

pub struct ExternalEditor {
    editor: String,
}

pub enum FileType {
    Json,
    Javascript,
}

impl FileType {
    fn get_ext(&self) -> &str {
        match self {
            FileType::Json => ".json",
            FileType::Javascript => ".js",
        }
    }
}

impl ExternalEditor {
    pub fn new(editor: &str) -> Self {
        Self {
            editor: String::from(editor),
        }
    }

    pub fn edit_value(&self, value: &mut String, file_type: FileType) -> anyhow::Result<String> {
        let file = tempfile::Builder::new()
            .suffix(file_type.get_ext())
            .tempfile()?;
        let mut handle = file.reopen()?;
        handle.write_all_at(value.as_bytes(), 0)?;
        Command::new(&self.editor)
            .current_dir(".")
            .arg(file.path())
            .status()?;

        let mut edited_value = String::new();
        handle.read_to_string(&mut edited_value)?;
        file.close()?;
        *value = edited_value.trim().to_string();

        Ok(value.to_string())
    }
}

pub static EXTERNAL_EDITOR: Lazy<ExternalEditor> = Lazy::new(|| ExternalEditor::new("nvim"));
