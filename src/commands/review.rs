use anyhow::Result;

use crate::engine::NovelEngine;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let result = engine.review_current_scene()?;
    let score = result.value.score;
    let issues = result.value.issues;
    let state = engine.get_status()?;
    let scene_id = state
        .current_scene_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("no current scene available to review"))?;
    let review_path = engine.review_report_path(scene_id);
    let summary = if issues.is_empty() {
        format!("Review completed. Score {}/100. No issues found.", score)
    } else {
        format!("Review completed. Score {}/100. Issues were found.", score)
    };

    let mut output = CommandOutput::ok("review", engine.workspace_dir(), summary)
        .detail("scene_id", scene_id)
        .detail("score", score.to_string())
        .detail("issue_count", issues.len().to_string())
        .artifact("review_json", &review_path);

    for warning in result.warnings {
        output = output.warning(warning);
    }

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

    let mut lines = vec![format!("Score: {}/100", score)];
    lines.extend(issues.iter().enumerate().map(|(index, issue)| {
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
    }));
    let body = lines.join("\n");

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
