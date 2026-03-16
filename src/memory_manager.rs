use anyhow::Result;
use std::path::PathBuf;

use crate::models::MemoryBundle;
use crate::utils::files::{append_string, ensure_dir, read_string_if_exists, write_string};

const PROMPT_STORY_MEMORY_CHAR_LIMIT: usize = 18_000;
const PROMPT_SCENE_SECTION_LIMIT: usize = 700;
const PROMPT_CHAPTER_SECTION_LIMIT: usize = 500;
const PROMPT_REWRITE_SECTION_LIMIT: usize = 350;
const PROMPT_WORLD_SECTION_LIMIT: usize = 1_800;
const PROMPT_OTHER_SECTION_LIMIT: usize = 700;
const PROMPT_SCENE_SECTION_KEEP: usize = 10;
const PROMPT_CHAPTER_SECTION_KEEP: usize = 6;
const PROMPT_REWRITE_SECTION_KEEP: usize = 3;
const PROMPT_WORLD_SECTION_KEEP: usize = 4;
const PROMPT_OTHER_SECTION_KEEP: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoryMemorySectionKind {
    Scene,
    Chapter,
    Rewrite,
    WorldExpansion,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoryMemorySection {
    heading: String,
    body: String,
}

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
                "# Core Memory\n\n- Genre: TBD\n- Tone: Focused, cinematic, character-driven\n- Long-term promise: A coherent serialized novel\n",
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

    pub fn load_prompt_bundle(&self) -> Result<MemoryBundle> {
        let bundle = self.load_bundle()?;
        Ok(MemoryBundle {
            core_memory: bundle.core_memory,
            story_memory: compact_story_memory_for_prompt(&bundle.story_memory),
            active_memory: bundle.active_memory,
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

fn compact_story_memory_for_prompt(story_memory: &str) -> String {
    if story_memory.len() <= PROMPT_STORY_MEMORY_CHAR_LIMIT {
        return story_memory.to_string();
    }

    let (header, sections) = parse_story_memory_sections(story_memory);
    if sections.is_empty() {
        return truncate_chars(story_memory, PROMPT_STORY_MEMORY_CHAR_LIMIT);
    }

    let selected_indices = select_story_memory_sections(&sections);
    if selected_indices.is_empty() {
        return truncate_chars(story_memory, PROMPT_STORY_MEMORY_CHAR_LIMIT);
    }

    let header = if header.trim().is_empty() {
        "# Story Memory".to_string()
    } else {
        header.trim_end().to_string()
    };
    let note =
        "> Prompt view: retained recent high-signal sections to fit context budget. Full story memory remains on disk.";
    let base = format!("{header}\n\n{note}\n");
    let mut rendered_sections = selected_indices
        .into_iter()
        .map(|index| (index, render_prompt_section(&sections[index])))
        .collect::<Vec<_>>();

    let mut kept = Vec::new();
    let mut total_len = base.len() + 1;
    while let Some((index, rendered)) = rendered_sections.pop() {
        let rendered_len = rendered.len() + 2;
        if kept.is_empty() || total_len + rendered_len <= PROMPT_STORY_MEMORY_CHAR_LIMIT {
            total_len += rendered_len;
            kept.push((index, rendered));
        }
    }

    if kept.is_empty() {
        return truncate_chars(story_memory, PROMPT_STORY_MEMORY_CHAR_LIMIT);
    }

    kept.sort_by_key(|(index, _)| *index);
    let body = kept
        .into_iter()
        .map(|(_, rendered)| rendered)
        .collect::<Vec<_>>()
        .join("\n\n");

    format!("{base}\n{body}\n")
}

fn parse_story_memory_sections(story_memory: &str) -> (String, Vec<StoryMemorySection>) {
    let mut header_lines = Vec::new();
    let mut sections = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_body = Vec::new();

    for line in story_memory.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some(previous_heading) = current_heading.take() {
                sections.push(StoryMemorySection {
                    heading: previous_heading,
                    body: current_body.join("\n").trim_end().to_string(),
                });
                current_body.clear();
            }
            current_heading = Some(heading.trim().to_string());
            continue;
        }

        if current_heading.is_some() {
            current_body.push(line.to_string());
        } else {
            header_lines.push(line.to_string());
        }
    }

    if let Some(heading) = current_heading {
        sections.push(StoryMemorySection {
            heading,
            body: current_body.join("\n").trim_end().to_string(),
        });
    }

    (header_lines.join("\n"), sections)
}

fn select_story_memory_sections(sections: &[StoryMemorySection]) -> Vec<usize> {
    let mut scene_count = 0usize;
    let mut chapter_count = 0usize;
    let mut rewrite_count = 0usize;
    let mut world_count = 0usize;
    let mut other_count = 0usize;
    let mut selected = Vec::new();

    for (index, section) in sections.iter().enumerate().rev() {
        let keep = match section.kind() {
            StoryMemorySectionKind::Scene if scene_count < PROMPT_SCENE_SECTION_KEEP => {
                scene_count += 1;
                true
            }
            StoryMemorySectionKind::Chapter if chapter_count < PROMPT_CHAPTER_SECTION_KEEP => {
                chapter_count += 1;
                true
            }
            StoryMemorySectionKind::Rewrite if rewrite_count < PROMPT_REWRITE_SECTION_KEEP => {
                rewrite_count += 1;
                true
            }
            StoryMemorySectionKind::WorldExpansion if world_count < PROMPT_WORLD_SECTION_KEEP => {
                world_count += 1;
                true
            }
            StoryMemorySectionKind::Other if other_count < PROMPT_OTHER_SECTION_KEEP => {
                other_count += 1;
                true
            }
            _ => false,
        };

        if keep {
            selected.push(index);
        }
    }

    selected.sort_unstable();
    selected
}

fn render_prompt_section(section: &StoryMemorySection) -> String {
    let mut rendered = format!("## {}", section.heading);
    if section.body.trim().is_empty() {
        return rendered;
    }

    rendered.push('\n');
    rendered.push_str(section.body.trim());
    if rendered.len() <= section_char_limit(section.kind()) {
        return rendered;
    }

    let heading = format!("## {}\n", section.heading);
    let note = "\n- Note: older details omitted for prompt budget.";
    let body_budget = section_char_limit(section.kind())
        .saturating_sub(heading.len())
        .saturating_sub(note.len());
    let truncated_body = truncate_chars(section.body.trim(), body_budget);
    format!("{heading}{truncated_body}{note}")
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut end = 0usize;
    for (count, (index, ch)) in value.char_indices().enumerate() {
        if count == max_chars {
            break;
        }
        end = index + ch.len_utf8();
    }

    let mut truncated = value[..end].trim_end().to_string();
    if let Some(last_whitespace) = truncated.rfind(char::is_whitespace) {
        if last_whitespace > truncated.len() / 2 {
            truncated.truncate(last_whitespace);
        }
    }
    truncated.trim_end().to_string()
}

fn section_char_limit(kind: StoryMemorySectionKind) -> usize {
    match kind {
        StoryMemorySectionKind::Scene => PROMPT_SCENE_SECTION_LIMIT,
        StoryMemorySectionKind::Chapter => PROMPT_CHAPTER_SECTION_LIMIT,
        StoryMemorySectionKind::Rewrite => PROMPT_REWRITE_SECTION_LIMIT,
        StoryMemorySectionKind::WorldExpansion => PROMPT_WORLD_SECTION_LIMIT,
        StoryMemorySectionKind::Other => PROMPT_OTHER_SECTION_LIMIT,
    }
}

impl StoryMemorySection {
    fn kind(&self) -> StoryMemorySectionKind {
        if self.heading.starts_with("Scene ") {
            StoryMemorySectionKind::Scene
        } else if self.heading.starts_with("Chapter ") {
            StoryMemorySectionKind::Chapter
        } else if self.heading.starts_with("Rewrite ") {
            StoryMemorySectionKind::Rewrite
        } else if self.heading.starts_with("World Expansion") {
            StoryMemorySectionKind::WorldExpansion
        } else {
            StoryMemorySectionKind::Other
        }
    }
}

#[cfg(test)]
mod tests {
    use super::compact_story_memory_for_prompt;

    #[test]
    fn keeps_story_memory_untouched_when_under_budget() {
        let story =
            "# Story Memory\n\n## Scene scene_001_001: Securing the Lead\n- Goal: Move forward\n";
        assert_eq!(compact_story_memory_for_prompt(story), story);
    }

    #[test]
    fn compacts_large_story_memory_but_keeps_recent_high_signal_sections() {
        let mut story = String::from("# Story Memory\n\n");
        story.push_str("## World Expansion\n");
        story.push_str(&"Ancient guild history. ".repeat(300));
        story.push_str("\n\n");
        for index in 1..=14 {
            story.push_str(&format!(
                "## Scene scene_001_{index:03}: Scene {index}\n- Goal: {}\n- Conflict: {}\n- Outcome: {}\n\n",
                "Move the investigation forward. ".repeat(12),
                "Pressure from the city archive. ".repeat(10),
                "A lead surfaces, but the cost rises. ".repeat(10),
            ));
        }
        story.push_str("## Chapter 001: First Descent\n");
        story.push_str(&"Compiled summary. ".repeat(50));
        story.push_str("\n\n## Rewrite scene_001_014\n");
        story.push_str(&"Instruction log. ".repeat(40));
        story.push('\n');

        let compacted = compact_story_memory_for_prompt(&story);

        assert!(compacted.len() < story.len());
        assert!(compacted.contains("Prompt view: retained recent high-signal sections"));
        assert!(compacted.contains("## World Expansion"));
        assert!(compacted.contains("## Scene scene_001_014: Scene 14"));
        assert!(!compacted.contains("## Scene scene_001_001: Scene 1"));
        assert!(compacted.len() <= super::PROMPT_STORY_MEMORY_CHAR_LIMIT + 256);
    }
}
