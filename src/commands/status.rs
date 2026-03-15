use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let state = engine.get_status()?;
    let missing = engine.missing_required_novel_fields();

    let summary = if !missing.is_empty() {
        "Workspace scaffold exists, but novel config is still incomplete."
    } else if state.current_scene_id.is_none() {
        "Workspace is ready for the first scene."
    } else if state.stage == "scene_approved" {
        "Current scene is approved. You can draft the next scene or compile a chapter."
    } else {
        "Current scene is in progress."
    };

    let mut output = CommandOutput::ok("status", engine.workspace_dir(), summary)
        .detail("arc", state.current_arc.to_string())
        .detail("chapter", state.current_chapter.to_string())
        .detail("scene", state.current_scene.to_string())
        .detail("stage", state.stage.clone())
        .detail(
            "current_scene_id",
            state.current_scene_id.as_deref().unwrap_or("-"),
        )
        .detail("current_goal", state.current_goal.as_deref().unwrap_or("-"))
        .detail(
            "open_conflict_count",
            state.open_conflicts.len().to_string(),
        )
        .artifact("workspace_config", engine.workspace_config_path());

    if !state.open_conflicts.is_empty() {
        output = output.body(super::sentence_list(&state.open_conflicts));
    }

    if !missing.is_empty() {
        output = output
            .warning(format!(
                "Missing required novel config: {}",
                missing.join(", ")
            ))
            .next_step(format!("Edit {}", engine.workspace_config_path().display()));
    } else if let Some(scene_id) = state.current_scene_id.as_deref() {
        output = output
            .next_step(super::workspace_command(
                engine,
                &format!("show {scene_id}"),
            ))
            .next_step(super::workspace_command(engine, "review"));
    } else {
        output = output.next_step(super::workspace_command(engine, "next-scene"));
    }

    Ok(output)
}
