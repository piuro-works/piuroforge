use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const BRIEF_SECTION_LIMIT: usize = 2_400;
const BIBLE_SECTION_LIMIT: usize = 4_800;
const PLOT_SECTION_LIMIT: usize = 3_200;
const RESEARCH_SECTION_LIMIT: usize = 1_600;
const FILE_EXCERPT_LIMIT: usize = 1_200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryFoundationBundle {
    pub prompt_context: String,
    pub status: StoryFoundationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryFoundationStatus {
    pub brief_docs: usize,
    pub character_docs: usize,
    pub world_docs: usize,
    pub plot_docs: usize,
    pub research_docs: usize,
    pub score: u32,
    pub missing_items: Vec<String>,
}

impl StoryFoundationStatus {
    pub fn level(&self) -> &'static str {
        match self.score {
            80..=100 => "robust",
            60..=79 => "workable",
            40..=59 => "thin",
            _ => "skeletal",
        }
    }

    pub fn total_docs(&self) -> usize {
        self.brief_docs
            + self.character_docs
            + self.world_docs
            + self.plot_docs
            + self.research_docs
    }
}

pub fn load_story_foundation(workspace_dir: &Path) -> Result<StoryFoundationBundle> {
    let brief_dir = workspace_dir.join("01_Brief");
    let characters_dir = workspace_dir.join("03_StoryBible").join("Characters");
    let world_dir = workspace_dir.join("03_StoryBible").join("World");
    let rules_dir = workspace_dir.join("03_StoryBible").join("Rules");
    let timeline_dir = workspace_dir.join("03_StoryBible").join("Timeline");
    let plot_dir = workspace_dir.join("03_StoryBible").join("Plot");
    let research_sources_dir = workspace_dir.join("04_Research").join("Sources");
    let research_notes_dir = workspace_dir.join("04_Research").join("Notes");
    let research_refs_dir = workspace_dir.join("04_Research").join("References");

    let brief_docs = collect_story_docs(&brief_dir)?;
    let character_docs = collect_story_docs(&characters_dir)?;
    let mut world_docs = collect_story_docs(&world_dir)?;
    world_docs.extend(collect_story_docs(&rules_dir)?);
    world_docs.extend(collect_story_docs(&timeline_dir)?);
    world_docs.sort();

    let plot_docs = collect_story_docs(&plot_dir)?;

    let mut research_docs = collect_story_docs(&research_sources_dir)?;
    research_docs.extend(collect_story_docs(&research_notes_dir)?);
    research_docs.extend(collect_story_docs(&research_refs_dir)?);
    research_docs.sort();

    let status = StoryFoundationStatus {
        brief_docs: brief_docs.len(),
        character_docs: character_docs.len(),
        world_docs: world_docs.len(),
        plot_docs: plot_docs.len(),
        research_docs: research_docs.len(),
        score: foundation_score(
            brief_docs.len(),
            character_docs.len(),
            world_docs.len(),
            plot_docs.len(),
            research_docs.len(),
        ),
        missing_items: missing_items(
            brief_docs.len(),
            character_docs.len(),
            world_docs.len(),
            plot_docs.len(),
        ),
    };

    let prompt_context = render_prompt_context(
        workspace_dir,
        &status,
        &brief_docs,
        &character_docs,
        &world_docs,
        &plot_docs,
        &research_docs,
    )?;

    Ok(StoryFoundationBundle {
        prompt_context,
        status,
    })
}

fn foundation_score(
    brief_docs: usize,
    character_docs: usize,
    world_docs: usize,
    plot_docs: usize,
    research_docs: usize,
) -> u32 {
    let mut score = 0;

    if brief_docs > 0 {
        score += 25;
    }
    if plot_docs > 0 {
        score += 30;
    }
    if character_docs > 0 {
        score += 20;
    }
    if world_docs > 0 {
        score += 15;
    }
    if research_docs > 0 {
        score += 5;
    }
    if brief_docs + character_docs + world_docs + plot_docs >= 4 {
        score += 5;
    }

    score.min(100)
}

fn missing_items(
    brief_docs: usize,
    character_docs: usize,
    world_docs: usize,
    plot_docs: usize,
) -> Vec<String> {
    let mut missing = Vec::new();

    if brief_docs == 0 {
        missing.push("project brief in 01_Brief".to_string());
    }
    if plot_docs == 0 {
        missing.push("plot outline in 03_StoryBible/Plot".to_string());
    }
    if character_docs == 0 {
        missing.push("character notes in 03_StoryBible/Characters".to_string());
    }
    if world_docs == 0 {
        missing.push("world/rules/timeline notes in 03_StoryBible".to_string());
    }

    missing
}

fn render_prompt_context(
    workspace_dir: &Path,
    status: &StoryFoundationStatus,
    brief_docs: &[PathBuf],
    character_docs: &[PathBuf],
    world_docs: &[PathBuf],
    plot_docs: &[PathBuf],
    research_docs: &[PathBuf],
) -> Result<String> {
    let missing = if status.missing_items.is_empty() {
        "None".to_string()
    } else {
        status.missing_items.join(" | ")
    };

    Ok(format!(
        "Story foundation score: {score}/100 ({level})\nMissing foundation items: {missing}\n\n\
Project brief excerpts:\n{brief}\n\n\
Character bible excerpts:\n{characters}\n\n\
World and rule excerpts:\n{world}\n\n\
Plot outline excerpts:\n{plot}\n\n\
Research excerpts:\n{research}",
        score = status.score,
        level = status.level(),
        missing = missing,
        brief = render_section(workspace_dir, brief_docs, BRIEF_SECTION_LIMIT)?,
        characters = render_section(workspace_dir, character_docs, BIBLE_SECTION_LIMIT)?,
        world = render_section(workspace_dir, world_docs, BIBLE_SECTION_LIMIT)?,
        plot = render_section(workspace_dir, plot_docs, PLOT_SECTION_LIMIT)?,
        research = render_section(workspace_dir, research_docs, RESEARCH_SECTION_LIMIT)?,
    ))
}

fn render_section(workspace_dir: &Path, docs: &[PathBuf], max_chars: usize) -> Result<String> {
    if docs.is_empty() {
        return Ok("None provided.".to_string());
    }

    let mut rendered = String::new();
    for path in docs {
        let excerpt = read_excerpt(path, FILE_EXCERPT_LIMIT)?;
        if excerpt.trim().is_empty() {
            continue;
        }

        let relative = path
            .strip_prefix(workspace_dir)
            .unwrap_or(path)
            .display()
            .to_string();
        let candidate = format!("### {relative}\n{excerpt}\n\n");

        if rendered.len() + candidate.len() > max_chars {
            break;
        }

        rendered.push_str(&candidate);
    }

    if rendered.trim().is_empty() {
        Ok("None provided.".to_string())
    } else {
        Ok(rendered.trim().to_string())
    }
}

fn read_excerpt(path: &Path, max_chars: usize) -> Result<String> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read story foundation file {}", path.display()))?;
    Ok(truncate_chars(content.trim(), max_chars))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut rendered = String::new();
    for ch in value.chars().take(max_chars) {
        rendered.push(ch);
    }

    if value.chars().count() <= max_chars {
        rendered
    } else {
        format!("{}...", rendered.trim_end())
    }
}

fn collect_story_docs(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut docs = Vec::new();
    collect_story_docs_recursive(dir, &mut docs)?;
    docs.sort();
    Ok(docs)
}

fn collect_story_docs_recursive(dir: &Path, docs: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)
        .with_context(|| format!("failed to read story directory {}", dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to inspect {}", dir.display()))?;
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();

        if file_name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_story_docs_recursive(&path, docs)?;
            continue;
        }

        if !is_story_doc(&path) {
            continue;
        }

        docs.push(path);
    }

    Ok(())
}

fn is_story_doc(path: &Path) -> bool {
    let extension = path.extension().and_then(|value| value.to_str());
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");

    matches!(extension, Some("md") | Some("txt"))
        && !file_name.eq_ignore_ascii_case("README.md")
        && !file_name.contains("Template")
}

#[cfg(test)]
mod tests {
    use super::load_story_foundation;
    use anyhow::Result;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn ignores_scaffold_readmes_and_counts_real_docs() -> Result<()> {
        let temp_dir = tempdir()?;
        let workspace = temp_dir.path();
        fs::create_dir_all(workspace.join("01_Brief"))?;
        fs::create_dir_all(workspace.join("03_StoryBible/Characters"))?;
        fs::create_dir_all(workspace.join("03_StoryBible/Plot"))?;
        fs::write(workspace.join("01_Brief/README.md"), "# Readme")?;
        fs::write(
            workspace.join("01_Brief/Story Brief.md"),
            "A brief about the city and the case.",
        )?;
        fs::write(
            workspace.join("03_StoryBible/Characters/Lead.md"),
            "Lead character dossier.",
        )?;
        fs::write(
            workspace.join("03_StoryBible/Plot/Arc One.md"),
            "Arc one outline.",
        )?;

        let bundle = load_story_foundation(workspace)?;

        assert_eq!(bundle.status.brief_docs, 1);
        assert_eq!(bundle.status.character_docs, 1);
        assert_eq!(bundle.status.plot_docs, 1);
        assert!(bundle.prompt_context.contains("Story Brief.md"));
        assert!(!bundle.prompt_context.contains("README.md"));

        Ok(())
    }

    #[test]
    fn reports_missing_foundation_items_when_workspace_is_thin() -> Result<()> {
        let temp_dir = tempdir()?;
        let workspace = temp_dir.path();
        fs::create_dir_all(workspace.join("01_Brief"))?;

        let bundle = load_story_foundation(workspace)?;

        assert!(bundle.status.score < 40);
        assert!(bundle
            .status
            .missing_items
            .iter()
            .any(|item| item.contains("plot outline")));

        Ok(())
    }
}
