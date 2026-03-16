use anyhow::{anyhow, Result};

use crate::agents::base::{fallback_warning, Agent, AgentContext, AgentRun};
use crate::codex_runner::CodexRunner;
use crate::prompts::{render_template, EDITOR_TEMPLATE};

#[derive(Debug, Clone)]
pub struct EditorAgent {
    runner: CodexRunner,
    use_codex: bool,
}

impl EditorAgent {
    pub fn new(runner: CodexRunner, use_codex: bool) -> Self {
        Self { runner, use_codex }
    }

    fn build_prompt(&self, context: &AgentContext) -> Result<String> {
        let scene = context
            .scene
            .as_ref()
            .ok_or_else(|| anyhow!("editor requires a scene"))?;
        let instruction = context
            .instruction
            .clone()
            .unwrap_or_else(|| "Tighten repetition and polish sentence flow.".to_string());
        let chapter_role = scene.effective_chapter_role(context.novel.chapter_scene_target);

        Ok(render_template(
            EDITOR_TEMPLATE,
            &[
                ("title", context.novel.title.as_str()),
                ("tone", context.novel.tone.as_str()),
                ("premise", context.novel.premise.as_str()),
                ("protagonist_name", context.novel.protagonist_name.as_str()),
                ("chapter_role", chapter_role.as_str()),
                ("instruction", instruction.as_str()),
                ("goal", scene.goal.as_str()),
                ("conflict", scene.conflict.as_str()),
                ("outcome", scene.outcome.as_str()),
                ("story_foundation", context.story_foundation.as_str()),
                ("text", scene.text.as_str()),
            ],
        ))
    }

    fn dummy_edit(&self, context: &AgentContext) -> Result<String> {
        let scene = context
            .scene
            .as_ref()
            .ok_or_else(|| anyhow!("editor requires a scene"))?;
        let instruction = context
            .instruction
            .clone()
            .unwrap_or_else(|| "Tighten repetition and polish sentence flow.".to_string());

        let mut text = scene.text.replace("  ", " ");
        if !text.ends_with('.') && !text.ends_with('!') && !text.ends_with('?') {
            text.push('.');
        }

        if context.instruction.is_some() {
            text.push_str(&format!(
                "\n\nThe revision now leans harder into {}",
                instruction.trim().trim_end_matches('.')
            ));
            if !text.ends_with('.') {
                text.push('.');
            }
        }

        Ok(text)
    }
}

impl Agent for EditorAgent {
    fn run(&self, context: &AgentContext) -> Result<AgentRun> {
        if self.use_codex {
            let prompt = self.build_prompt(context)?;
            match self.runner.run_prompt_named("editor", &prompt) {
                Ok(response) => return Ok(AgentRun::direct(response)),
                Err(error) if !context.allow_dummy_fallback => return Err(error),
                Err(error) => {
                    return Ok(AgentRun::fallback(
                        self.dummy_edit(context)?,
                        fallback_warning("editor", &error),
                    ));
                }
            }
        }

        Ok(AgentRun::fallback(
            self.dummy_edit(context)?,
            "editor used dummy fallback because codex access is disabled by configuration.",
        ))
    }
}
