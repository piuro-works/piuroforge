use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let path = engine.generate_next_bundle()?;
    let bundle_number = extract_bundle_number(&path).unwrap_or(0);
    let bundle_short_title = engine
        .bundle_short_title(bundle_number)?
        .unwrap_or_else(|| format!("Bundle {bundle_number:03}"));

    let output = CommandOutput::ok(
        "next-bundle",
        engine.workspace_dir(),
        "Bundle markdown generated successfully.",
    )
    .detail("bundle", bundle_number.to_string())
    .detail("short_title", bundle_short_title)
    .detail("bundle_markdown", path.display().to_string())
    .artifact("bundle_markdown", &path)
    .next_step(super::workspace_command(engine, "status"));

    Ok(super::finalize_workspace_change(
        engine,
        output,
        &format!("piuroforge: compile bundle {bundle_number:03}"),
    ))
}

fn extract_bundle_number(path: &std::path::Path) -> Option<u32> {
    let file_name = path.file_name()?.to_str()?;
    let bundle = file_name.strip_prefix("bundle_")?;
    let bundle = bundle.split(['-', '.']).next()?;
    bundle.parse().ok()
}
