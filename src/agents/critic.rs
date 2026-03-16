use anyhow::{anyhow, Result};
use std::sync::Arc;

use crate::agents::base::{fallback_warning, Agent, AgentContext, AgentRun};
use crate::llm_runner::PromptRunner;
use crate::models::{review_score_from_issue_count, ReviewIssue};
use crate::prompts::{render_template, CRITIC_TEMPLATE};

#[derive(Clone)]
pub struct CriticAgent {
    runner: Arc<dyn PromptRunner>,
}

impl CriticAgent {
    pub fn new(runner: Arc<dyn PromptRunner>) -> Self {
        Self { runner }
    }

    fn build_prompt(&self, context: &AgentContext) -> Result<String> {
        let scene = context
            .scene
            .as_ref()
            .ok_or_else(|| anyhow!("critic requires a scene"))?;
        let bundle_role = scene.effective_bundle_role(context.novel.bundle_scene_target);
        let scene_length_guidance = if context.novel.serialized_workflow {
            "serialized episode mode expects roughly 1800-2600 Korean characters unless a short punchy beat is clearly intentional"
        } else {
            "compact draft mode expects roughly 800-1200 Korean characters unless there is a clear reason to go shorter or longer"
        };

        Ok(render_template(
            CRITIC_TEMPLATE,
            &[
                ("title", context.novel.title.as_str()),
                ("genre", context.novel.genre.as_str()),
                ("tone", context.novel.tone.as_str()),
                ("premise", context.novel.premise.as_str()),
                ("protagonist_name", context.novel.protagonist_name.as_str()),
                ("bundle_role", bundle_role.as_str()),
                ("goal", scene.goal.as_str()),
                ("conflict", scene.conflict.as_str()),
                ("outcome", scene.outcome.as_str()),
                ("scene_length_guidance", scene_length_guidance),
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
        let prompt = self.build_prompt(context)?;
        match self.runner.run_prompt_named("critic", &prompt) {
            Ok(response) => Ok(AgentRun::direct(response)),
            Err(error) if !context.allow_dummy_fallback => Err(error),
            Err(error) => Ok(AgentRun::fallback(
                self.dummy_review(context)?,
                fallback_warning("critic", &error),
            )),
        }
    }
}
