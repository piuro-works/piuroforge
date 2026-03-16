use anyhow::{anyhow, Result};

use crate::agents::base::{fallback_warning, Agent, AgentContext, AgentRun};
use crate::codex_runner::CodexRunner;
use crate::prompts::{render_template, WRITER_TEMPLATE};

#[derive(Debug, Clone)]
pub struct WriterAgent {
    runner: CodexRunner,
    use_codex: bool,
}

impl WriterAgent {
    pub fn new(runner: CodexRunner, use_codex: bool) -> Self {
        Self { runner, use_codex }
    }

    fn build_prompt(&self, context: &AgentContext) -> Result<String> {
        let plan = context
            .scene_plan
            .as_ref()
            .ok_or_else(|| anyhow!("writer requires a scene plan"))?;

        let scene_id = plan.scene_id();
        Ok(render_template(
            WRITER_TEMPLATE,
            &[
                ("scene_id", scene_id.as_str()),
                ("title", context.novel.title.as_str()),
                ("genre", context.novel.genre.as_str()),
                ("tone", context.novel.tone.as_str()),
                ("premise", context.novel.premise.as_str()),
                ("protagonist_name", context.novel.protagonist_name.as_str()),
                ("language", context.novel.language.as_str()),
                ("goal", plan.goal.as_str()),
                ("conflict", plan.conflict.as_str()),
                ("outcome", plan.outcome.as_str()),
                ("core_memory", context.memory.core_memory.as_str()),
                ("story_memory", context.memory.story_memory.as_str()),
                ("active_memory", context.memory.active_memory.as_str()),
            ],
        ))
    }

    fn dummy_text(&self, context: &AgentContext) -> Result<String> {
        let plan = context
            .scene_plan
            .as_ref()
            .ok_or_else(|| anyhow!("writer requires a scene plan"))?;

        Ok(format!(
            "The protagonist stepped into the scene with a single objective in mind: {goal}. The air around them carried the weight of unfinished business, and every choice sharpened the stakes.\n\n\
The plan faltered when {conflict}. What should have been a clean advance turned into a measured test of nerve, timing, and loyalty.\n\n\
By the end of the scene, {outcome} The victory was real enough to matter, but incomplete enough to demand the next move immediately.",
            goal = plan.goal.as_str(),
            conflict = plan.conflict.to_ascii_lowercase(),
            outcome = plan.outcome.as_str()
        ))
    }
}

impl Agent for WriterAgent {
    fn run(&self, context: &AgentContext) -> Result<AgentRun> {
        if self.use_codex {
            let prompt = self.build_prompt(context)?;
            match self.runner.run_prompt_named("writer", &prompt) {
                Ok(response) => return Ok(AgentRun::direct(response)),
                Err(error) if !context.allow_dummy_fallback => return Err(error),
                Err(error) => {
                    return Ok(AgentRun::fallback(
                        self.dummy_text(context)?,
                        fallback_warning("writer", &error),
                    ));
                }
            }
        }

        Ok(AgentRun::fallback(
            self.dummy_text(context)?,
            "writer used dummy fallback because codex access is disabled by configuration.",
        ))
    }
}
