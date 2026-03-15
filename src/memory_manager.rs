use anyhow::Result;
use std::path::PathBuf;

use crate::models::MemoryBundle;
use crate::utils::files::{append_string, ensure_dir, read_string_if_exists, write_string};

#[derive(Debug, Clone)]
pub struct MemoryManager {
    memory_dir: PathBuf,
    core_path: PathBuf,
    story_path: PathBuf,
    active_path: PathBuf,
}

impl MemoryManager {
    pub fn new(memory_dir: PathBuf) -> Self {
        let core_path = memory_dir.join("core_memory.md");
        let story_path = memory_dir.join("story_memory.md");
        let active_path = memory_dir.join("active_memory.md");

        Self {
            memory_dir,
            core_path,
            story_path,
            active_path,
        }
    }

    pub fn ensure_files(&self) -> Result<()> {
        ensure_dir(&self.memory_dir)?;

        if !self.core_path.exists() {
            write_string(
                &self.core_path,
                "# Core Memory\n\n- Genre: TBD\n- Tone: Focused, cinematic, character-driven\n- Long-term promise: A coherent serialized novel MVP\n",
            )?;
        }

        if !self.story_path.exists() {
            write_string(&self.story_path, "# Story Memory\n\n")?;
        }

        if !self.active_path.exists() {
            write_string(&self.active_path, "# Active Memory\n\n")?;
        }

        Ok(())
    }

    pub fn load_bundle(&self) -> Result<MemoryBundle> {
        self.ensure_files()?;

        Ok(MemoryBundle {
            core_memory: read_string_if_exists(&self.core_path)?.unwrap_or_default(),
            story_memory: read_string_if_exists(&self.story_path)?.unwrap_or_default(),
            active_memory: read_string_if_exists(&self.active_path)?.unwrap_or_default(),
        })
    }

    pub fn append_story_memory(&self, entry: &str) -> Result<()> {
        self.ensure_files()?;
        let formatted = if entry.ends_with('\n') {
            format!("\n{}", entry)
        } else {
            format!("\n{}\n", entry)
        };
        append_string(&self.story_path, &formatted)
    }

    pub fn overwrite_active_memory(&self, content: &str) -> Result<()> {
        self.ensure_files()?;
        write_string(&self.active_path, content)
    }
}
