use std::{
    env,
    fmt::Debug,
    fs::{create_dir, File, OpenOptions},
    io::{Read, Write},
    path::{self, Path, PathBuf},
    process::Command,
};

use once_cell::sync::Lazy;

use crate::ui::layouts::CLI_ARGS;

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
        handle.write_all(value.as_bytes())?;
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

pub const MONGO_COLLECTIONS_FILE: Lazy<String> = Lazy::new(|| {
    let path = Path::new(CONFIG_PATH.as_str()).join(".collections.txt");

    if !path.exists() {
        File::create(path.clone()).expect("Failed to collections mongo file");
    }

    path.to_str().unwrap().to_string()
});

pub const HISTORY_FILE: Lazy<String> = Lazy::new(|| {
    let path = Path::new(CONFIG_PATH.as_str()).join(".command_history.txt");

    if !path.exists() {
        File::create(path.clone()).expect("Failed to create command history file");
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

pub struct DebugFile {
    location: PathBuf,
}

impl DebugFile {
    pub fn new(location: PathBuf) -> Self {
        if !location.exists() {
            File::create(location.clone()).unwrap();
        }

        Self { location }
    }

    pub fn write_log(&self, data: &impl Debug) {
        if !CLI_ARGS.debug {
            return;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(self.location.clone())
            .unwrap();
        file.write_all(
            format!(
                "\n[{}]: {:?}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                data
            )
            .as_bytes(),
        )
        .unwrap();
    }
}

pub static DEBUG_FILE: Lazy<DebugFile> = Lazy::new(|| {
    let config_path = CONFIG_PATH.clone().to_string();
    let debug_file_path = path::Path::new(&config_path).join("debug.log");
    DebugFile::new(debug_file_path)
});
