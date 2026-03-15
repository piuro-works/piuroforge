use anyhow::{anyhow, Result};

use crate::agents::base::{Agent, AgentContext};
use crate::codex_runner::CodexRunner;
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

        Ok(render_template(
            CRITIC_TEMPLATE,
            &[
                ("title", context.novel.title.as_str()),
                ("genre", context.novel.genre.as_str()),
                ("tone", context.novel.tone.as_str()),
                ("premise", context.novel.premise.as_str()),
                ("protagonist_name", context.novel.protagonist_name.as_str()),
                ("goal", scene.goal.as_str()),
                ("conflict", scene.conflict.as_str()),
                ("outcome", scene.outcome.as_str()),
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
            issues.push(
                r#"{"issue_type":"pacing","description":"The scene is compact; consider adding one sensory beat or emotional reaction.","line_start":1,"line_end":3}"#,
            );
        }

        if !scene
            .text
            .to_ascii_lowercase()
            .contains(&scene.conflict.to_ascii_lowercase())
        {
            issues.push(
                r#"{"issue_type":"clarity","description":"The conflict stated in the plan is not fully visible in the prose.","line_start":2,"line_end":6}"#,
            );
        }

        if issues.is_empty() {
            issues.push(
                r#"{"issue_type":"style","description":"The draft is serviceable, but one sharper closing image would strengthen the ending.","line_start":4,"line_end":6}"#,
            );
        }

        Ok(format!("[{}]", issues.join(",")))
    }
}

impl Agent for CriticAgent {
    fn run(&self, context: &AgentContext) -> Result<String> {
        if self.use_codex {
            let prompt = self.build_prompt(context)?;
            match self.runner.run_prompt_named("critic", &prompt) {
                Ok(response) => return Ok(response),
                Err(error) if !context.allow_dummy_fallback => return Err(error),
                Err(_) => {}
            }
        }

        self.dummy_review(context)
    }
}
