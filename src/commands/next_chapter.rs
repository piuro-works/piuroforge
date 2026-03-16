use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let path = engine.generate_next_chapter()?;
    let chapter_number = extract_chapter_number(&path).unwrap_or(0);
    let chapter_short_title = engine
        .chapter_short_title(chapter_number)?
        .unwrap_or_else(|| format!("Chapter {chapter_number:03}"));

    let output = CommandOutput::ok(
        "next-chapter",
        engine.workspace_dir(),
        "Chapter markdown generated successfully.",
    )
    .detail("chapter", chapter_number.to_string())
    .detail("short_title", chapter_short_title)
    .detail("chapter_markdown", path.display().to_string())
    .artifact("chapter_markdown", &path)
    .next_step(super::workspace_command(engine, "status"));

    Ok(super::finalize_workspace_change(
        engine,
        output,
        &format!("piuroforge: compile chapter {chapter_number:03}"),
    ))
}

fn extract_chapter_number(path: &std::path::Path) -> Option<u32> {
    let file_name = path.file_name()?.to_str()?;
    let chapter = file_name.strip_prefix("chapter_")?;
    let chapter = chapter.split(['-', '.']).next()?;
    chapter.parse().ok()
}
