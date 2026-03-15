use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))
}

pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    Ok(())
}

pub fn write_string(path: &Path, content: &str) -> Result<()> {
    ensure_parent(path)?;
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}

pub fn read_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

pub fn read_string_if_exists(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub fn append_string(path: &Path, content: &str) -> Result<()> {
    use std::io::Write;

    ensure_parent(path)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;

    file.write_all(content.as_bytes())
        .with_context(|| format!("failed to append {}", path.display()))
}

pub fn list_markdown_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("failed to inspect {}", dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("md") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}
