use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine, scene_id: &str, instruction: &str) -> Result<CommandOutput> {
    let result = engine.rewrite_scene(scene_id, instruction)?;
    let scene = result.value;
    let history_dir = engine.rewrite_history_dir(scene_id);

    let mut output = CommandOutput::ok(
        "rewrite",
        engine.workspace_dir(),
        "Scene rewritten successfully. Original and revised snapshots were preserved.",
    )
    .detail("scene_id", scene.id.clone())
    .detail("short_title", scene.effective_short_title())
    .detail("status", scene.status.clone())
    .detail("instruction", instruction)
    .artifact("active_scene", engine.scene_markdown_path(scene_id))
    .artifact("rewrite_history", history_dir)
    .next_step(super::workspace_command(engine, "review"))
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
        &format!("heeforge: rewrite scene {}", scene.id),
    ))
}
