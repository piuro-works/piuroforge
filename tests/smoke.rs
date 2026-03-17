use anyhow::Result;
use piuroforge::models::Scene;
use piuroforge::novel_backend::{
    NovelBackend, ReviewRequest, ReviewResponse, RewriteRequest, RewriteResponse,
    SceneGenerationRequest, SceneGenerationResponse, WorldExpansionRequest, WorldExpansionResponse,
};
use piuroforge::utils::markdown::render_scene;
use piuroforge::{Config, NovelEngine};
use serde_json::Value;
use std::sync::Arc;
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
    assert!(global_config.contains("# PiuroForge global settings"));
    assert!(global_config.contains("codex login"));
    assert!(global_config.contains("llm_backend = \"codex_cli\""));
    assert!(global_config.contains("codex_command = \"codex\""));
    assert!(global_config.contains("allow_dummy_fallback = false"));
    assert!(global_config.contains("log_prompts = false"));
    assert!(global_config.contains("workspace_auto_commit = false"));
    assert!(workspace.join(".novel/workspace.json").exists());
    assert!(workspace.join("novel.toml").exists());
    assert!(workspace.join("README.md").exists());
    assert!(workspace.join("02_Draft/Scenes").exists());
    assert!(workspace.join("02_Draft/Bundles").exists());
    assert!(workspace.join("03_StoryBible/Characters").exists());
    assert!(workspace.join("03_StoryBible/Voice").exists());
    assert!(workspace.join("04_Research/Sources").exists());
    assert!(workspace.join("06_Review/Feedback").exists());
    assert!(workspace.join("06_Review/Revisions").exists());
    let workspace_config = std::fs::read_to_string(workspace.join("novel.toml"))?;
    assert!(workspace_config.contains("bundle_scene_target = 3"));
    assert!(workspace_config.contains("[launch_contract]"));
    assert!(workspace_config.contains("must_show_by_scene_3 = []"));
    assert!(workspace_config.contains("incident -> escalation -> cliffhanger"));
    assert!(workspace_config.contains("title = \"Demo Novel\""));
    let workspace_readme = std::fs::read_to_string(workspace.join("README.md"))?;
    assert!(workspace_readme.contains("This workspace separates human-facing manuscript files"));
    assert!(workspace_readme.contains("First Run Checklist"));
    assert!(workspace_readme.contains("piuroforge doctor"));
    assert!(workspace_readme.contains("project brief"));
    assert!(workspace_readme.contains("style/tone guide"));
    assert!(workspace_readme.contains("If Doctor says ready"));
    assert!(workspace_readme.contains("allow_dummy_fallback = false"));
    assert!(workspace_readme.contains("approval prompts"));
    let draft_readme = std::fs::read_to_string(workspace.join("02_Draft/README.md"))?;
    assert!(draft_readme.contains("Human-facing manuscript work lives here."));
    let template = std::fs::read_to_string(workspace.join("98_Templates/Scene Template.md"))?;
    assert!(template.contains("## Bundle Role"));
    assert!(template.contains("incident / escalation / cliffhanger"));
    assert!(template.contains("## Objective"));
    let character_template =
        std::fs::read_to_string(workspace.join("98_Templates/Character Template.md"))?;
    assert!(character_template.contains("## Speech Rhythm"));
    assert!(character_template.contains("## Taboo Phrases"));
    let style_template =
        std::fs::read_to_string(workspace.join("98_Templates/Style Guide Template.md"))?;
    assert!(style_template.contains("## Style Principles"));
    let tone_template =
        std::fs::read_to_string(workspace.join("98_Templates/Tone Guide Template.md"))?;
    assert!(tone_template.contains("## Tone Targets"));
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
fn generate_next_scene_rejects_launch_contract_conflict() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let mut config = Config::with_global_config_dir(workspace.clone(), global_dir)?;
    config.novel_settings.genre = "Fantasy".to_string();
    config.novel_settings.tone = "Fast, dangerous, serialized".to_string();
    config.novel_settings.premise = "An exile tries to flee a collapsing occupation city.".to_string();
    config.novel_settings.protagonist_name = "Ulan".to_string();
    config.novel_settings.serialized_workflow = true;
    config.novel_settings.launch_contract.must_show_by_scene_3 = vec![
        "larzesh".to_string(),
        "golem_hint".to_string(),
    ];
    config.allow_dummy_fallback = true;
    config.global_settings.allow_dummy_fallback = true;
    config.codex_command = "codex-command-for-tests-that-does-not-exist".to_string();
    config.global_settings.codex_command =
        "codex-command-for-tests-that-does-not-exist".to_string();
    let engine = NovelEngine::new(config)?;

    engine.init_project()?;
    std::fs::write(
        workspace.join("03_StoryBible/Plot/PLOT-000-Launch.md"),
        "# Launch\n\n## Episode Spine\n1. 배급소 붕괴와 통행패 압수\n2. 배수 골목 추락과 첫 추격 회피\n3. 떠나기 위한 최소 짐과 유적 단서 확보\n4. 검문 강화 속 비공식 탈출로로 도시 이탈\n5. 길 위에서 인간 질서의 압박 재확인\n6. 협곡 길과 호송대 전조\n7. 사슬에 묶인 라르제쉬 조우\n",
    )?;

    let error = engine
        .generate_next_scene()
        .expect_err("expected launch contract conflict");
    assert!(error.to_string().contains("launch contract validation failed"));
    assert!(error.to_string().contains("larzesh"));
    assert!(error.to_string().contains("golem_hint"));

    Ok(())
}

#[test]
fn generate_next_bundle_saves_slugged_markdown() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let engine = test_engine(workspace.clone(), temp_dir.path().join("config-home"))?;

    engine.init_project()?;
    engine.generate_next_scene()?;
    engine.generate_next_scene()?;
    engine.generate_next_scene()?;
    let bundle_path = engine.generate_next_bundle()?;

    assert_eq!(
        bundle_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
        "bundle_001-securing-the-lead.md"
    );

    let content = std::fs::read_to_string(&bundle_path)?;
    assert!(content.contains("# Bundle 001"));
    assert!(content.contains("## Short Title\nSecuring the Lead"));
    assert!(!content.contains("Status: draft\ndraft"));

    let state = engine.get_status()?;
    assert_eq!(state.current_bundle, 2);
    assert_eq!(state.current_scene, 0);
    assert_eq!(state.stage, "bundle_ready");

    Ok(())
}

#[test]
fn generate_next_scene_stops_after_bundle_scene_target() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let engine = test_engine(workspace.clone(), temp_dir.path().join("config-home"))?;

    engine.init_project()?;
    engine.generate_next_scene()?;
    engine.generate_next_scene()?;
    engine.generate_next_scene()?;

    let error = engine
        .generate_next_scene()
        .expect_err("expected bundle scene target limit");
    assert!(error.to_string().contains("bundle scene limit reached"));

    Ok(())
}

#[test]
fn serialized_workflow_auto_advances_internal_bundle_after_approval() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let mut config = Config::with_global_config_dir(workspace.clone(), global_dir)?;
    config.novel_settings.genre = "Mystery".to_string();
    config.novel_settings.tone = "Focused, cinematic, character-driven".to_string();
    config.novel_settings.premise =
        "A damaged investigator chases a missing sibling through a city built on edited memories."
            .to_string();
    config.novel_settings.protagonist_name = "Yunseo".to_string();
    config.novel_settings.serialized_workflow = true;
    config.allow_dummy_fallback = true;
    config.global_settings.allow_dummy_fallback = true;
    config.codex_command = "codex-command-for-tests-that-does-not-exist".to_string();
    config.global_settings.codex_command =
        "codex-command-for-tests-that-does-not-exist".to_string();
    let engine = NovelEngine::new(config)?;

    engine.init_project()?;
    let first = engine.generate_next_scene()?.value;
    let second = engine.generate_next_scene()?.value;
    let third = engine.generate_next_scene()?.value;
    assert_eq!(third.id, "scene_001_003");

    let error = engine
        .generate_next_scene()
        .expect_err("expected approval gate at serialized boundary");
    assert!(error
        .to_string()
        .contains("review and approve scene_001_003 before drafting the next scene"));

    engine.approve_scene(&first.id)?;
    engine.approve_scene(&second.id)?;
    engine.approve_scene(&third.id)?;

    let fourth = engine.generate_next_scene()?.value;
    assert_eq!(fourth.id, "scene_002_001");

    let state = engine.get_status()?;
    assert_eq!(state.current_bundle, 2);
    assert_eq!(state.current_scene, 1);

    Ok(())
}

#[test]
fn serialized_workflow_can_compile_previous_completed_bundle() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let mut config = Config::with_global_config_dir(workspace.clone(), global_dir)?;
    config.novel_settings.genre = "Mystery".to_string();
    config.novel_settings.tone = "Focused, cinematic, character-driven".to_string();
    config.novel_settings.premise =
        "A damaged investigator chases a missing sibling through a city built on edited memories."
            .to_string();
    config.novel_settings.protagonist_name = "Yunseo".to_string();
    config.novel_settings.serialized_workflow = true;
    config.allow_dummy_fallback = true;
    config.global_settings.allow_dummy_fallback = true;
    config.codex_command = "codex-command-for-tests-that-does-not-exist".to_string();
    config.global_settings.codex_command =
        "codex-command-for-tests-that-does-not-exist".to_string();
    let engine = NovelEngine::new(config)?;

    engine.init_project()?;
    let first = engine.generate_next_scene()?.value;
    let second = engine.generate_next_scene()?.value;
    let third = engine.generate_next_scene()?.value;
    engine.approve_scene(&first.id)?;
    engine.approve_scene(&second.id)?;
    engine.approve_scene(&third.id)?;
    let fourth = engine.generate_next_scene()?.value;
    assert_eq!(fourth.id, "scene_002_001");

    let bundle_path = engine.generate_next_bundle()?;
    assert_eq!(
        bundle_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
        "bundle_001-securing-the-lead.md"
    );

    let state = engine.get_status()?;
    assert_eq!(state.current_bundle, 2);
    assert_eq!(state.current_scene, 1);

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
    assert!(!issues.value.issues.is_empty());
    assert!(issues.value.score <= 100);
    let report = std::fs::read_to_string(workspace.join("06_Review/Feedback/scene_001_001.json"))?;
    assert!(report.contains("\"scene_id\": \"scene_001_001\""));
    assert!(report.contains("\"score\""));
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
    let record_json: Value = serde_json::from_str(&record)?;
    assert!(original_snapshot.contains(original.text.lines().next().unwrap_or_default()));
    assert!(rewritten_snapshot.contains("The revision now leans harder into"));
    assert!(record.contains(
        "\"original_snapshot_path\": \"06_Review/Revisions/scene_001_001/rewrite_001_original.md\""
    ));
    assert!(record.contains("\"rewritten_snapshot_path\": \"06_Review/Revisions/scene_001_001/rewrite_001_rewritten.md\""));
    assert_eq!(
        record_json["source_review_score"].as_u64(),
        Some(issues.value.score as u64)
    );
    assert!(record_json["post_rewrite_review_score"].is_null());

    let revised_review = engine.review_current_scene()?;
    let updated_record = std::fs::read_to_string(history_dir.join("rewrite_001.json"))?;
    let updated_record_json: Value = serde_json::from_str(&updated_record)?;
    assert_eq!(
        updated_record_json["source_review_score"].as_u64(),
        Some(issues.value.score as u64)
    );
    assert_eq!(
        updated_record_json["post_rewrite_review_score"].as_u64(),
        Some(revised_review.value.score as u64)
    );

    Ok(())
}

#[test]
fn next_bundle_rejects_gapped_scene_sequence() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let engine = test_engine(workspace.clone(), temp_dir.path().join("config-home"))?;

    engine.init_project()?;
    write_scene(
        &workspace,
        Scene {
            id: "scene_001_001".to_string(),
            bundle: 1,
            scene_number: 1,
            short_title: "Goal One".to_string(),
            bundle_role: "incident".to_string(),
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
            bundle: 1,
            scene_number: 3,
            short_title: "Goal Three".to_string(),
            bundle_role: "cliffhanger".to_string(),
            goal: "Goal three".to_string(),
            conflict: "Conflict three".to_string(),
            outcome: "Outcome three".to_string(),
            text: "Scene three text.".to_string(),
            status: "draft".to_string(),
        },
    )?;

    let error = engine
        .generate_next_bundle()
        .expect_err("expected sequence validation error");
    assert!(error.to_string().contains("scene order is invalid"));

    Ok(())
}

#[test]
fn novel_engine_can_use_injected_backend() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("backend-novel");
    let global_dir = temp_dir.path().join("config-home");
    let mut config = Config::with_global_config_dir(workspace.clone(), global_dir)?;
    config.novel_settings.genre = "Mystery".to_string();
    config.novel_settings.tone = "Focused, cinematic, character-driven".to_string();
    config.novel_settings.premise =
        "A damaged investigator chases a missing sibling through a city built on edited memories."
            .to_string();
    config.novel_settings.protagonist_name = "Yunseo".to_string();

    let engine = NovelEngine::with_backend(config, Arc::new(StubBackend))?;
    engine.init_project()?;
    let scene = engine.generate_next_scene()?.value;

    assert_eq!(scene.id, "scene_001_001");
    assert_eq!(scene.short_title, "Backend Separation");
    assert!(scene.text.contains("Stub backend final scene."));

    let saved = std::fs::read_to_string(scene_file_path(&workspace, "scene_001_001")?)?;
    assert!(saved.contains("Stub backend final scene."));
    let log =
        std::fs::read_to_string(workspace.join(".novel/logs/scene_generation/scene_001_001.json"))?;
    assert!(log.contains("\"planner_output\": \"stub-plan\""));

    Ok(())
}

#[test]
fn config_layers_remain_separated() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("book-one");
    let global_dir = temp_dir.path().join("global-piuroforge");
    std::fs::create_dir_all(&global_dir)?;
    std::fs::write(
        global_dir.join("config.toml"),
        "version = 1\nllm_backend = \"codex_cli\"\ncodex_command = \"custom-codex\"\nallow_dummy_fallback = false\nworkspace_auto_commit = true\ndefault_language = \"en\"\n",
    )?;
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(
        workspace.join("novel.toml"),
        "version = 1\ntitle = \"Book One\"\nauthor = \"Hee\"\nlanguage = \"ko\"\ngenre = \"Mystery\"\ntone = \"Tense\"\n",
    )?;

    let config = Config::with_global_config_dir(workspace.clone(), global_dir)?;
    assert_eq!(config.llm_backend, "codex_cli");
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

struct StubBackend;

impl NovelBackend for StubBackend {
    fn generate_scene(&self, request: SceneGenerationRequest) -> Result<SceneGenerationResponse> {
        Ok(SceneGenerationResponse {
            final_scene: Scene {
                id: request.scene_id,
                bundle: request.bundle,
                scene_number: request.scene_number,
                short_title: "Backend Separation".to_string(),
                bundle_role: "incident".to_string(),
                goal: "Prove the control engine only persists and advances state.".to_string(),
                conflict: "The Codex-facing generation path must stay behind the backend boundary."
                    .to_string(),
                outcome:
                    "A canned scene is persisted without the control engine knowing how it was produced."
                        .to_string(),
                text: "Stub backend final scene.".to_string(),
                status: "draft".to_string(),
            },
            planner_output: "stub-plan".to_string(),
            planner_fallback_warning: None,
            writer_output: "stub-writer".to_string(),
            writer_fallback_warning: None,
            editor_output: "stub-editor".to_string(),
            editor_fallback_warning: None,
            warnings: Vec::new(),
        })
    }

    fn review_scene(&self, _request: ReviewRequest) -> Result<ReviewResponse> {
        Ok(ReviewResponse {
            score: 100,
            issues: Vec::new(),
            critic_fallback_warning: None,
            warnings: Vec::new(),
        })
    }

    fn rewrite_scene(&self, request: RewriteRequest) -> Result<RewriteResponse> {
        Ok(RewriteResponse {
            rewritten_scene: Scene {
                text: format!("{}\n\nRewritten by stub backend.", request.scene.text),
                status: "draft".to_string(),
                ..request.scene
            },
            editor_fallback_warning: None,
            warnings: Vec::new(),
        })
    }

    fn expand_world(&self, _request: WorldExpansionRequest) -> Result<WorldExpansionResponse> {
        Ok(WorldExpansionResponse {
            expansion: "# World Expansion\n\n- Stub backend expansion.\n".to_string(),
            warnings: Vec::new(),
        })
    }
}
