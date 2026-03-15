use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let memory = engine.get_memory()?;
    let body = format!(
        "=== Core Memory ===\n{}\n\n=== Story Memory ===\n{}\n\n=== Active Memory ===\n{}",
        memory.core_memory.trim(),
        memory.story_memory.trim(),
        memory.active_memory.trim()
    );

    Ok(CommandOutput::ok(
        "memory",
        engine.workspace_dir(),
        "Memory bundle loaded successfully.",
    )
    .detail("core_memory_bytes", memory.core_memory.len().to_string())
    .detail("story_memory_bytes", memory.story_memory.len().to_string())
    .detail(
        "active_memory_bytes",
        memory.active_memory.len().to_string(),
    )
    .body(body))
}
