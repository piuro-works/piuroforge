use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let expansion = engine.expand_world()?;

    let output = CommandOutput::ok(
        "expand-world",
        engine.workspace_dir(),
        "World memory expanded successfully.",
    )
    .detail("memory_scope", "story_memory")
    .next_step(super::workspace_command(engine, "memory"))
    .body(expansion);

    Ok(super::finalize_workspace_change(
        engine,
        output,
        "heeforge: expand world memory",
    ))
}
