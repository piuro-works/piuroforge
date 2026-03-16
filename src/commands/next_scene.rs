use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let scene = engine.generate_next_scene()?;
    let log_path = engine.scene_generation_log_path(&scene.id);
    let scene_path = engine.scene_markdown_path(&scene.id);

    let output = CommandOutput::ok(
        "next-scene",
        engine.workspace_dir(),
        "Scene generated successfully.",
    )
    .detail("scene_id", scene.id.clone())
    .detail("short_title", scene.effective_short_title())
    .detail("goal", scene.goal.clone())
    .detail("conflict", scene.conflict.clone())
    .detail("outcome", scene.outcome.clone())
    .detail("status", scene.status.clone())
    .artifact("scene_markdown", scene_path)
    .artifact("generation_log", log_path)
    .next_step(super::workspace_command(engine, "review"))
    .next_step(super::workspace_command(
        engine,
        &format!("show {}", scene.id),
    ))
    .body(scene.text);

    Ok(super::finalize_workspace_change(
        engine,
        output,
        &format!("heeforge: draft scene {}", scene.id),
    ))
}
