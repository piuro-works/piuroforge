pub const SCAFFOLD_DIRS: &[&str] = &[
    "00_Inbox",
    "01_Brief",
    "02_Draft/Scenes",
    "02_Draft/Chapters",
    "02_Draft/Fragments",
    "02_Draft/Illustrations",
    "03_StoryBible/Characters",
    "03_StoryBible/World",
    "03_StoryBible/Rules",
    "03_StoryBible/Timeline",
    "03_StoryBible/Plot",
    "04_Research/Sources",
    "04_Research/Notes",
    "04_Research/References",
    "05_LLM/Prompts",
    "05_LLM/Outputs",
    "05_LLM/Sessions",
    "06_Review/Feedback",
    "06_Review/Revisions",
    "07_Archive/Snapshots",
    "07_Archive/Deprecated",
    "98_Templates",
];

pub struct WorkspaceFile {
    pub relative_path: &'static str,
    pub content: String,
}

pub fn scaffold_files(title: &str) -> Vec<WorkspaceFile> {
    vec![
        WorkspaceFile {
            relative_path: "README.md",
            content: workspace_root_readme(title),
        },
        WorkspaceFile {
            relative_path: "00_Inbox/README.md",
            content: INBOX_README.to_string(),
        },
        WorkspaceFile {
            relative_path: "01_Brief/README.md",
            content: BRIEF_README.to_string(),
        },
        WorkspaceFile {
            relative_path: "02_Draft/README.md",
            content: DRAFT_README.to_string(),
        },
        WorkspaceFile {
            relative_path: "03_StoryBible/README.md",
            content: STORY_BIBLE_README.to_string(),
        },
        WorkspaceFile {
            relative_path: "04_Research/README.md",
            content: RESEARCH_README.to_string(),
        },
        WorkspaceFile {
            relative_path: "05_LLM/README.md",
            content: LLM_README.to_string(),
        },
        WorkspaceFile {
            relative_path: "06_Review/README.md",
            content: REVIEW_README.to_string(),
        },
        WorkspaceFile {
            relative_path: "07_Archive/README.md",
            content: ARCHIVE_README.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/Story Brief Template.md",
            content: STORY_BRIEF_TEMPLATE.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/Scene Template.md",
            content: SCENE_TEMPLATE.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/Chapter Template.md",
            content: CHAPTER_TEMPLATE.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/Character Template.md",
            content: CHARACTER_TEMPLATE.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/Source Template.md",
            content: SOURCE_TEMPLATE.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/Review Pass Template.md",
            content: REVIEW_TEMPLATE.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/LLM Session Template.md",
            content: LLM_SESSION_TEMPLATE.to_string(),
        },
    ]
}

fn workspace_root_readme(title: &str) -> String {
    format!(
        "# {}\n\n\
This workspace separates human-facing manuscript files from HeeForge runtime data.\n\n\
## First Run Checklist\n\n\
1. Open a terminal once and run `codex login`.\n\
2. Run `heeforge doctor` in this workspace before the first real scene.\n\
3. If Doctor says ready, your HeeForge setup is finished. You can move on to `heeforge next-scene`.\n\
4. Open `~/.config/heeforge/config.toml` if you want to review your HeeForge settings.\n\
5. Leave `allow_dummy_fallback = false` for real drafting. Turn it on only if you intentionally want placeholder text while testing the folder workflow.\n\
6. If you want automatic Git history for this novel workspace, turn on `workspace_auto_commit = true` in the same config file.\n\n\
If `next-scene` fails with `codex_unavailable`, that usually means either `codex login` is not finished yet or this machine cannot reach the Codex service because of internet, DNS, VPN, or proxy issues.\n\n\
If you run HeeForge through another assistant, IDE agent, or sandboxed tool, that host may still ask for its own approval prompts. Those prompts are outside HeeForge.\n\n\
## Human Folders\n\n\
- `00_Inbox/`: raw captures, scraps, and external notes\n\
- `01_Brief/`: briefs, pitches, and project framing docs\n\
- `02_Draft/Scenes/`: scene drafts tracked in the novel repo\n\
- `02_Draft/Chapters/`: compiled chapter drafts tracked in the novel repo\n\
- `02_Draft/Fragments/`: loose fragments and salvageable prose\n\
- `02_Draft/Illustrations/`: art references and illustration notes\n\
- `03_StoryBible/`: characters, world, rules, timeline, and plot docs\n\
- `04_Research/`: sources, notes, and reference extracts\n\
- `05_LLM/`: prompts, outputs, and session exports worth keeping\n\
- `06_Review/Feedback/`: saved review reports for tracked editorial feedback\n\
- `06_Review/Revisions/`: rewrite snapshots and revision records\n\
- `07_Archive/`: snapshots and deprecated materials\n\
- `98_Templates/`: reusable templates for briefs, scenes, and review passes\n\n\
## Internal Runtime Data\n\n\
- `.novel/workspace.json`: engine metadata\n\
- `.novel/state/`: machine-managed workflow state\n\
- `.novel/logs/`: runtime generation logs\n\
- `.novel/memory/`: engine memory files\n\n\
Recommended Git model: initialize a Git repository at this workspace root and commit the human-facing folders plus `novel.toml`, while leaving `.novel/state/`, `.novel/logs/`, and transient runtime files ignored.\n"
        ,
        title
    )
}

const INBOX_README: &str = "\
# Inbox

Drop unprocessed material here first.

Suggested contents:
- copied notes
- raw interview or chat logs
- image captions
- scraps that are not yet canon

Move stabilized material into `01_Brief/`, `03_StoryBible/`, or `04_Research/` once it becomes part of the project.
";

const BRIEF_README: &str = "\
# Brief

Use this folder for the project brief, pitch package, or mandate documents.

Suggested file naming:
- `BRIEF-001-Project-Name.md`
- `BRIEF-002-Season-Package.md`

This is the place to lock premise, target length, audience, and voice commitments before drafting accelerates.
";

const DRAFT_README: &str = "\
# Draft

Human-facing manuscript work lives here.

- `Scenes/`: atomic scene files generated or revised during drafting
- `Chapters/`: chapter-level compiled manuscripts
- `Fragments/`: salvageable prose blocks, cut passages, alternates
- `Illustrations/`: prompts, notes, or references for art packages

Generated scene naming:
- `scene_001_001-securing-the-lead.md`
- keep the stable scene id at the front so CLI commands can still target `scene_001_001`

Generated chapter naming:
- `chapter_001-securing-the-lead.md`
- chapter slugs are derived from the compiled chapter short title
";

const STORY_BIBLE_README: &str = "\
# Story Bible

Use this folder to separate durable canon from draft prose.

- `Characters/`: cast files, role sheets, arc notes
- `World/`: locations, institutions, technology, cultures
- `Rules/`: invariants, style rules, motif rules, system constraints
- `Timeline/`: chronology, event sequence, date logic
- `Plot/`: arc outlines, chapter maps, structural plans

Keep these files readable in Git. They should be reference material for future drafting, not runtime logs.
";

const RESEARCH_README: &str = "\
# Research

Keep evidence and interpretation separate.

- `Sources/`: captured source records
- `Notes/`: extracted notes and synthesis
- `References/`: stable reference material worth revisiting

Recommended habit: every factual claim that matters to the manuscript should be traceable to either a source file or a canon decision in the story bible.
";

const LLM_README: &str = "\
# LLM

Use this folder for durable prompt engineering artifacts that are worth versioning.

- `Prompts/`: manually curated prompt text
- `Outputs/`: outputs worth preserving outside runtime logs
- `Sessions/`: summarized session records with decisions and follow-ups

Avoid storing transient engine runtime data here. That belongs in `.novel/`.
";

const REVIEW_README: &str = "\
# Review

Track editorial feedback separately from runtime logs.

- `Feedback/`: review reports, issue lists, editorial diagnostics
- `Revisions/`: before/after snapshots and rewrite records

This folder should read like a real editing history when viewed in Git or a file browser.
";

const ARCHIVE_README: &str = "\
# Archive

Move obsolete or superseded material here instead of deleting it immediately.

- `Snapshots/`: milestone copies worth retaining
- `Deprecated/`: drafts or plans that are no longer active

If a document is no longer live canon but still useful for traceability, archive it here with a dated folder or prefix.
";

const STORY_BRIEF_TEMPLATE: &str = "\
# Story Brief

## Working Title

## One-Sentence Premise

## Core Promise To The Reader

## Primary Genre And Tone

## Target Format
- Length:
- Serialization or standalone:
- POV strategy:

## Protagonist

## Central Conflict

## Voice Commitments
- What the prose must consistently do:
- What the prose must avoid:

## Story Engine
- What generates the next chapter naturally?
- What keeps pressure on the cast?

## Delivery Targets
- Chapter cadence:
- Scene density:
- Review cadence:
";

const SCENE_TEMPLATE: &str = "\
# Scene

## Scene ID

## Short Title

## Viewpoint

## Purpose In Chapter

## Objective

## Conflict

## Turn

## Outcome

## Continuity Notes

## Draft
";

const CHAPTER_TEMPLATE: &str = "\
# Chapter

## Chapter ID

## Short Title

## Chapter Purpose

## Entry Condition

## Escalation Spine

## Turning Point

## Exit Hook

## Scene Inventory
- scene_001_001:

## Draft
";

const CHARACTER_TEMPLATE: &str = "\
# Character

## Character ID

## Role In Story

## Public Surface

## Core Desire

## Fear Or Vulnerability

## Contradiction

## Voice Notes

## Relationship Map

## Non-Negotiable Invariants

## Arc Pressure
";

const SOURCE_TEMPLATE: &str = "\
# Source Record

## Source ID

## Citation

## Access Date

## Reliability Notes

## Extract

## Why It Matters To The Novel

## Canon Decision Triggered By This Source
";

const REVIEW_TEMPLATE: &str = "\
# Review Pass

## Review ID

## Scope

## Primary Goal

## Findings
1. 

## Decisions

## Required Rewrites

## Deferred Issues

## Follow-Up Command Or Prompt
";

const LLM_SESSION_TEMPLATE: &str = "\
# LLM Session

## Session ID

## Date

## Objective

## Inputs
- story bible files:
- draft files:
- review files:

## Prompt Summary

## Output Summary

## Decisions Locked

## Open Questions

## Follow-Ups
";
