use anyhow::Result;
use heeforge::{Config, NovelEngine};
use serde_json::Value;
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
    assert_eq!(payload["workspace"], workspace.display().to_string());
    assert!(payload["summary"]
        .as_str()
        .unwrap_or_default()
        .contains("Workspace"));
    assert!(payload["details"].is_array());
    assert!(payload["next_steps"].is_array());

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
    NovelEngine::new(config)
}

fn novel_bin() -> &'static str {
    env!("CARGO_BIN_EXE_heeforge")
}
