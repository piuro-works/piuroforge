use anyhow::Result;

use crate::engine::NovelEngine;
use crate::models::bundle_role_for;
use crate::output::CommandOutput;

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    let state = engine.get_status()?;
    let missing = engine.missing_required_novel_fields();
    let foundation = engine.story_foundation_status()?;
    let bundle_scene_target = engine.bundle_scene_target();
    let serialized_workflow = engine.serialized_workflow_enabled();
    let next_scene_role = if state.current_scene < bundle_scene_target {
        bundle_role_for(state.current_scene + 1, bundle_scene_target)
    } else {
        "-".to_string()
    };

    let summary = if !missing.is_empty() {
        "Workspace scaffold exists, but novel config is still incomplete."
    } else if state.current_scene >= bundle_scene_target {
        if serialized_workflow {
            if state.stage == "scene_approved" {
                "Current serialized scene bundle is full. The next scene will start a new internal bundle automatically."
            } else {
                "Current serialized scene bundle is full. Approve the current scene before drafting the next serialized scene."
            }
        } else {
            "Bundle scene target is full. Compile the bundle before drafting more scenes."
        }
    } else if foundation.score < 40 {
        "Workspace is technically ready, but the story foundation is still skeletal."
    } else if foundation.score < 60 {
        "Workspace is ready, but the story foundation is still thin."
    } else if state.current_scene_id.is_none() {
        "Workspace is ready for the first scene."
    } else if state.stage == "scene_approved" {
        "Current scene is approved. You can draft the next scene or compile a bundle."
    } else {
        "Current scene is in progress."
    };

    let mut output = CommandOutput::ok("status", engine.workspace_dir(), summary)
        .detail("arc", state.current_arc.to_string())
        .detail("bundle", state.current_bundle.to_string())
        .detail("scene", state.current_scene.to_string())
        .detail("bundle_scene_target", bundle_scene_target.to_string())
        .detail("serialized_workflow", serialized_workflow.to_string())
        .detail(
            "bundle_progress",
            format!("{}/{}", state.current_scene, bundle_scene_target),
        )
        .detail("next_scene_role", next_scene_role)
        .detail("stage", state.stage.clone())
        .detail(
            "current_scene_id",
            state.current_scene_id.as_deref().unwrap_or("-"),
        )
        .detail("current_goal", state.current_goal.as_deref().unwrap_or("-"))
        .detail(
            "open_conflict_count",
            state.open_conflicts.len().to_string(),
        )
        .detail("foundation_score", foundation.score.to_string())
        .detail("foundation_level", foundation.level())
        .detail("brief_doc_count", foundation.brief_docs.to_string())
        .detail(
            "story_bible_doc_count",
            (foundation.character_docs + foundation.world_docs + foundation.voice_docs).to_string(),
        )
        .detail("plot_doc_count", foundation.plot_docs.to_string())
        .detail("voice_doc_count", foundation.voice_docs.to_string())
        .detail("research_doc_count", foundation.research_docs.to_string())
        .artifact("workspace_config", engine.workspace_config_path());

    if !state.open_conflicts.is_empty() {
        output = output.body(super::sentence_list(&state.open_conflicts));
    }

    if !missing.is_empty() {
        output = output
            .warning(format!(
                "Missing required novel config: {}",
                missing.join(", ")
            ))
            .next_step(super::workspace_command(engine, "doctor"))
            .next_step(format!("Edit {}", engine.workspace_config_path().display()));
    } else {
        if foundation.score < 60 {
            for missing_item in &foundation.missing_items {
                output = output.warning(format!(
                    "Story foundation is {} ({}/100): missing {}.",
                    foundation.level(),
                    foundation.score,
                    missing_item
                ));
            }

            output = output.next_step(format!(
                "Add or expand docs in {}/01_Brief and {}/03_StoryBible before the next serious draft",
                engine.workspace_dir().display(),
                engine.workspace_dir().display()
            ));
        }

        if state.current_scene >= bundle_scene_target {
            if serialized_workflow {
                if state.stage == "scene_approved" {
                    output = output
                        .warning(format!(
                            "Internal bundle {:03} already has {} scene(s). The next `next-scene` call will roll into bundle {:03} automatically.",
                            state.current_bundle,
                            bundle_scene_target,
                            state.current_bundle + 1
                        ))
                        .next_step(super::workspace_command(engine, "next-scene"))
                        .next_step(super::workspace_command(engine, "next-bundle"));
                } else {
                    output = output.warning(format!(
                        "Internal bundle {:03} already has {} scene(s). Review and approve the current scene before drafting the next serialized scene.",
                        state.current_bundle,
                        bundle_scene_target
                    ));
                    if let Some(scene_id) = state.current_scene_id.as_deref() {
                        output = output
                            .next_step(super::workspace_command(
                                engine,
                                &format!("show {scene_id}"),
                            ))
                            .next_step(super::workspace_command(engine, "review"))
                            .next_step(super::workspace_command(
                                engine,
                                &format!("approve {scene_id}"),
                            ));
                    }
                }
            } else {
                output = output
                    .warning(format!(
                        "Bundle {:03} already has {} scene(s), which matches the target. Compile it before drafting scene {:03}.",
                        state.current_bundle,
                        bundle_scene_target,
                        bundle_scene_target + 1
                    ))
                    .next_step(super::workspace_command(engine, "next-bundle"));

                if let Some(scene_id) = state.current_scene_id.as_deref() {
                    output = output.next_step(super::workspace_command(
                        engine,
                        &format!("show {scene_id}"),
                    ));
                }
            }
        } else if let Some(scene_id) = state.current_scene_id.as_deref() {
            output = output
                .next_step(super::workspace_command(
                    engine,
                    &format!("show {scene_id}"),
                ))
                .next_step(super::workspace_command(engine, "doctor"))
                .next_step(super::workspace_command(engine, "review"));
        } else {
            output = output
                .next_step(super::workspace_command(engine, "doctor"))
                .next_step(super::workspace_command(engine, "next-scene"));
        }
    }

    Ok(output)
}
