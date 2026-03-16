use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine, scene_id: &str) -> Result<CommandOutput> {
    let scene = engine.show_scene(scene_id)?;

    Ok(
        CommandOutput::ok("show", engine.workspace_dir(), "Scene loaded successfully.")
            .detail("scene_id", scene.id.clone())
            .detail("short_title", scene.effective_short_title())
            .detail(
                "chapter_role",
                scene.effective_chapter_role(engine.chapter_scene_target()),
            )
            .detail("goal", scene.goal.clone())
            .detail("conflict", scene.conflict.clone())
            .detail("outcome", scene.outcome.clone())
            .detail("status", scene.status.clone())
            .artifact("scene_markdown", engine.scene_markdown_path(scene_id))
            .next_step(super::workspace_command(engine, "review"))
            .body(scene.text),
    )
}
