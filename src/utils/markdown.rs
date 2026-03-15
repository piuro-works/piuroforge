use anyhow::{anyhow, Context, Result};

use crate::models::Scene;

pub fn render_scene(scene: &Scene) -> String {
    format!(
        "# Scene {id}\n\n## Short Title\n{short_title}\n\n## Goal\n{goal}\n\n## Conflict\n{conflict}\n\n## Outcome\n{outcome}\n\n## Status\n{status}\n\n## Text\n{text}\n",
        id = scene.id.as_str(),
        short_title = scene.effective_short_title(),
        goal = scene.goal.trim(),
        conflict = scene.conflict.trim(),
        outcome = scene.outcome.trim(),
        status = scene.status.trim(),
        text = scene.text.trim()
    )
}

pub fn parse_scene(markdown: &str) -> Result<Scene> {
    let mut lines = markdown.lines();
    let header = lines
        .next()
        .ok_or_else(|| anyhow!("scene markdown is empty"))?
        .trim()
        .to_string();
    let id = header
        .strip_prefix("# Scene ")
        .ok_or_else(|| anyhow!("scene markdown is missing '# Scene <id>' header"))?
        .trim()
        .to_string();

    let mut goal = String::new();
    let mut conflict = String::new();
    let mut outcome = String::new();
    let mut short_title = String::new();
    let mut status = String::new();
    let mut text = String::new();
    let mut current_section: Option<&str> = None;

    for line in lines {
        if let Some(section) = line.trim().strip_prefix("## ") {
            current_section = Some(section);
            continue;
        }

        let target = match current_section {
            Some("Short Title") => &mut short_title,
            Some("Goal") => &mut goal,
            Some("Conflict") => &mut conflict,
            Some("Outcome") => &mut outcome,
            Some("Status") => &mut status,
            Some("Text") => &mut text,
            _ => continue,
        };

        if !target.is_empty() {
            target.push('\n');
        }
        target.push_str(line);
    }

    let (chapter, scene_number) = parse_scene_identity(&id)?;

    Ok(Scene {
        id,
        chapter,
        scene_number,
        short_title: short_title.trim().to_string(),
        goal: goal.trim().to_string(),
        conflict: conflict.trim().to_string(),
        outcome: outcome.trim().to_string(),
        text: text.trim().to_string(),
        status: {
            let status = status.trim();
            if status.is_empty() {
                "draft".to_string()
            } else {
                status.to_string()
            }
        },
    })
}

pub fn render_chapter(chapter: u32, short_title: &str, scenes: &[Scene]) -> String {
    let mut content = format!("# Chapter {:03}\n\n", chapter);
    if !short_title.trim().is_empty() {
        content.push_str(&format!("## Short Title\n{}\n\n", short_title.trim()));
    }
    content.push_str(&format!("Compiled from {} scene(s).\n\n", scenes.len()));

    for scene in scenes {
        content.push_str(&format!(
            "## {}: {}\n\n",
            scene.id.as_str(),
            scene.effective_short_title()
        ));
        content.push_str(&format!("Goal: {}\n\n", scene.goal.trim()));
        content.push_str(&format!("Conflict: {}\n\n", scene.conflict.trim()));
        content.push_str(&format!("Outcome: {}\n\n", scene.outcome.trim()));
        content.push_str(&format!("Status: {}\n\n", scene.status.trim()));
        content.push_str(&scene.text);
        content.push_str("\n\n");
    }

    content
}

fn parse_scene_identity(scene_id: &str) -> Result<(u32, u32)> {
    let mut parts = scene_id.split('_');
    let prefix = parts.next().unwrap_or_default();
    let chapter = parts.next().unwrap_or_default();
    let scene = parts.next().unwrap_or_default();

    if prefix != "scene" {
        return Err(anyhow!("invalid scene id '{}'", scene_id));
    }

    let chapter = chapter
        .parse::<u32>()
        .with_context(|| format!("invalid chapter in scene id '{}'", scene_id))?;
    let scene = scene
        .parse::<u32>()
        .with_context(|| format!("invalid scene number in scene id '{}'", scene_id))?;

    Ok((chapter, scene))
}
