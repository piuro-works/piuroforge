use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

use heeforge::config::{WORKSPACE_DIR_NAME, WORKSPACE_MANIFEST_FILE};
use heeforge::output::{emit_command, emit_error, CommandOutput, ErrorOutput, OutputFormat};
use heeforge::{commands, Config, NovelEngine};

const CLI_AFTER_HELP: &str = "\
Quickstart:
  heeforge init ~/novels/my-book
  cd ~/novels/my-book
  heeforge doctor
  heeforge next-scene
  heeforge review

Automation:
  heeforge --workspace ~/novels/my-book --format json status
  heeforge --workspace ~/novels/my-book --format json next-scene
";

const INIT_AFTER_HELP: &str = "\
Examples:
  heeforge init ~/novels/my-book
  heeforge init ~/novels/my-book --title \"기억 편집자\" --genre Mystery --tone \"Tense\" --premise \"...\" --protagonist \"윤서\" --language ko
  heeforge --format json init ~/novels/my-book --no-input
";

const STATUS_AFTER_HELP: &str = "\
Examples:
  heeforge status
  heeforge --workspace ~/novels/my-book --format json status
";

const DOCTOR_AFTER_HELP: &str = "\
Examples:
  heeforge doctor
  heeforge --workspace ~/novels/my-book --format json doctor
";

const NEXT_SCENE_AFTER_HELP: &str = "\
Examples:
  heeforge next-scene
  heeforge --workspace ~/novels/my-book --format json next-scene
";

const REVIEW_AFTER_HELP: &str = "\
Examples:
  heeforge review
  heeforge --workspace ~/novels/my-book --format json review
";

const REWRITE_AFTER_HELP: &str = "\
Examples:
  heeforge rewrite scene_001_001 --instruction \"대사를 더 날카롭게\"
  heeforge --workspace ~/novels/my-book --format json rewrite scene_001_001 --instruction \"Compress repeated exposition\"
";

const APPROVE_AFTER_HELP: &str = "\
Examples:
  heeforge approve scene_001_001
  heeforge --workspace ~/novels/my-book --format json approve scene_001_001
";

const NEXT_CHAPTER_AFTER_HELP: &str = "\
Examples:
  heeforge next-chapter
  heeforge --workspace ~/novels/my-book --format json next-chapter
";

const EXPAND_WORLD_AFTER_HELP: &str = "\
Examples:
  heeforge expand-world
  heeforge --workspace ~/novels/my-book --format json expand-world
";

const MEMORY_AFTER_HELP: &str = "\
Examples:
  heeforge memory
  heeforge --workspace ~/novels/my-book --format json memory
";

const SHOW_AFTER_HELP: &str = "\
Examples:
  heeforge show scene_001_001
  heeforge --workspace ~/novels/my-book --format json show scene_001_001
";

#[derive(Debug, Parser)]
#[command(
    name = "heeforge",
    version,
    about = "HeeForge CLI novel engine for one workspace = one novel.",
    long_about = "HeeForge creates one novel workspace, then generates, reviews, rewrites, approves, and compiles scenes from the terminal. Use `--format json` when another LLM agent needs stable, machine-readable output.",
    after_long_help = CLI_AFTER_HELP
)]
struct Cli {
    #[arg(
        long,
        global = true,
        help = "Path to a single novel workspace. If omitted, the CLI auto-detects the nearest workspace."
    )]
    workspace: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        value_enum,
        default_value_t = OutputFormat::Text,
        help = "Output mode. Use `json` for Codex CLI, OpenClaw, or other LLM agents."
    )]
    format: OutputFormat,
    #[command(subcommand)]
    command: NovelCommand,
}

#[derive(Debug, Subcommand)]
enum NovelCommand {
    #[command(
        about = "Create or refresh a single novel workspace.",
        long_about = "Initialize one novel workspace, write config files, and optionally collect required novel metadata interactively. In JSON mode, interactive prompts are skipped automatically unless values are supplied as flags.",
        after_long_help = INIT_AFTER_HELP
    )]
    Init(commands::init::InitCommand),
    #[command(
        about = "Show current workspace progress and next recommended action.",
        after_long_help = STATUS_AFTER_HELP
    )]
    Status,
    #[command(
        about = "Diagnose workspace setup, Codex connectivity, and draft readiness.",
        after_long_help = DOCTOR_AFTER_HELP
    )]
    Doctor,
    #[command(
        about = "Generate the next scene by running planner, writer, and editor.",
        after_long_help = NEXT_SCENE_AFTER_HELP
    )]
    NextScene,
    #[command(
        about = "Review the current scene and save a JSON issue report.",
        after_long_help = REVIEW_AFTER_HELP
    )]
    Review,
    #[command(
        about = "Rewrite a scene while preserving original and revised snapshots.",
        after_long_help = REWRITE_AFTER_HELP
    )]
    Rewrite {
        scene_id: String,
        #[arg(long)]
        instruction: String,
    },
    #[command(
        about = "Mark a scene as approved.",
        after_long_help = APPROVE_AFTER_HELP
    )]
    Approve { scene_id: String },
    #[command(
        about = "Compile the current chapter markdown after validating scene order.",
        after_long_help = NEXT_CHAPTER_AFTER_HELP
    )]
    NextChapter,
    #[command(
        about = "Append one new worldbuilding section to story memory.",
        after_long_help = EXPAND_WORLD_AFTER_HELP
    )]
    ExpandWorld,
    #[command(
        about = "Show the current memory bundle.",
        after_long_help = MEMORY_AFTER_HELP
    )]
    Memory,
    #[command(
        about = "Show one scene and its current text.",
        after_long_help = SHOW_AFTER_HELP
    )]
    Show { scene_id: String },
}

impl NovelCommand {
    fn name(&self) -> &'static str {
        match self {
            Self::Init(_) => "init",
            Self::Status => "status",
            Self::Doctor => "doctor",
            Self::NextScene => "next-scene",
            Self::Review => "review",
            Self::Rewrite { .. } => "rewrite",
            Self::Approve { .. } => "approve",
            Self::NextChapter => "next-chapter",
            Self::ExpandWorld => "expand-world",
            Self::Memory => "memory",
            Self::Show { .. } => "show",
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let format = cli.format;
    let command_name = cli.command.name().to_string();
    let workspace_hint = resolve_workspace_path(&cli).ok();

    match run(cli) {
        Ok(output) => {
            if let Err(error) = emit_command(&output, format) {
                eprintln!("failed to render command output: {error}");
                process::exit(1);
            }
        }
        Err(error) => {
            let payload = ErrorOutput::from_error(&command_name, workspace_hint.as_deref(), &error);
            if let Err(render_error) = emit_error(&payload, format) {
                eprintln!("failed to render error output: {render_error}");
                eprintln!("{}", payload.render_text());
            }
            process::exit(1);
        }
    }
}

fn run(cli: Cli) -> Result<CommandOutput> {
    let workspace = resolve_workspace_path(&cli)?;

    match cli.command {
        NovelCommand::Init(command) => {
            let mut config = Config::new(workspace)?;
            commands::init::prepare_config(&mut config, &command, cli.format)?;
            let engine = NovelEngine::new(config)?;
            commands::init::run(&engine)
        }
        NovelCommand::Status => {
            let engine = NovelEngine::new(Config::new(workspace)?)?;
            commands::status::run(&engine)
        }
        NovelCommand::Doctor => {
            let config = Config::new(workspace)?;
            commands::doctor::run(&config)
        }
        NovelCommand::NextScene => {
            let engine = NovelEngine::new(Config::new(workspace)?)?;
            commands::next_scene::run(&engine)
        }
        NovelCommand::Review => {
            let engine = NovelEngine::new(Config::new(workspace)?)?;
            commands::review::run(&engine)
        }
        NovelCommand::Rewrite {
            scene_id,
            instruction,
        } => {
            let engine = NovelEngine::new(Config::new(workspace)?)?;
            commands::rewrite::run(&engine, &scene_id, &instruction)
        }
        NovelCommand::Approve { scene_id } => {
            let engine = NovelEngine::new(Config::new(workspace)?)?;
            commands::approve::run(&engine, &scene_id)
        }
        NovelCommand::NextChapter => {
            let engine = NovelEngine::new(Config::new(workspace)?)?;
            commands::next_chapter::run(&engine)
        }
        NovelCommand::ExpandWorld => {
            let engine = NovelEngine::new(Config::new(workspace)?)?;
            commands::expand_world::run(&engine)
        }
        NovelCommand::Memory => {
            let engine = NovelEngine::new(Config::new(workspace)?)?;
            commands::memory::run(&engine)
        }
        NovelCommand::Show { scene_id } => {
            let engine = NovelEngine::new(Config::new(workspace)?)?;
            commands::show::run(&engine, &scene_id)
        }
    }
}

fn resolve_workspace_path(cli: &Cli) -> Result<PathBuf> {
    if let NovelCommand::Init(command) = &cli.command {
        if let Some(path) = &command.path {
            return normalize_path(path.clone());
        }
    }

    if let Some(workspace) = &cli.workspace {
        return normalize_path(workspace.clone());
    }

    let current_dir = std::env::current_dir()?;
    Ok(detect_workspace_root(current_dir.clone()).unwrap_or(current_dir))
}

fn normalize_path(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(std::env::current_dir()?.join(path))
}

fn detect_workspace_root(start: PathBuf) -> Option<PathBuf> {
    let mut current = start;

    loop {
        let marker = current
            .join(WORKSPACE_DIR_NAME)
            .join(WORKSPACE_MANIFEST_FILE);
        if marker.exists() {
            return Some(current);
        }

        if !current.pop() {
            return None;
        }
    }
}
