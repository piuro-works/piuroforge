use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

const POLISH_INSTRUCTION: &str = "Line-edit pass only. Improve Korean sentence order, cadence, transitions, repetition control, paragraph flow, and dialogue rhythm. Preserve all story beats, facts, goals, conflicts, outcomes, and implied character choices. Do not add new plot information or remove meaningful beats unless a cut is strictly needed for sentence clarity.";

pub fn run(engine: &NovelEngine, scene_id: Option<&str>) -> Result<CommandOutput> {
    let scene_id = super::resolve_target_scene_id(engine, scene_id, "polish")?;
    let result = engine.rewrite_scene(&scene_id, POLISH_INSTRUCTION)?;
    let scene = result.value;
    let history_dir = engine.rewrite_history_dir(&scene_id);

    let mut output = CommandOutput::ok(
        "polish",
        engine.workspace_dir(),
        "Scene line-edited successfully. Sentence flow and rhythm were tightened without changing the story plan.",
    )
    .detail("scene_id", scene.id.clone())
    .detail("short_title", scene.effective_short_title())
    .detail("status", scene.status.clone())
    .detail("pass", "line_edit")
    .artifact("active_scene", engine.scene_markdown_path(&scene_id))
    .artifact("rewrite_history", history_dir)
    .next_step(super::workspace_command(
        engine,
        &format!("proofread {}", scene.id),
    ))
    .next_step(super::workspace_command(
        engine,
        &format!("show {}", scene.id),
    ))
    .body(scene.text);

    for warning in result.warnings {
        output = output.warning(warning);
    }

    Ok(super::finalize_workspace_change(
        engine,
        output,
        &format!("piuroforge: polish scene {}", scene.id),
    ))
}
