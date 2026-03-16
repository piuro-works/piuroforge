use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let issues = engine.review_current_scene()?;
    let state = engine.get_status()?;
    let scene_id = state
        .current_scene_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no current scene available to review"))?;
    let review_path = engine.review_report_path(scene_id);
    let summary = if issues.is_empty() {
        "Review completed. No issues found."
    } else {
        "Review completed. Issues were found."
    };

    let mut output = CommandOutput::ok("review", engine.workspace_dir(), summary)
        .detail("scene_id", scene_id)
        .detail("issue_count", issues.len().to_string())
        .artifact("review_json", &review_path);

    if issues.is_empty() {
        output = output
            .next_step(super::workspace_command(
                engine,
                &format!("approve {scene_id}"),
            ))
            .next_step(super::workspace_command(
                engine,
                &format!("show {scene_id}"),
            ));
        return Ok(super::finalize_workspace_change(
            engine,
            output,
            &format!("heeforge: review scene {scene_id}"),
        ));
    }

    let body = issues
        .iter()
        .enumerate()
        .map(|(index, issue)| {
            let range = match (issue.line_start, issue.line_end) {
                (Some(start), Some(end)) => format!("lines {}-{}", start, end),
                (Some(start), None) => format!("line {}", start),
                _ => "lines n/a".to_string(),
            };
            format!(
                "{}. [{}] {} ({})",
                index + 1,
                issue.issue_type,
                issue.description,
                range
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    output = output
        .next_step(super::workspace_command(
            engine,
            &format!("rewrite {scene_id} --instruction \"Describe the intended fix\""),
        ))
        .next_step(super::workspace_command(
            engine,
            &format!("show {scene_id}"),
        ))
        .body(body);

    Ok(super::finalize_workspace_change(
        engine,
        output,
        &format!("heeforge: review scene {scene_id}"),
    ))
}
