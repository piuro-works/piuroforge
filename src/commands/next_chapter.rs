use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let path = engine.generate_next_chapter()?;

    Ok(CommandOutput::ok(
        "next-chapter",
        engine.workspace_dir(),
        "Chapter markdown generated successfully.",
    )
    .detail("chapter_markdown", path.display().to_string())
    .artifact("chapter_markdown", &path)
    .next_step(super::workspace_command(engine, "status")))
}
