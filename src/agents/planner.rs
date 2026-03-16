use anyhow::Result;
use std::sync::Arc;

use crate::agents::base::{fallback_warning, Agent, AgentContext, AgentRun};
use crate::llm_runner::PromptRunner;
use crate::prompts::{render_template, PLANNER_TEMPLATE};

const PROMPT_OPEN_CONFLICTS_KEEP: usize = 10;

#[derive(Clone)]
pub struct PlannerAgent {
    runner: Arc<dyn PromptRunner>,
}

impl PlannerAgent {
    pub fn new(runner: Arc<dyn PromptRunner>) -> Self {
        Self { runner }
    }

    fn build_prompt(&self, context: &AgentContext) -> Result<String> {
        let bundle = context.state.current_bundle;
        let next_scene = context.state.current_scene + 1;
        let bundle_scene_target = context.novel.bundle_scene_target.max(1);
        let previous_scene_goal = context
            .state
            .current_goal
            .clone()
            .unwrap_or_else(|| "None".to_string());
        let previous_scene_outcome = context
            .state
            .current_outcome
            .clone()
            .unwrap_or_else(|| "None".to_string());
        let open_conflicts = render_open_conflicts(&context.state.open_conflicts);
        let bundle_role = crate::models::bundle_role_for(next_scene, bundle_scene_target);

        Ok(render_template(
            PLANNER_TEMPLATE,
            &[
                ("bundle", &bundle.to_string()),
                ("scene_number", &next_scene.to_string()),
                ("bundle_scene_target", &bundle_scene_target.to_string()),
                ("bundle_role", bundle_role.as_str()),
                ("title", context.novel.title.as_str()),
                ("genre", context.novel.genre.as_str()),
                ("tone", context.novel.tone.as_str()),
                ("premise", context.novel.premise.as_str()),
                ("protagonist_name", context.novel.protagonist_name.as_str()),
                ("language", context.novel.language.as_str()),
                ("stage", context.state.stage.as_str()),
                ("previous_scene_goal", previous_scene_goal.as_str()),
                ("previous_scene_outcome", previous_scene_outcome.as_str()),
                ("open_conflicts", open_conflicts.as_str()),
                ("story_foundation", context.story_foundation.as_str()),
                ("core_memory", context.memory.core_memory.as_str()),
                ("story_memory", context.memory.story_memory.as_str()),
                ("active_memory", context.memory.active_memory.as_str()),
            ],
        ))
    }

    fn dummy_plan(&self, context: &AgentContext) -> String {
        let bundle = context.state.current_bundle;
        let scene_number = context.state.current_scene + 1;

        format!(
            "{{\n  \"bundle\": {bundle},\n  \"scene_number\": {scene_number},\n  \"short_title\": \"Securing the Lead\",\n  \"goal\": \"The protagonist secures a concrete lead that moves the current arc forward.\",\n  \"conflict\": \"A trusted ally withholds a critical detail until the protagonist proves commitment.\",\n  \"outcome\": \"The protagonist earns partial trust, but the missing detail opens a larger threat.\"\n}}",
            bundle = bundle,
            scene_number = scene_number
        )
    }
}

impl Agent for PlannerAgent {
    fn run(&self, context: &AgentContext) -> Result<AgentRun> {
        let prompt = self.build_prompt(context)?;
        match self.runner.run_prompt_named("planner", &prompt) {
            Ok(response) => Ok(AgentRun::direct(response)),
            Err(error) if !context.allow_dummy_fallback => Err(error),
            Err(error) => Ok(AgentRun::fallback(
                self.dummy_plan(context),
                fallback_warning("planner", &error),
            )),
        }
    }
}

fn render_open_conflicts(conflicts: &[String]) -> String {
    if conflicts.is_empty() {
        return "None".to_string();
    }

    if conflicts.len() <= PROMPT_OPEN_CONFLICTS_KEEP {
        return conflicts.join(" | ");
    }

    let retained = &conflicts[conflicts.len() - PROMPT_OPEN_CONFLICTS_KEEP..];
    format!(
        "latest {} of {}: {}",
        PROMPT_OPEN_CONFLICTS_KEEP,
        conflicts.len(),
        retained.join(" | ")
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;

    use super::{PlannerAgent, render_open_conflicts};
    use crate::agents::base::AgentContext;
    use crate::config::NovelSettings;
    use crate::llm_runner::PromptRunner;
    use crate::models::{MemoryBundle, StoryState};

    struct NoopRunner;

    impl PromptRunner for NoopRunner {
        fn run_prompt_named(&self, _label: &str, _prompt: &str) -> Result<String> {
            unreachable!("prompt runner should not be called in planner prompt tests")
        }
    }

    #[test]
    fn keeps_open_conflicts_unchanged_when_short() {
        let conflicts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(render_open_conflicts(&conflicts), "a | b | c");
    }

    #[test]
    fn reduces_open_conflicts_to_recent_window_when_long() {
        let conflicts = (1..=14)
            .map(|index| format!("conflict-{index}"))
            .collect::<Vec<_>>();

        let rendered = render_open_conflicts(&conflicts);
        assert!(rendered.contains("latest 10 of 14"));
        assert!(rendered.contains("conflict-14"));
        assert!(!rendered.contains("conflict-1 |"));
        assert!(!rendered.contains("conflict-2 |"));
    }

    #[test]
    fn build_prompt_includes_previous_scene_context_and_transition_rules() {
        let planner = PlannerAgent::new(Arc::new(NoopRunner));
        let mut state = StoryState::default();
        state.current_bundle = 1;
        state.current_scene = 1;
        state.stage = "scene_approved".to_string();
        state.current_goal = Some("Keep the last ration and pass.".to_string());
        state.current_outcome = Some("The ration and pass are lost, so the protagonist must flee."
            .to_string());
        state.open_conflicts = vec!["The city tightens registration and marks non-humans.".to_string()];

        let context = AgentContext {
            state,
            novel: NovelSettings {
                title: "Test".to_string(),
                genre: "Fantasy".to_string(),
                tone: "Harsh".to_string(),
                premise: "Escape the city.".to_string(),
                protagonist_name: "Ulaanbaatar".to_string(),
                language: "ko".to_string(),
                ..NovelSettings::default()
            },
            memory: MemoryBundle::default(),
            story_foundation: "Episode 2 should move into the drainage alleys, not retry the gate."
                .to_string(),
            scene_plan: None,
            scene: None,
            instruction: None,
            allow_dummy_fallback: false,
        };

        let prompt = planner.build_prompt(&context).expect("planner prompt should render");

        assert!(prompt.contains("Previous scene goal: Keep the last ration and pass."));
        assert!(prompt.contains(
            "Previous scene outcome: The ration and pass are lost, so the protagonist must flee."
        ));
        assert!(prompt.contains("do not simply retry the exact same tactical objective"));
        assert!(prompt.contains("change at least one of these between consecutive scenes"));
    }
}
