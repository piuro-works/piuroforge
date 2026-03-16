use anyhow::Result;
use clap::Args;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::config::Config;
use crate::engine::NovelEngine;
use crate::output::{CommandOutput, OutputFormat};

#[derive(Debug, Clone, Args)]
pub struct InitCommand {
    #[arg(help = "Path to the novel workspace to create or refresh.")]
    pub path: Option<PathBuf>,
    #[arg(long, help = "Novel title written to novel.toml.")]
    pub title: Option<String>,
    #[arg(long, help = "Primary genre for planning and drafting prompts.")]
    pub genre: Option<String>,
    #[arg(long, help = "Desired narrative tone or style guidance.")]
    pub tone: Option<String>,
    #[arg(long, help = "One-line premise or story hook.")]
    pub premise: Option<String>,
    #[arg(long = "protagonist", help = "Main protagonist name.")]
    pub protagonist_name: Option<String>,
    #[arg(long, help = "Output language for generated prose.")]
    pub language: Option<String>,
    #[arg(
        long,
        default_value_t = false,
        help = "Do not ask interactive questions for missing fields."
    )]
    pub no_input: bool,
}

pub fn prepare_config(
    config: &mut Config,
    command: &InitCommand,
    format: OutputFormat,
) -> Result<()> {
    apply_option(&mut config.novel_settings.title, command.title.as_ref());
    apply_option(&mut config.novel_settings.genre, command.genre.as_ref());
    apply_option(&mut config.novel_settings.tone, command.tone.as_ref());
    apply_option(&mut config.novel_settings.premise, command.premise.as_ref());
    apply_option(
        &mut config.novel_settings.protagonist_name,
        command.protagonist_name.as_ref(),
    );
    apply_option(
        &mut config.novel_settings.language,
        command.language.as_ref(),
    );

    if command.no_input || format == OutputFormat::Json {
        return Ok(());
    }

    let workspace_name = config.workspace_name();
    let default_language = config.global_settings.default_language.clone();

    prompt_if_missing(
        "Title",
        &mut config.novel_settings.title,
        Some(workspace_name.as_str()),
        true,
    )?;
    prompt_if_missing(
        "Genre",
        &mut config.novel_settings.genre,
        Some("Mystery"),
        true,
    )?;
    prompt_if_missing(
        "Tone",
        &mut config.novel_settings.tone,
        Some("Focused, cinematic, character-driven"),
        true,
    )?;
    prompt_if_missing("Premise", &mut config.novel_settings.premise, None, true)?;
    prompt_if_missing(
        "Protagonist name",
        &mut config.novel_settings.protagonist_name,
        None,
        true,
    )?;
    prompt_if_missing(
        "Language",
        &mut config.novel_settings.language,
        Some(default_language.as_str()),
        true,
    )?;

    Ok(())
}

pub fn run(engine: &NovelEngine) -> Result<CommandOutput> {
    engine.bootstrap_workspace()?;

    let missing = engine.missing_required_novel_fields();
    let mut output = if missing.is_empty() {
        let output = CommandOutput::ok(
            "init",
            engine.workspace_dir(),
            "Novel workspace initialized and ready for scene generation.",
        )
        .next_step("codex login")
        .next_step(super::workspace_command(engine, "doctor"))
        .next_step(format!("Open {}", engine.global_config_path().display()))
        .next_step(super::workspace_command(engine, "next-scene"));
        if engine.workspace_auto_commit_enabled() {
            output
        } else {
            output.next_step("git init")
        }
    } else {
        CommandOutput::ok(
            "init",
            engine.workspace_dir(),
            "Novel workspace initialized, but required novel metadata is still incomplete.",
        )
        .warning(format!(
            "Missing required novel config: {}",
            missing.join(", ")
        ))
        .next_step(super::workspace_command(engine, "doctor"))
        .next_step(format!("Edit {}", engine.workspace_config_path().display()))
        .next_step(super::workspace_command(engine, "next-scene"))
    };

    output = output
        .detail("title", engine.novel_title())
        .detail(
            "workspace_readme",
            engine.workspace_readme_path().display().to_string(),
        )
        .detail(
            "global_config",
            engine.global_config_path().display().to_string(),
        )
        .detail(
            "writer_setup",
            "Run `codex login` once, then run `heeforge doctor`.",
        )
        .detail(
            "setup_done_when",
            "If `heeforge doctor` says ready, HeeForge setup is finished and you can draft.",
        )
        .detail(
            "hosted_agent_note",
            "If you run HeeForge through another assistant or sandboxed tool, that host may still ask for its own approval prompts. Those prompts do not come from HeeForge itself.",
        )
        .detail(
            "workspace_config",
            engine.workspace_config_path().display().to_string(),
        )
        .detail("runtime_data", engine.novel_dir().display().to_string())
        .artifact("workspace_manifest", engine.workspace_manifest_path())
        .artifact("workspace_readme", engine.workspace_readme_path())
        .artifact("workspace_config", engine.workspace_config_path())
        .artifact("global_config", engine.global_config_path());

    Ok(super::finalize_workspace_change(
        engine,
        output,
        "heeforge: initialize workspace",
    ))
}

fn apply_option(target: &mut String, value: Option<&String>) {
    if let Some(value) = value {
        *target = value.trim().to_string();
    }
}

fn prompt_if_missing(
    label: &str,
    target: &mut String,
    default: Option<&str>,
    required: bool,
) -> Result<()> {
    if !target.trim().is_empty() {
        return Ok(());
    }

    loop {
        match default {
            Some(default) if !default.is_empty() => {
                print!("{label} [{default}]: ");
            }
            _ => {
                print!("{label}: ");
            }
        }
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            if let Some(default) = default.filter(|value| !value.is_empty()) {
                *target = default.to_string();
                return Ok(());
            }
            if !required {
                return Ok(());
            }
            continue;
        }

        *target = input.to_string();
        return Ok(());
    }
}
