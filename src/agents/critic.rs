use anyhow::{anyhow, Result};

use crate::agents::base::{fallback_warning, Agent, AgentContext, AgentRun};
use crate::codex_runner::CodexRunner;
use crate::models::{review_score_from_issue_count, ReviewIssue};
use crate::prompts::{render_template, CRITIC_TEMPLATE};

#[derive(Debug, Clone)]
pub struct CriticAgent {
    runner: CodexRunner,
    use_codex: bool,
}

impl CriticAgent {
    pub fn new(runner: CodexRunner, use_codex: bool) -> Self {
        Self { runner, use_codex }
    }

    fn build_prompt(&self, context: &AgentContext) -> Result<String> {
        let scene = context
            .scene
            .as_ref()
            .ok_or_else(|| anyhow!("critic requires a scene"))?;
        let chapter_role = scene.effective_chapter_role(context.novel.chapter_scene_target);

        Ok(render_template(
            CRITIC_TEMPLATE,
            &[
                ("title", context.novel.title.as_str()),
                ("genre", context.novel.genre.as_str()),
                ("tone", context.novel.tone.as_str()),
                ("premise", context.novel.premise.as_str()),
                ("protagonist_name", context.novel.protagonist_name.as_str()),
                ("chapter_role", chapter_role.as_str()),
                ("goal", scene.goal.as_str()),
                ("conflict", scene.conflict.as_str()),
                ("outcome", scene.outcome.as_str()),
                ("story_foundation", context.story_foundation.as_str()),
                ("text", scene.text.as_str()),
            ],
        ))
    }

    fn dummy_review(&self, context: &AgentContext) -> Result<String> {
        let scene = context
            .scene
            .as_ref()
            .ok_or_else(|| anyhow!("critic requires a scene"))?;

        let mut issues = Vec::new();

        if scene.text.len() < 250 {
            issues.push(ReviewIssue {
                issue_type: "pacing".to_string(),
                description:
                    "The scene is compact; consider adding one sensory beat or emotional reaction."
                        .to_string(),
                line_start: Some(1),
                line_end: Some(3),
            });
        }

        if !scene
            .text
            .to_ascii_lowercase()
            .contains(&scene.conflict.to_ascii_lowercase())
        {
            issues.push(ReviewIssue {
                issue_type: "clarity".to_string(),
                description: "The conflict stated in the plan is not fully visible in the prose."
                    .to_string(),
                line_start: Some(2),
                line_end: Some(6),
            });
        }

        if issues.is_empty() {
            issues.push(ReviewIssue {
                issue_type: "style".to_string(),
                description: "The draft is serviceable, but one sharper closing image would strengthen the ending."
                    .to_string(),
                line_start: Some(4),
                line_end: Some(6),
            });
        }

        Ok(serde_json::json!({
            "score": review_score_from_issue_count(issues.len()),
            "issues": issues,
        })
        .to_string())
    }
}

impl Agent for CriticAgent {
    fn run(&self, context: &AgentContext) -> Result<AgentRun> {
        if self.use_codex {
            let prompt = self.build_prompt(context)?;
            match self.runner.run_prompt_named("critic", &prompt) {
                Ok(response) => return Ok(AgentRun::direct(response)),
                Err(error) if !context.allow_dummy_fallback => return Err(error),
                Err(error) => {
                    return Ok(AgentRun::fallback(
                        self.dummy_review(context)?,
                        fallback_warning("critic", &error),
                    ));
                }
            }
        }

        Ok(AgentRun::fallback(
            self.dummy_review(context)?,
            "critic used dummy fallback because codex access is disabled by configuration.",
        ))
    }
}
