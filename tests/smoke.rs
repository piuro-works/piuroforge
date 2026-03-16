use anyhow::Result;
use heeforge::models::Scene;
use heeforge::utils::markdown::render_scene;
use heeforge::{Config, NovelEngine};
use tempfile::tempdir;

#[test]
fn init_project_creates_workspace_scaffold() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let config =
        Config::with_global_config_dir(workspace.clone(), temp_dir.path().join("config-home"))?;
    let engine = NovelEngine::new(config)?;

    engine.init_project()?;

    assert!(temp_dir.path().join("config-home/config.toml").exists());
    let global_config = std::fs::read_to_string(temp_dir.path().join("config-home/config.toml"))?;
    assert!(global_config.contains("# HeeForge global settings"));
    assert!(global_config.contains("codex login"));
    assert!(global_config.contains("codex_command = \"codex\""));
    assert!(global_config.contains("allow_dummy_fallback = false"));
    assert!(global_config.contains("log_prompts = false"));
    assert!(global_config.contains("workspace_auto_commit = false"));
    assert!(workspace.join(".novel/workspace.json").exists());
    assert!(workspace.join("novel.toml").exists());
    assert!(workspace.join("README.md").exists());
    assert!(workspace.join("02_Draft/Scenes").exists());
    assert!(workspace.join("02_Draft/Chapters").exists());
    assert!(workspace.join("03_StoryBible/Characters").exists());
    assert!(workspace.join("04_Research/Sources").exists());
    assert!(workspace.join("06_Review/Feedback").exists());
    assert!(workspace.join("06_Review/Revisions").exists());
    let workspace_config = std::fs::read_to_string(workspace.join("novel.toml"))?;
    assert!(workspace_config.contains("title = \"Demo Novel\""));
    let workspace_readme = std::fs::read_to_string(workspace.join("README.md"))?;
    assert!(workspace_readme.contains("This workspace separates human-facing manuscript files"));
    assert!(workspace_readme.contains("First Run Checklist"));
    assert!(workspace_readme.contains("allow_dummy_fallback = false"));
    let draft_readme = std::fs::read_to_string(workspace.join("02_Draft/README.md"))?;
    assert!(draft_readme.contains("Human-facing manuscript work lives here."));
    let template = std::fs::read_to_string(workspace.join("98_Templates/Scene Template.md"))?;
    assert!(template.contains("## Objective"));
    let review_template =
        std::fs::read_to_string(workspace.join("98_Templates/Review Pass Template.md"))?;
    assert!(review_template.contains("## Findings"));
    assert!(workspace.join(".novel/state/project_state.json").exists());
    assert!(workspace.join(".novel/memory/core_memory.md").exists());
    assert!(workspace.join(".novel/memory/story_memory.md").exists());
    assert!(workspace.join(".novel/memory/active_memory.md").exists());
    let gitignore = std::fs::read_to_string(workspace.join(".gitignore"))?;
    assert!(gitignore.contains("/.novel/state/"));
    assert!(gitignore.contains("/.novel/memory/active_memory.md"));

    Ok(())
}

#[test]
fn generate_next_scene_saves_dummy_scene_markdown() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let engine = test_engine(workspace.clone(), temp_dir.path().join("config-home"))?;

    engine.init_project()?;
    let scene = engine.generate_next_scene()?.value;

    assert_eq!(scene.id, "scene_001_001");
    let scene_path = scene_file_path(&workspace, "scene_001_001")?;
    assert!(scene_path.exists());
    assert_eq!(
        scene_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
        "scene_001_001-securing-the-lead.md"
    );

    let saved = std::fs::read_to_string(scene_path)?;
    assert!(saved.contains("# Scene scene_001_001"));
    assert!(saved.contains("## Short Title\nSecuring the Lead"));
    let reloaded = engine.show_scene("scene_001_001")?;
    assert_eq!(reloaded.status, "draft");
    assert!(saved.contains("## Text"));
    let log =
        std::fs::read_to_string(workspace.join(".novel/logs/scene_generation/scene_001_001.json"))?;
    assert!(log.contains("\"scene_id\": \"scene_001_001\""));

    Ok(())
}

#[test]
fn generate_next_scene_requires_initial_novel_metadata() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let config = Config::with_global_config_dir(workspace, global_dir)?;
    let engine = NovelEngine::new(config)?;

    engine.init_project()?;
    let error = engine
        .generate_next_scene()
        .expect_err("expected missing config validation");
    assert!(error.to_string().contains("missing required novel config"));
    assert!(error.to_string().contains("premise"));
    assert!(error.to_string().contains("protagonist_name"));

    Ok(())
}

#[test]
fn generate_next_chapter_saves_slugged_markdown() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let engine = test_engine(workspace.clone(), temp_dir.path().join("config-home"))?;

    engine.init_project()?;
    engine.generate_next_scene()?;
    let chapter_path = engine.generate_next_chapter()?;

    assert_eq!(
        chapter_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
        "chapter_001-securing-the-lead.md"
    );

    let content = std::fs::read_to_string(&chapter_path)?;
    assert!(content.contains("# Chapter 001"));
    assert!(content.contains("## Short Title\nSecuring the Lead"));
    assert!(!content.contains("Status: draft\ndraft"));

    let state = engine.get_status()?;
    assert_eq!(state.current_chapter, 2);
    assert_eq!(state.current_scene, 0);
    assert_eq!(state.stage, "chapter_ready");

    Ok(())
}

#[test]
fn review_and_rewrite_persist_artifacts() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let engine = test_engine(workspace.clone(), temp_dir.path().join("config-home"))?;

    engine.init_project()?;
    let original = engine.generate_next_scene()?.value;

    let issues = engine.review_current_scene()?;
    assert!(!issues.value.is_empty());
    let report = std::fs::read_to_string(workspace.join("06_Review/Feedback/scene_001_001.json"))?;
    assert!(report.contains("\"scene_id\": \"scene_001_001\""));
    assert!(report.contains("\"critic_fallback_warning\""));

    let rewritten = engine
        .rewrite_scene("scene_001_001", "Make it darker and sharper")?
        .value;
    assert_eq!(rewritten.id, "scene_001_001");
    let history_dir = workspace.join("06_Review/Revisions/scene_001_001");
    assert!(history_dir.join("rewrite_001_original.md").exists());
    assert!(history_dir.join("rewrite_001_rewritten.md").exists());
    assert!(history_dir.join("rewrite_001.json").exists());

    let original_snapshot = std::fs::read_to_string(history_dir.join("rewrite_001_original.md"))?;
    let rewritten_snapshot = std::fs::read_to_string(history_dir.join("rewrite_001_rewritten.md"))?;
    let record = std::fs::read_to_string(history_dir.join("rewrite_001.json"))?;
    assert!(original_snapshot.contains(original.text.lines().next().unwrap_or_default()));
    assert!(rewritten_snapshot.contains("The revision now leans harder into"));
    assert!(record.contains(
        "\"original_snapshot_path\": \"06_Review/Revisions/scene_001_001/rewrite_001_original.md\""
    ));
    assert!(record.contains("\"rewritten_snapshot_path\": \"06_Review/Revisions/scene_001_001/rewrite_001_rewritten.md\""));

    Ok(())
}

#[test]
fn next_chapter_rejects_gapped_scene_sequence() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let engine = test_engine(workspace.clone(), temp_dir.path().join("config-home"))?;

    engine.init_project()?;
    write_scene(
        &workspace,
        Scene {
            id: "scene_001_001".to_string(),
            chapter: 1,
            scene_number: 1,
            short_title: "Goal One".to_string(),
            goal: "Goal one".to_string(),
            conflict: "Conflict one".to_string(),
            outcome: "Outcome one".to_string(),
            text: "Scene one text.".to_string(),
            status: "draft".to_string(),
        },
    )?;
    write_scene(
        &workspace,
        Scene {
            id: "scene_001_003".to_string(),
            chapter: 1,
            scene_number: 3,
            short_title: "Goal Three".to_string(),
            goal: "Goal three".to_string(),
            conflict: "Conflict three".to_string(),
            outcome: "Outcome three".to_string(),
            text: "Scene three text.".to_string(),
            status: "draft".to_string(),
        },
    )?;

    let error = engine
        .generate_next_chapter()
        .expect_err("expected sequence validation error");
    assert!(error.to_string().contains("scene order is invalid"));

    Ok(())
}

#[test]
fn config_layers_remain_separated() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("book-one");
    let global_dir = temp_dir.path().join("global-heeforge");
    std::fs::create_dir_all(&global_dir)?;
    std::fs::write(
        global_dir.join("config.toml"),
        "version = 1\ncodex_command = \"custom-codex\"\nallow_dummy_fallback = false\nworkspace_auto_commit = true\ndefault_language = \"en\"\n",
    )?;
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(
        workspace.join("novel.toml"),
        "version = 1\ntitle = \"Book One\"\nauthor = \"Hee\"\nlanguage = \"ko\"\ngenre = \"Mystery\"\ntone = \"Tense\"\n",
    )?;

    let config = Config::with_global_config_dir(workspace.clone(), global_dir)?;
    assert_eq!(config.codex_command, "custom-codex");
    assert!(!config.allow_dummy_fallback);
    assert!(config.workspace_auto_commit);
    assert_eq!(config.novel_settings.title, "Book One");
    assert_eq!(config.novel_settings.genre, "Mystery");
    assert_eq!(config.global_settings.default_language, "en");
    assert_eq!(config.workspace_config_path, workspace.join("novel.toml"));

    Ok(())
}

fn write_scene(workspace: &std::path::Path, scene: Scene) -> Result<()> {
    let path = workspace.join("02_Draft/Scenes").join(scene.file_name());
    std::fs::create_dir_all(path.parent().expect("scene parent"))?;
    std::fs::write(path, render_scene(&scene))?;
    Ok(())
}

fn scene_file_path(workspace: &std::path::Path, scene_id: &str) -> Result<std::path::PathBuf> {
    let dir = workspace.join("02_Draft/Scenes");
    for path in std::fs::read_dir(&dir)? {
        let path = path?.path();
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if file_name == format!("{scene_id}.md") || file_name.starts_with(&format!("{scene_id}-")) {
            return Ok(path);
        }
    }

    Err(anyhow::anyhow!("scene file not found for {scene_id}"))
}

fn test_engine(
    root: std::path::PathBuf,
    global_config_dir: std::path::PathBuf,
) -> Result<NovelEngine> {
    let mut config = Config::with_global_config_dir(root, global_config_dir)?;
    config.novel_settings.genre = "Mystery".to_string();
    config.novel_settings.tone = "Focused, cinematic, character-driven".to_string();
    config.novel_settings.premise =
        "A damaged investigator chases a missing sibling through a city built on edited memories."
            .to_string();
    config.novel_settings.protagonist_name = "Yunseo".to_string();
    config.allow_dummy_fallback = true;
    config.global_settings.allow_dummy_fallback = true;
    config.codex_command = "codex-command-for-tests-that-does-not-exist".to_string();
    config.global_settings.codex_command =
        "codex-command-for-tests-that-does-not-exist".to_string();
    NovelEngine::new(config)
}
