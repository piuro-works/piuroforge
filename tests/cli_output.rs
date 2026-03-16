use anyhow::Result;
use heeforge::models::Scene;
use heeforge::utils::markdown::render_scene;
use heeforge::{Config, NovelEngine};
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn help_mentions_json_output_and_examples() -> Result<()> {
    let output = Command::new(novel_bin()).arg("--help").output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("--format"));
    assert!(stdout.contains("Use `json` for Codex CLI"));
    assert!(stdout.contains("Quickstart:"));
    assert!(stdout.contains("heeforge doctor"));
    assert!(stdout.contains("heeforge --workspace ~/novels/my-book --format json status"));

    Ok(())
}

#[test]
fn status_json_output_is_structured() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("status")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "status");
    assert_same_path(
        payload["workspace"].as_str().unwrap_or_default(),
        &workspace,
    )?;
    assert!(payload["summary"]
        .as_str()
        .unwrap_or_default()
        .contains("Workspace"));
    assert!(payload["details"].is_array());
    assert!(payload["next_steps"].is_array());

    Ok(())
}

#[test]
fn doctor_json_reports_setup_issues_without_workspace() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("doctor-novel");
    let global_dir = temp_dir.path().join("config-home");

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("doctor")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .env(
            "HEEFORGE_CODEX_CMD",
            "codex-command-for-tests-that-does-not-exist",
        )
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "doctor");
    assert_eq!(detail_value(&payload, "workspace_ready"), Some("no"));
    assert_eq!(detail_value(&payload, "codex_cli"), Some("missing"));
    assert_eq!(detail_value(&payload, "codex_connection"), Some("missing"));
    assert!(warning_contains(&payload, "No HeeForge workspace marker"));
    assert!(warning_contains(&payload, "Codex CLI was not found"));
    assert!(payload["next_steps"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .any(|item| item.as_str().unwrap_or_default().contains("heeforge init")));
    assert!(payload["next_steps"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .any(|item| item.as_str().unwrap_or_default().contains("codex login")));

    Ok(())
}

#[test]
fn init_json_creates_ready_workspace_without_prompting() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("fresh-novel");
    let global_dir = temp_dir.path().join("config-home");

    let output = Command::new(novel_bin())
        .arg("--format")
        .arg("json")
        .arg("init")
        .arg(&workspace)
        .arg("--title")
        .arg("Fresh Novel")
        .arg("--genre")
        .arg("Mystery")
        .arg("--tone")
        .arg("Tense, atmospheric")
        .arg("--premise")
        .arg("A damaged investigator chases a missing sibling through a city built on edited memories.")
        .arg("--protagonist")
        .arg("Yunseo")
        .arg("--language")
        .arg("ko")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "init");
    assert_same_path(
        payload["workspace"].as_str().unwrap_or_default(),
        &workspace,
    )?;
    assert_eq!(detail_value(&payload, "title"), Some("Fresh Novel"));
    assert_eq!(
        detail_value(&payload, "writer_setup"),
        Some("Run `codex login` once, then run `heeforge doctor`.")
    );
    assert_eq!(
        detail_value(&payload, "setup_done_when"),
        Some("If `heeforge doctor` says ready, HeeForge setup is finished and you can draft.")
    );
    assert!(detail_value(&payload, "hosted_agent_note")
        .unwrap_or_default()
        .contains("approval prompts"));
    assert!(payload["next_steps"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .any(|item| item.as_str().unwrap_or_default().contains("doctor")));
    assert!(workspace.join("novel.toml").exists());
    assert!(workspace.join("README.md").exists());
    assert!(workspace.join("98_Templates/Scene Template.md").exists());

    Ok(())
}

#[test]
fn workspace_auto_commit_initializes_repo_and_tracks_mutations() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("git-auto-novel");
    let global_dir = temp_dir.path().join("config-home");

    let init_output = Command::new(novel_bin())
        .arg("--format")
        .arg("json")
        .arg("init")
        .arg(&workspace)
        .arg("--title")
        .arg("Git Auto Novel")
        .arg("--genre")
        .arg("Mystery")
        .arg("--tone")
        .arg("Tense, atmospheric")
        .arg("--premise")
        .arg("A damaged investigator chases a missing sibling through a city built on edited memories.")
        .arg("--protagonist")
        .arg("Yunseo")
        .arg("--language")
        .arg("ko")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .env("HEEFORGE_WORKSPACE_AUTO_COMMIT", "true")
        .output()?;

    assert!(init_output.status.success());
    assert!(workspace.join(".git").exists());

    let init_stdout = String::from_utf8(init_output.stdout)?;
    let init_payload: Value = serde_json::from_str(&init_stdout)?;
    assert_eq!(
        detail_value(&init_payload, "workspace_git"),
        Some("initialized")
    );
    assert!(detail_value(&init_payload, "git_commit").is_some());
    assert_eq!(
        git_stdout(&workspace, ["log", "-1", "--pretty=%s"])?,
        "heeforge: initialize workspace"
    );

    let next_scene_output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("next-scene")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .env("HEEFORGE_ALLOW_DUMMY", "true")
        .env("HEEFORGE_WORKSPACE_AUTO_COMMIT", "true")
        .env(
            "HEEFORGE_CODEX_CMD",
            "codex-command-for-tests-that-does-not-exist",
        )
        .output()?;

    assert!(next_scene_output.status.success());

    let next_scene_stdout = String::from_utf8(next_scene_output.stdout)?;
    let next_scene_payload: Value = serde_json::from_str(&next_scene_stdout)?;
    assert!(detail_value(&next_scene_payload, "git_commit").is_some());
    assert!(warning_contains(&next_scene_payload, "dummy fallback"));
    assert_eq!(
        git_stdout(&workspace, ["rev-list", "--count", "HEAD"])?,
        "2"
    );
    assert_eq!(
        git_stdout(&workspace, ["log", "-1", "--pretty=%s"])?,
        "heeforge: draft scene scene_001_001"
    );

    Ok(())
}

#[test]
fn next_scene_json_error_when_codex_unavailable_by_default() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("codex-required-novel");
    let global_dir = temp_dir.path().join("config-home");

    let init_output = Command::new(novel_bin())
        .arg("--format")
        .arg("json")
        .arg("init")
        .arg(&workspace)
        .arg("--title")
        .arg("Codex Required Novel")
        .arg("--genre")
        .arg("Mystery")
        .arg("--tone")
        .arg("Tense, atmospheric")
        .arg("--premise")
        .arg("A damaged investigator chases a missing sibling through a city built on edited memories.")
        .arg("--protagonist")
        .arg("Yunseo")
        .arg("--language")
        .arg("ko")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(init_output.status.success());

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("next-scene")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .env(
            "HEEFORGE_CODEX_CMD",
            "codex-command-for-tests-that-does-not-exist",
        )
        .output()?;

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr)?;
    let payload: Value = serde_json::from_str(&stderr)?;
    assert_eq!(payload["status"], "error");
    assert_eq!(payload["command"], "next-scene");
    assert_eq!(payload["error_code"], "codex_unavailable");
    assert!(payload["reason"]
        .as_str()
        .unwrap_or_default()
        .contains("codex"));
    assert!(payload["remediation"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .any(|item| {
            item.as_str()
                .unwrap_or_default()
                .contains("allow_dummy_fallback = true")
        }));

    Ok(())
}

#[test]
fn next_scene_json_with_opt_in_dummy_fallback_surfaces_warning() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("codex-opt-in-novel");
    let global_dir = temp_dir.path().join("config-home");

    let init_output = Command::new(novel_bin())
        .arg("--format")
        .arg("json")
        .arg("init")
        .arg(&workspace)
        .arg("--title")
        .arg("Codex Opt In Novel")
        .arg("--genre")
        .arg("Mystery")
        .arg("--tone")
        .arg("Tense, atmospheric")
        .arg("--premise")
        .arg("A damaged investigator chases a missing sibling through a city built on edited memories.")
        .arg("--protagonist")
        .arg("Yunseo")
        .arg("--language")
        .arg("ko")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(init_output.status.success());

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("next-scene")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .env("HEEFORGE_ALLOW_DUMMY", "true")
        .env(
            "HEEFORGE_CODEX_CMD",
            "codex-command-for-tests-that-does-not-exist",
        )
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "next-scene");
    assert!(warning_contains(&payload, "planner used dummy fallback"));
    assert!(warning_contains(&payload, "writer used dummy fallback"));
    assert!(warning_contains(&payload, "editor used dummy fallback"));

    Ok(())
}

#[test]
fn status_json_auto_detects_workspace_from_nested_directory() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let nested_dir = workspace.join("notes/drafts");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;
    std::fs::create_dir_all(&nested_dir)?;

    let output = Command::new(novel_bin())
        .arg("--format")
        .arg("json")
        .arg("status")
        .current_dir(&nested_dir)
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "status");
    assert_same_path(
        payload["workspace"].as_str().unwrap_or_default(),
        &workspace,
    )?;

    Ok(())
}

#[test]
fn review_json_output_is_structured() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;
    engine.generate_next_scene()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("review")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .env(
            "HEEFORGE_CODEX_CMD",
            "codex-command-for-tests-that-does-not-exist",
        )
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "review");
    assert_eq!(detail_value(&payload, "scene_id"), Some("scene_001_001"));
    assert!(warning_contains(&payload, "critic used dummy fallback"));
    let issue_count = detail_value(&payload, "issue_count")
        .unwrap_or("0")
        .parse::<u32>()?;
    assert!(issue_count >= 1);
    assert!(payload["body"].as_str().unwrap_or_default().contains("1."));
    assert!(workspace
        .join("06_Review/Feedback/scene_001_001.json")
        .exists());

    Ok(())
}

#[test]
fn review_json_error_without_current_scene_is_structured() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("review")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr)?;
    let payload: Value = serde_json::from_str(&stderr)?;
    assert_eq!(payload["status"], "error");
    assert_eq!(payload["command"], "review");
    assert_eq!(payload["error_code"], "no_current_scene");
    assert!(payload["reason"]
        .as_str()
        .unwrap_or_default()
        .contains("no current scene available to review"));
    assert!(payload["remediation"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .any(|item| item.as_str().unwrap_or_default().contains("next-scene")));

    Ok(())
}

#[test]
fn next_scene_json_error_contains_remediation() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let config = Config::with_global_config_dir(workspace.clone(), global_dir.clone())?;
    let engine = NovelEngine::new(config)?;
    engine.init_project()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("next-scene")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr)?;
    let payload: Value = serde_json::from_str(&stderr)?;
    assert_eq!(payload["status"], "error");
    assert_eq!(payload["error_code"], "missing_novel_config");
    assert!(payload["reason"]
        .as_str()
        .unwrap_or_default()
        .contains("missing required novel config"));
    assert!(payload["remediation"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .any(|item| item.as_str().unwrap_or_default().contains("novel.toml")));

    Ok(())
}

#[test]
fn show_json_output_is_structured() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;
    engine.generate_next_scene()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("show")
        .arg("scene_001_001")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "show");
    assert_eq!(detail_value(&payload, "scene_id"), Some("scene_001_001"));
    assert_eq!(
        detail_value(&payload, "short_title"),
        Some("Securing the Lead")
    );
    assert!(payload["body"]
        .as_str()
        .unwrap_or_default()
        .contains("The protagonist stepped into the scene"));

    Ok(())
}

#[test]
fn show_json_error_for_missing_scene_is_structured() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("show")
        .arg("scene_999_999")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr)?;
    let payload: Value = serde_json::from_str(&stderr)?;
    assert_eq!(payload["status"], "error");
    assert_eq!(payload["command"], "show");
    assert_eq!(payload["error_code"], "command_failed");
    assert!(payload["reason"]
        .as_str()
        .unwrap_or_default()
        .contains("scene_999_999"));
    let expected_example = format!("heeforge --workspace {} show", workspace.display());
    assert_eq!(
        payload["example_command"].as_str().unwrap_or_default(),
        expected_example
    );

    Ok(())
}

#[test]
fn next_scene_json_auto_detects_workspace_from_nested_directory() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let nested_dir = workspace.join("notes/drafts");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;
    std::fs::create_dir_all(&nested_dir)?;

    let output = Command::new(novel_bin())
        .arg("--format")
        .arg("json")
        .arg("next-scene")
        .current_dir(&nested_dir)
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .env(
            "HEEFORGE_CODEX_CMD",
            "codex-command-for-tests-that-does-not-exist",
        )
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "next-scene");
    assert!(warning_contains(&payload, "planner used dummy fallback"));
    assert_same_path(
        payload["workspace"].as_str().unwrap_or_default(),
        &workspace,
    )?;
    assert_eq!(
        detail_value(&payload, "short_title"),
        Some("Securing the Lead")
    );
    let scene_path = scene_file_path(&workspace, "scene_001_001")?;
    assert!(scene_path.exists());
    assert_eq!(
        scene_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default(),
        "scene_001_001-securing-the-lead.md"
    );

    Ok(())
}

#[test]
fn memory_json_output_is_structured() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;
    engine.generate_next_scene()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("memory")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "memory");
    let core_bytes = detail_value(&payload, "core_memory_bytes")
        .unwrap_or("0")
        .parse::<usize>()?;
    let story_bytes = detail_value(&payload, "story_memory_bytes")
        .unwrap_or("0")
        .parse::<usize>()?;
    let active_bytes = detail_value(&payload, "active_memory_bytes")
        .unwrap_or("0")
        .parse::<usize>()?;
    assert!(core_bytes > 0);
    assert!(story_bytes > 0);
    assert!(active_bytes > 0);
    let body = payload["body"].as_str().unwrap_or_default();
    assert!(body.contains("=== Core Memory ==="));
    assert!(body.contains("Securing the Lead"));

    Ok(())
}

#[test]
fn rewrite_json_output_preserves_history_and_updates_scene() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;
    engine.generate_next_scene()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("rewrite")
        .arg("scene_001_001")
        .arg("--instruction")
        .arg("Make it darker and sharper")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .env(
            "HEEFORGE_CODEX_CMD",
            "codex-command-for-tests-that-does-not-exist",
        )
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "rewrite");
    assert_same_path(
        payload["workspace"].as_str().unwrap_or_default(),
        &workspace,
    )?;
    assert_eq!(detail_value(&payload, "scene_id"), Some("scene_001_001"));
    assert_eq!(
        detail_value(&payload, "short_title"),
        Some("Securing the Lead")
    );
    assert_eq!(detail_value(&payload, "status"), Some("draft"));
    assert_eq!(
        detail_value(&payload, "instruction"),
        Some("Make it darker and sharper")
    );
    assert!(warning_contains(&payload, "editor used dummy fallback"));
    assert!(payload["body"]
        .as_str()
        .unwrap_or_default()
        .contains("The revision now leans harder into Make it darker and sharper."));

    let scene_markdown = std::fs::read_to_string(scene_file_path(&workspace, "scene_001_001")?)?;
    assert!(scene_markdown.contains("## Short Title\nSecuring the Lead"));
    assert!(scene_markdown.contains("## Status\ndraft"));
    assert!(
        scene_markdown.contains("The revision now leans harder into Make it darker and sharper.")
    );

    let history_dir = workspace.join("06_Review/Revisions/scene_001_001");
    assert!(history_dir.join("rewrite_001_original.md").exists());
    assert!(history_dir.join("rewrite_001_rewritten.md").exists());
    assert!(history_dir.join("rewrite_001.json").exists());

    Ok(())
}

#[test]
fn expand_world_json_output_updates_story_memory() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("expand-world")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .env(
            "HEEFORGE_CODEX_CMD",
            "codex-command-for-tests-that-does-not-exist",
        )
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "expand-world");
    assert_eq!(detail_value(&payload, "memory_scope"), Some("story_memory"));
    assert!(warning_contains(
        &payload,
        "expand-world used dummy fallback"
    ));
    assert!(payload["body"]
        .as_str()
        .unwrap_or_default()
        .contains("# World Expansion"));

    let story_memory = std::fs::read_to_string(workspace.join(".novel/memory/story_memory.md"))?;
    assert!(story_memory.contains("## World Expansion"));

    Ok(())
}

#[test]
fn next_chapter_json_output_includes_short_title_and_slugged_path() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;
    engine.generate_next_scene()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("next-chapter")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "next-chapter");
    assert_eq!(detail_value(&payload, "chapter"), Some("1"));
    assert_eq!(
        detail_value(&payload, "short_title"),
        Some("Securing the Lead")
    );

    let chapter_path = payload["artifacts"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["path"].as_str())
        .ok_or_else(|| anyhow::anyhow!("missing chapter artifact path"))?;
    assert!(chapter_path.ends_with("chapter_001-securing-the-lead.md"));
    let chapter_markdown = std::fs::read_to_string(chapter_path)?;
    assert!(chapter_markdown.contains("## Short Title\nSecuring the Lead"));

    Ok(())
}

#[test]
fn next_chapter_json_error_without_scenes_is_structured() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("next-chapter")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr)?;
    let payload: Value = serde_json::from_str(&stderr)?;
    assert_eq!(payload["status"], "error");
    assert_eq!(payload["command"], "next-chapter");
    assert_eq!(payload["error_code"], "empty_chapter");
    assert!(payload["reason"]
        .as_str()
        .unwrap_or_default()
        .contains("no scenes found for chapter 001"));
    assert!(payload["remediation"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .any(|item| item.as_str().unwrap_or_default().contains("next-scene")));

    Ok(())
}

#[test]
fn next_chapter_json_error_for_gapped_sequence_is_structured() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
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

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("next-chapter")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr)?;
    let payload: Value = serde_json::from_str(&stderr)?;
    assert_eq!(payload["status"], "error");
    assert_eq!(payload["command"], "next-chapter");
    assert_eq!(payload["error_code"], "invalid_scene_sequence");
    assert!(payload["reason"]
        .as_str()
        .unwrap_or_default()
        .contains("scene order is invalid"));
    assert!(payload["remediation"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .any(|item| item.as_str().unwrap_or_default().contains("without gaps")));

    Ok(())
}

#[test]
fn approve_json_output_marks_scene_and_state_as_approved() -> Result<()> {
    let temp_dir = tempdir()?;
    let workspace = temp_dir.path().join("demo-novel");
    let global_dir = temp_dir.path().join("config-home");
    let engine = ready_engine(workspace.clone(), global_dir.clone())?;
    engine.init_project()?;
    engine.generate_next_scene()?;

    let output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("approve")
        .arg("scene_001_001")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout)?;
    let payload: Value = serde_json::from_str(&stdout)?;
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["command"], "approve");
    assert_same_path(
        payload["workspace"].as_str().unwrap_or_default(),
        &workspace,
    )?;
    assert_eq!(detail_value(&payload, "scene_id"), Some("scene_001_001"));

    let scene_markdown = std::fs::read_to_string(scene_file_path(&workspace, "scene_001_001")?)?;
    assert!(scene_markdown.contains("## Status\napproved"));

    let status_output = Command::new(novel_bin())
        .arg("--workspace")
        .arg(&workspace)
        .arg("--format")
        .arg("json")
        .arg("status")
        .env("HEEFORGE_CONFIG_DIR", &global_dir)
        .output()?;

    assert!(status_output.status.success());

    let status_stdout = String::from_utf8(status_output.stdout)?;
    let status_payload: Value = serde_json::from_str(&status_stdout)?;
    assert_eq!(
        detail_value(&status_payload, "stage"),
        Some("scene_approved")
    );
    assert_eq!(
        detail_value(&status_payload, "current_scene_id"),
        Some("scene_001_001")
    );

    Ok(())
}

fn ready_engine(
    workspace: std::path::PathBuf,
    global_dir: std::path::PathBuf,
) -> Result<NovelEngine> {
    let mut config = Config::with_global_config_dir(workspace, global_dir)?;
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

fn novel_bin() -> &'static str {
    env!("CARGO_BIN_EXE_heeforge")
}

fn assert_same_path(actual: &str, expected: &Path) -> Result<()> {
    let actual = std::fs::canonicalize(actual)?;
    let expected = std::fs::canonicalize(expected)?;
    assert_eq!(actual, expected);
    Ok(())
}

fn detail_value<'a>(payload: &'a Value, label: &str) -> Option<&'a str> {
    payload["details"]
        .as_array()?
        .iter()
        .find(|item| item["label"].as_str() == Some(label))
        .and_then(|item| item["value"].as_str())
}

fn warning_contains(payload: &Value, needle: &str) -> bool {
    payload["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str())
        .any(|warning| warning.contains(needle))
}

fn scene_file_path(workspace: &Path, scene_id: &str) -> Result<std::path::PathBuf> {
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

fn write_scene(workspace: &Path, scene: Scene) -> Result<()> {
    let path = workspace.join("02_Draft/Scenes").join(scene.file_name());
    std::fs::create_dir_all(path.parent().expect("scene parent"))?;
    std::fs::write(path, render_scene(&scene))?;
    Ok(())
}

fn git_stdout<const N: usize>(workspace: &Path, args: [&str; N]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(workspace)
        .args(args)
        .output()?;
    assert!(
        output.status.success(),
        "git command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
