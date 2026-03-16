use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let result = engine.expand_world()?;
    let expansion = result.value;

    let mut output = CommandOutput::ok(
        "expand-world",
        engine.workspace_dir(),
        "World memory expanded successfully.",
    )
    .detail("memory_scope", "story_memory")
    .next_step(super::workspace_command(engine, "memory"))
    .body(expansion);

    for warning in result.warnings {
        output = output.warning(warning);
    }

    Ok(super::finalize_workspace_change(
        engine,
        output,
        "heeforge: expand world memory",
    ))
}
