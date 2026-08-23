use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rfd::FileDialog;

use crate::config::language_for_path;

#[derive(Debug)]
pub struct LoadedFile {
    pub path: PathBuf,
    pub text: String,
    pub language: String,
}

pub fn open_file_dialog() -> io::Result<Option<LoadedFile>> {
    let Some(path) = FileDialog::new()
        .set_title("Open file")
        .pick_file()
    else {
        return Ok(None);
    };

    load_file(&path).map(Some)
}

pub fn load_file(path: &Path) -> io::Result<LoadedFile> {
    let text = fs::read_to_string(path)?;
    let language = language_for_path(path).to_string();

    Ok(LoadedFile {
        path: path.to_path_buf(),
        text,
        language,
    })
}

pub fn save_file(path: &Path, text: &str) -> io::Result<()> {
    fs::write(path, text)
}

pub fn save_file_dialog(default_name: &str, text: &str) -> io::Result<Option<PathBuf>> {
    let Some(path) = FileDialog::new()
        .set_title("Save file")
        .set_file_name(default_name)
        .save_file()
    else {
        return Ok(None);
    };

    save_file(&path, text)?;
    Ok(Some(path))
}
