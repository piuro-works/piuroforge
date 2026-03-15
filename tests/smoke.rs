use anyhow::Result;
use novel_engine::models::Scene;
use novel_engine::utils::markdown::render_scene;
use novel_engine::{Config, NovelEngine};
use tempfile::tempdir;

#[test]
fn init_project_creates_workspace_scaffold() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let engine = test_engine(workspace.clone(), temp_dir.path().join("config-home"))?;

    engine.init_project()?;

    assert!(temp_dir.path().join("config-home/config.toml").exists());
    let global_config = std::fs::read_to_string(temp_dir.path().join("config-home/config.toml"))?;
    assert!(global_config.contains("codex_command = \"codex\""));
    assert!(workspace.join(".novel/workspace.json").exists());
    assert!(workspace.join("novel.toml").exists());
    let workspace_config = std::fs::read_to_string(workspace.join("novel.toml"))?;
    assert!(workspace_config.contains("title = \"Demo Novel\""));
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
    let scene = engine.generate_next_scene()?;

    assert_eq!(scene.id, "scene_001_001");
    let scene_path = workspace.join(".novel/scenes/scene_001_001.md");
    assert!(scene_path.exists());

    let saved = std::fs::read_to_string(scene_path)?;
    assert!(saved.contains("# Scene scene_001_001"));
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
fn review_and_rewrite_persist_artifacts() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let engine = test_engine(workspace.clone(), temp_dir.path().join("config-home"))?;

    engine.init_project()?;
    let original = engine.generate_next_scene()?;

    let issues = engine.review_current_scene()?;
    assert!(!issues.is_empty());
    let report = std::fs::read_to_string(workspace.join(".novel/logs/reviews/scene_001_001.json"))?;
    assert!(report.contains("\"scene_id\": \"scene_001_001\""));

    let rewritten = engine.rewrite_scene("scene_001_001", "Make it darker and sharper")?;
    assert_eq!(rewritten.id, "scene_001_001");
    let history_dir = workspace.join(".novel/logs/rewrites/scene_001_001");
    assert!(history_dir.join("rewrite_001_original.md").exists());
    assert!(history_dir.join("rewrite_001_rewritten.md").exists());
    assert!(history_dir.join("rewrite_001.json").exists());

    let original_snapshot = std::fs::read_to_string(history_dir.join("rewrite_001_original.md"))?;
    let rewritten_snapshot = std::fs::read_to_string(history_dir.join("rewrite_001_rewritten.md"))?;
    assert!(original_snapshot.contains(original.text.lines().next().unwrap_or_default()));
    assert!(rewritten_snapshot.contains("The revision now leans harder into"));

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
        "version = 1\ncodex_command = \"custom-codex\"\nallow_dummy_fallback = false\ndefault_language = \"en\"\n",
    )?;
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(
        workspace.join("novel.toml"),
        "version = 1\ntitle = \"Book One\"\nauthor = \"Hee\"\nlanguage = \"ko\"\ngenre = \"Mystery\"\ntone = \"Tense\"\n",
    )?;

    let config = Config::with_global_config_dir(workspace.clone(), global_dir)?;
    assert_eq!(config.codex_command, "custom-codex");
    assert!(!config.allow_dummy_fallback);
    assert_eq!(config.novel_settings.title, "Book One");
    assert_eq!(config.novel_settings.genre, "Mystery");
    assert_eq!(config.global_settings.default_language, "en");
    assert_eq!(config.workspace_config_path, workspace.join("novel.toml"));

    Ok(())
}

fn write_scene(workspace: &std::path::Path, scene: Scene) -> Result<()> {
    let path = workspace
        .join(".novel/scenes")
        .join(format!("{}.md", scene.id));
    std::fs::create_dir_all(path.parent().expect("scene parent"))?;
    std::fs::write(path, render_scene(&scene))?;
    Ok(())
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
    config.codex_command = "codex-command-for-tests-that-does-not-exist".to_string();
    NovelEngine::new(config)
}
