use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const BRIEF_SECTION_LIMIT: usize = 2_400;
const BIBLE_SECTION_LIMIT: usize = 4_800;
const PLOT_SECTION_LIMIT: usize = 3_200;
const RESEARCH_SECTION_LIMIT: usize = 1_600;
const VOICE_SECTION_LIMIT: usize = 2_400;
const CHARACTER_VOICE_GUIDE_LIMIT: usize = 2_400;
const NARRATIVE_STYLE_GUIDE_LIMIT: usize = 2_400;
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
    pub voice_docs: usize,
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
            + self.voice_docs
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
    let voice_dir = workspace_dir.join("03_StoryBible").join("Voice");
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
    let voice_docs = collect_story_docs(&voice_dir)?;

    let mut research_docs = collect_story_docs(&research_sources_dir)?;
    research_docs.extend(collect_story_docs(&research_notes_dir)?);
    research_docs.extend(collect_story_docs(&research_refs_dir)?);
    research_docs.sort();

    let status = StoryFoundationStatus {
        brief_docs: brief_docs.len(),
        character_docs: character_docs.len(),
        world_docs: world_docs.len(),
        plot_docs: plot_docs.len(),
        voice_docs: voice_docs.len(),
        research_docs: research_docs.len(),
        score: foundation_score(
            brief_docs.len(),
            character_docs.len(),
            world_docs.len(),
            plot_docs.len(),
            voice_docs.len(),
            research_docs.len(),
        ),
        missing_items: missing_items(
            brief_docs.len(),
            character_docs.len(),
            world_docs.len(),
            plot_docs.len(),
            voice_docs.len(),
        ),
    };

    let prompt_context = render_prompt_context(
        workspace_dir,
        &status,
        &brief_docs,
        &character_docs,
        &world_docs,
        &plot_docs,
        &voice_docs,
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
    voice_docs: usize,
    research_docs: usize,
) -> u32 {
    let mut score = 0;

    if brief_docs > 0 {
        score += 20;
    }
    if plot_docs > 0 {
        score += 25;
    }
    if character_docs > 0 {
        score += 20;
    }
    if world_docs > 0 {
        score += 15;
    }
    if voice_docs > 0 {
        score += 10;
    }
    if research_docs > 0 {
        score += 5;
    }
    if brief_docs + character_docs + world_docs + plot_docs + voice_docs >= 4 {
        score += 5;
    }

    score.min(100)
}

fn missing_items(
    brief_docs: usize,
    character_docs: usize,
    world_docs: usize,
    plot_docs: usize,
    voice_docs: usize,
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
    if voice_docs == 0 {
        missing.push("style/tone/voice guide in 03_StoryBible/Voice".to_string());
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
    voice_docs: &[PathBuf],
    research_docs: &[PathBuf],
) -> Result<String> {
    let missing = if status.missing_items.is_empty() {
        "None".to_string()
    } else {
        status.missing_items.join(" | ")
    };
    let character_voice_guide = render_character_voice_guide(workspace_dir, character_docs)?;
    let narrative_style_guide = render_narrative_style_guide(workspace_dir, voice_docs)?;

    Ok(format!(
        "Story foundation score: {score}/100 ({level})\nMissing foundation items: {missing}\n\n\
Project brief excerpts:\n{brief}\n\n\
Narrative style guide:\n{style}\n\n\
Character voice guide:\n{voice}\n\n\
Character bible excerpts:\n{characters}\n\n\
World and rule excerpts:\n{world}\n\n\
Plot outline excerpts:\n{plot}\n\n\
Voice and tone excerpts:\n{voice_docs}\n\n\
Research excerpts:\n{research}",
        score = status.score,
        level = status.level(),
        missing = missing,
        brief = render_section(workspace_dir, brief_docs, BRIEF_SECTION_LIMIT)?,
        style = narrative_style_guide,
        voice = character_voice_guide,
        characters = render_section(workspace_dir, character_docs, BIBLE_SECTION_LIMIT)?,
        world = render_section(workspace_dir, world_docs, BIBLE_SECTION_LIMIT)?,
        plot = render_section(workspace_dir, plot_docs, PLOT_SECTION_LIMIT)?,
        voice_docs = render_section(workspace_dir, voice_docs, VOICE_SECTION_LIMIT)?,
        research = render_section(workspace_dir, research_docs, RESEARCH_SECTION_LIMIT)?,
    ))
}

fn render_narrative_style_guide(workspace_dir: &Path, docs: &[PathBuf]) -> Result<String> {
    if docs.is_empty() {
        return Ok("No project-level style guide provided yet.".to_string());
    }

    let mut rendered = String::new();
    for path in docs {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read story foundation file {}", path.display()))?;
        let profile = build_narrative_style_profile(&content);
        if profile.is_empty() {
            continue;
        }

        let relative = path
            .strip_prefix(workspace_dir)
            .unwrap_or(path)
            .display()
            .to_string();
        let mut candidate = format!("### {relative}\n");
        for line in profile {
            candidate.push_str("- ");
            candidate.push_str(&line);
            candidate.push('\n');
        }
        candidate.push('\n');

        if rendered.len() + candidate.len() > NARRATIVE_STYLE_GUIDE_LIMIT {
            break;
        }

        rendered.push_str(&candidate);
    }

    if rendered.trim().is_empty() {
        Ok("No explicit style/tone/voice sections found yet.".to_string())
    } else {
        Ok(rendered.trim().to_string())
    }
}

fn build_narrative_style_profile(content: &str) -> Vec<String> {
    let sections = extract_markdown_sections(content);
    let mut profile = Vec::new();

    push_profile_line(
        &mut profile,
        "Style principles",
        first_section(&sections, &["Style Principles", "Style Guide", "Style"]),
    );
    push_profile_line(
        &mut profile,
        "Tone targets",
        first_section(&sections, &["Tone Targets", "Tone", "Mood"]),
    );
    push_profile_line(
        &mut profile,
        "Genre style",
        first_section(&sections, &["Genre Style", "Genre Expectations"]),
    );
    push_profile_line(
        &mut profile,
        "Narrative voice",
        first_section(&sections, &["Narrative Voice", "Voice"]),
    );
    push_profile_line(
        &mut profile,
        "Dialogue mode",
        first_section(&sections, &["Dialogue Mode", "Dialogue Guidance"]),
    );
    push_profile_line(
        &mut profile,
        "Avoid",
        first_section(&sections, &["Avoid", "Do Not Do", "Restrictions"]),
    );
    push_profile_line(
        &mut profile,
        "Safe style note",
        first_section(&sections, &["Safe Style Note", "Safety Note"]),
    );

    profile
}

fn render_character_voice_guide(workspace_dir: &Path, docs: &[PathBuf]) -> Result<String> {
    if docs.is_empty() {
        return Ok("No character voice profiles provided yet.".to_string());
    }

    let mut rendered = String::new();
    for path in docs {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read story foundation file {}", path.display()))?;
        let profile = build_character_voice_profile(&content);
        if profile.is_empty() {
            continue;
        }

        let relative = path
            .strip_prefix(workspace_dir)
            .unwrap_or(path)
            .display()
            .to_string();
        let mut candidate = format!("### {relative}\n");
        for line in profile {
            candidate.push_str("- ");
            candidate.push_str(&line);
            candidate.push('\n');
        }
        candidate.push('\n');

        if rendered.len() + candidate.len() > CHARACTER_VOICE_GUIDE_LIMIT {
            break;
        }

        rendered.push_str(&candidate);
    }

    if rendered.trim().is_empty() {
        Ok("No explicit character voice sections found yet.".to_string())
    } else {
        Ok(rendered.trim().to_string())
    }
}

fn build_character_voice_profile(content: &str) -> Vec<String> {
    let sections = extract_markdown_sections(content);
    let mut profile = Vec::new();

    push_profile_line(
        &mut profile,
        "Character",
        first_section(&sections, &["Character ID", "Name", "Character"]),
    );
    push_profile_line(
        &mut profile,
        "Role",
        first_section(&sections, &["Role In Story", "Role"]),
    );
    push_profile_line(
        &mut profile,
        "Voice",
        first_section(&sections, &["Voice Notes", "Voice", "Voice Summary"]),
    );
    push_profile_line(
        &mut profile,
        "Speech rhythm",
        first_section(&sections, &["Speech Rhythm", "Dialogue Rhythm"]),
    );
    push_profile_line(
        &mut profile,
        "Preferred diction",
        first_section(
            &sections,
            &["Favorite Diction", "Preferred Diction", "Signature Words"],
        ),
    );
    push_profile_line(
        &mut profile,
        "Taboo phrases",
        first_section(&sections, &["Taboo Phrases", "Avoid Saying"]),
    );
    push_profile_line(
        &mut profile,
        "Emotional leakage",
        first_section(&sections, &["Emotional Leakage", "Emotion Under Stress"]),
    );
    push_profile_line(
        &mut profile,
        "Invariants",
        first_section(&sections, &["Non-Negotiable Invariants", "Invariants"]),
    );

    profile
}

fn extract_markdown_sections(content: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_body = String::new();

    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some(heading) = current_heading.take() {
                sections.push((heading, current_body.trim().to_string()));
                current_body.clear();
            }
            current_heading = Some(heading.trim().to_string());
            continue;
        }

        if current_heading.is_some() {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    if let Some(heading) = current_heading {
        sections.push((heading, current_body.trim().to_string()));
    }

    sections
}

fn first_section<'a>(sections: &'a [(String, String)], names: &[&str]) -> Option<&'a str> {
    for name in names {
        if let Some((_, value)) = sections
            .iter()
            .find(|(heading, value)| heading.eq_ignore_ascii_case(name) && !value.trim().is_empty())
        {
            return Some(value.as_str());
        }
    }

    None
}

fn push_profile_line(profile: &mut Vec<String>, label: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };

    let compact = compact_markdown_value(value, 220);
    if compact.is_empty() {
        return;
    }

    profile.push(format!("{label}: {compact}"));
}

fn compact_markdown_value(value: &str, max_chars: usize) -> String {
    let compact = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_start_matches("- ").trim())
        .collect::<Vec<_>>()
        .join(" | ");

    truncate_chars(compact.trim(), max_chars)
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

    #[test]
    fn character_voice_guide_extracts_voice_sections() -> Result<()> {
        let temp_dir = tempdir()?;
        let workspace = temp_dir.path();
        fs::create_dir_all(workspace.join("03_StoryBible/Characters"))?;
        fs::write(
            workspace.join("03_StoryBible/Characters/Lead.md"),
            "# Character\n\n## Character ID\nSeorin\n\n## Role In Story\nProtagonist\n\n## Voice Notes\nShort, precise, and impatient.\n\n## Speech Rhythm\nMostly clipped sentences.\n\n## Taboo Phrases\nNever begs. Never speaks in slogans.\n\n## Non-Negotiable Invariants\nWill not bluff about facts she has not verified.\n",
        )?;

        let bundle = load_story_foundation(workspace)?;

        assert!(bundle.prompt_context.contains("Character voice guide:"));
        assert!(bundle
            .prompt_context
            .contains("Voice: Short, precise, and impatient."));
        assert!(bundle
            .prompt_context
            .contains("Speech rhythm: Mostly clipped sentences."));
        assert!(bundle
            .prompt_context
            .contains("Taboo phrases: Never begs. Never speaks in slogans."));

        Ok(())
    }

    #[test]
    fn narrative_style_guide_extracts_safe_style_sections() -> Result<()> {
        let temp_dir = tempdir()?;
        let workspace = temp_dir.path();
        fs::create_dir_all(workspace.join("03_StoryBible/Voice"))?;
        fs::write(
            workspace.join("03_StoryBible/Voice/Style Guide.md"),
            "# Style Guide\n\n## Style Principles\n짧고 가독성 높은 문장. 감정을 직접 설명하지 않는다.\n\n## Tone Targets\n건조하지만 긴장감 있게 유지한다.\n\n## Genre Style\n현대 미스터리 스릴러의 빠른 전개.\n\n## Safe Style Note\n작가 이름 대신 문체 특징과 장르 기대치를 사용한다.\n",
        )?;

        let bundle = load_story_foundation(workspace)?;

        assert_eq!(bundle.status.voice_docs, 1);
        assert!(bundle
            .status
            .missing_items
            .iter()
            .any(|item| item.contains("project brief")));
        assert!(bundle.prompt_context.contains("Narrative style guide:"));
        assert!(bundle
            .prompt_context
            .contains("Style principles: 짧고 가독성 높은 문장."));
        assert!(bundle
            .prompt_context
            .contains("Tone targets: 건조하지만 긴장감 있게 유지한다."));
        assert!(bundle
            .prompt_context
            .contains("Genre style: 현대 미스터리 스릴러의 빠른 전개."));
        assert!(bundle
            .prompt_context
            .contains("Safe style note: 작가 이름 대신 문체 특징과 장르 기대치를 사용한다."));

        Ok(())
    }
}
