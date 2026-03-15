use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine, scene_id: &str) -> Result<CommandOutput> {
    engine.approve_scene(scene_id)?;

    Ok(CommandOutput::ok(
        "approve",
        engine.workspace_dir(),
        "Scene approved successfully.",
    )
    .detail("scene_id", scene_id)
    .artifact("scene_markdown", engine.scene_markdown_path(scene_id))
    .next_step(super::workspace_command(engine, "next-scene"))
    .next_step(super::workspace_command(engine, "next-chapter")))
}
