use anyhow::{anyhow, Result};

use crate::agents::base::{Agent, AgentContext};
use crate::codex_runner::CodexRunner;
use crate::prompts::{render_template, PLANNER_TEMPLATE};

#[derive(Debug, Clone)]
pub struct PlannerAgent {
    runner: CodexRunner,
    use_codex: bool,
}

impl PlannerAgent {
    pub fn new(runner: CodexRunner, use_codex: bool) -> Self {
        Self { runner, use_codex }
    }

    fn build_prompt(&self, context: &AgentContext) -> Result<String> {
        let chapter = context.state.current_chapter;
        let next_scene = context.state.current_scene + 1;
        let current_goal = context
            .state
            .current_goal
            .clone()
            .unwrap_or_else(|| "None".to_string());
        let open_conflicts = if context.state.open_conflicts.is_empty() {
            "None".to_string()
        } else {
            context.state.open_conflicts.join(" | ")
        };

        Ok(render_template(
            PLANNER_TEMPLATE,
            &[
                ("chapter", &chapter.to_string()),
                ("scene_number", &next_scene.to_string()),
                ("title", context.novel.title.as_str()),
                ("genre", context.novel.genre.as_str()),
                ("tone", context.novel.tone.as_str()),
                ("premise", context.novel.premise.as_str()),
                ("protagonist_name", context.novel.protagonist_name.as_str()),
                ("language", context.novel.language.as_str()),
                ("stage", context.state.stage.as_str()),
                ("current_goal", current_goal.as_str()),
                ("open_conflicts", open_conflicts.as_str()),
                ("core_memory", context.memory.core_memory.as_str()),
                ("story_memory", context.memory.story_memory.as_str()),
                ("active_memory", context.memory.active_memory.as_str()),
            ],
        ))
    }

    fn dummy_plan(&self, context: &AgentContext) -> String {
        let chapter = context.state.current_chapter;
        let scene_number = context.state.current_scene + 1;

        format!(
            "{{\n  \"chapter\": {chapter},\n  \"scene_number\": {scene_number},\n  \"goal\": \"The protagonist secures a concrete lead that moves the current arc forward.\",\n  \"conflict\": \"A trusted ally withholds a critical detail until the protagonist proves commitment.\",\n  \"outcome\": \"The protagonist earns partial trust, but the missing detail opens a larger threat.\"\n}}",
            chapter = chapter,
            scene_number = scene_number
        )
    }
}

impl Agent for PlannerAgent {
    fn run(&self, context: &AgentContext) -> Result<String> {
        if self.use_codex {
            let prompt = self.build_prompt(context)?;
            match self.runner.run_prompt(&prompt) {
                Ok(response) => return Ok(response),
                Err(error) if !context.allow_dummy_fallback => return Err(error),
                Err(_) => {}
            }
        }

        if context.allow_dummy_fallback {
            return Ok(self.dummy_plan(context));
        }

        Err(anyhow!("planner agent could not produce a scene plan"))
    }
}
