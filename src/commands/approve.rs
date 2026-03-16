use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine, scene_id: &str) -> Result<CommandOutput> {
    engine.approve_scene(scene_id)?;

    let output = CommandOutput::ok(
        "approve",
        engine.workspace_dir(),
        "Scene approved successfully.",
    )
    .detail("scene_id", scene_id)
    .artifact("scene_markdown", engine.scene_markdown_path(scene_id))
    .next_step(super::workspace_command(engine, "next-scene"))
    .next_step(super::workspace_command(engine, "next-chapter"));

    Ok(super::finalize_workspace_change(
        engine,
        output,
        &format!("heeforge: approve scene {scene_id}"),
    ))
}
