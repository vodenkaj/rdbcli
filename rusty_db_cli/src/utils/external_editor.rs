use std::{
    env,
    fs::{create_dir, File},
    io::Read,
    os::unix::prelude::FileExt,
    path::Path,
    process::Command,
};

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
        *value = edited_value.to_string();

        Ok(value.to_string())
    }

    pub fn edit_file(&self, path: &str) -> anyhow::Result<String> {
        let mut handle = File::open(path)?;
        Command::new(&self.editor)
            .current_dir(".")
            .arg(path)
            .status()?;

        let mut edited_value = String::new();
        handle.read_to_string(&mut edited_value)?;

        Ok(edited_value.to_string())
    }
}

pub static EXTERNAL_EDITOR: Lazy<ExternalEditor> = Lazy::new(|| {
    ExternalEditor::new(
        &env::vars()
            .find(|(key, _)| key == "EDITOR")
            .expect("EDITOR env to be set")
            .1,
    )
});

pub const MONGO_QUERY_FILE: Lazy<String> = Lazy::new(|| {
    let path = Path::new(CONFIG_PATH.as_str()).join(".mongo.js");

    if !path.exists() {
        File::create(path.clone()).expect("Failed to create mongo file");
    }

    path.to_str().unwrap().to_string()
});

const CONFIG_DIR_NAME: &str = "rusty_db_cli";

pub const CONFIG_PATH: Lazy<String> = Lazy::new(|| {
    let home = home::home_dir().expect("HomeDir to be available");

    let xdg_dir = home.join(".config");
    if !xdg_dir.exists() {
        create_dir(xdg_dir.clone()).expect("Failed to create .config dir");
    }
    let xdg_dir_config = xdg_dir.join(CONFIG_DIR_NAME);
    if !xdg_dir_config.exists() {
        create_dir(xdg_dir_config.clone()).expect("Failed to create .config/rusty_db_cli dir");
    }

    return xdg_dir_config.to_str().unwrap().to_string();
});
