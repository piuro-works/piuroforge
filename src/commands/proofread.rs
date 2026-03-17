use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

const PROOFREAD_INSTRUCTION: &str = "Proofread pass only. Fix spelling, spacing, punctuation, particles, obvious typo-level grammar problems, awkward but local wording, and term consistency. Preserve sentence meaning, scene rhythm, paragraph structure, voice, and all story content. Do not add imagery, change pacing, or rewrite the scene at a structural level.";

pub fn run(engine: &NovelEngine, scene_id: Option<&str>) -> Result<CommandOutput> {
    let scene_id = super::resolve_target_scene_id(engine, scene_id, "proofread")?;
    let result = engine.rewrite_scene(&scene_id, PROOFREAD_INSTRUCTION)?;
    let scene = result.value;
    let history_dir = engine.rewrite_history_dir(&scene_id);

    let mut output = CommandOutput::ok(
        "proofread",
        engine.workspace_dir(),
        "Scene proofread successfully. Local copy edits were applied without changing the scene plan.",
    )
    .detail("scene_id", scene.id.clone())
    .detail("short_title", scene.effective_short_title())
    .detail("status", scene.status.clone())
    .detail("pass", "proofread")
    .artifact("active_scene", engine.scene_markdown_path(&scene_id))
    .artifact("rewrite_history", history_dir)
    .next_step(super::workspace_command(
        engine,
        &format!("show {}", scene.id),
    ))
    .next_step(super::workspace_command(
        engine,
        &format!("approve {}", scene.id),
    ))
    .body(scene.text);

    for warning in result.warnings {
        output = output.warning(warning);
    }

    Ok(super::finalize_workspace_change(
        engine,
        output,
        &format!("piuroforge: proofread scene {}", scene.id),
    ))
}
