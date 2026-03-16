pub const SCAFFOLD_DIRS: &[&str] = &[
    "00_Inbox",
    "01_Brief",
    "02_Draft/Scenes",
    "02_Draft/Bundles",
    "02_Draft/Fragments",
    "02_Draft/Illustrations",
    "03_StoryBible/Characters",
    "03_StoryBible/World",
    "03_StoryBible/Rules",
    "03_StoryBible/Timeline",
    "03_StoryBible/Plot",
    "03_StoryBible/Voice",
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
            relative_path: "98_Templates/Bundle Template.md",
            content: BUNDLE_TEMPLATE.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/Character Template.md",
            content: CHARACTER_TEMPLATE.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/Style Guide Template.md",
            content: STYLE_GUIDE_TEMPLATE.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/Tone Guide Template.md",
            content: TONE_GUIDE_TEMPLATE.to_string(),
        },
        WorkspaceFile {
            relative_path: "98_Templates/Narrative Voice Template.md",
            content: NARRATIVE_VOICE_TEMPLATE.to_string(),
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
This workspace separates human-facing manuscript files from PiuroForge runtime data.\n\n\
## First Run Checklist\n\n\
1. Open a terminal once and run `codex login`.\n\
2. Run `piuroforge doctor` in this workspace before the first real scene.\n\
3. Before the first serious scene, fill at least one project brief, one plot note, one character or world note, and one style/tone guide if possible.\n\
4. Treat `scene` as the primary draft unit. In serialized workflows, one scene usually maps to one upload episode.\n\
5. Keep the default scene rhythm inside each internal bundle simple: incident -> escalation -> cliffhanger.\n\
6. If Doctor says ready, your PiuroForge setup is finished. You can move on to `piuroforge next-scene`.\n\
7. Open `~/.config/piuroforge/config.toml` if you want to review your PiuroForge settings.\n\
8. Leave `allow_dummy_fallback = false` for real drafting. Turn it on only if you intentionally want placeholder text while testing the folder workflow.\n\
9. If you want automatic Git history for this novel workspace, turn on `workspace_auto_commit = true` in the same config file.\n\n\
If `next-scene` fails with `codex_unavailable`, that usually means either `codex login` is not finished yet or this machine cannot reach the Codex service because of internet, DNS, VPN, or proxy issues.\n\n\
If you run PiuroForge through another assistant, IDE agent, or sandboxed tool, that host may still ask for its own approval prompts. Those prompts are outside PiuroForge.\n\n\
## Human Folders\n\n\
- `00_Inbox/`: raw captures, scraps, and external notes\n\
- `01_Brief/`: briefs, pitches, and project framing docs\n\
- `02_Draft/Scenes/`: primary draft units tracked in the novel repo; serialized workflows often use one scene per upload episode\n\
- `02_Draft/Bundles/`: compiled internal bundles of multiple scenes tracked in the novel repo\n\
- `02_Draft/Fragments/`: loose fragments and salvageable prose\n\
- `02_Draft/Illustrations/`: art references and illustration notes\n\
- `03_StoryBible/`: characters, world, rules, timeline, and plot docs\n\
- `03_StoryBible/Voice/`: project-level style, tone, and narrative voice guides\n\
- `04_Research/`: sources, notes, and reference extracts\n\
- `05_LLM/`: prompts, outputs, and session exports worth keeping\n\
- `06_Review/Feedback/`: saved review reports for tracked editorial feedback\n\
- `06_Review/Revisions/`: rewrite snapshots and revision records\n\
- `07_Archive/`: snapshots and deprecated materials\n\
- `98_Templates/`: reusable templates for briefs, style guides, scenes, and review passes\n\n\
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
Also lock the default bundle rhythm here before scenes pile up.
";

const DRAFT_README: &str = "\
# Draft

Human-facing manuscript work lives here.

- `Scenes/`: atomic scene files generated or revised during drafting; in many serialized workflows one scene maps to one upload episode
- `Bundles/`: compiled internal manuscripts that bundle multiple scenes
- `Fragments/`: salvageable prose blocks, cut passages, alternates
- `Illustrations/`: prompts, notes, or references for art packages

Generated scene naming:
- `scene_001_001-securing-the-lead.md`
- keep the stable scene id at the front so CLI commands can still target `scene_001_001`

Default scene rhythm inside each internal bundle:
- scene 1 = incident
- scene 2 = escalation
- scene 3 = cliffhanger

Generated bundle naming:
- `bundle_001-securing-the-lead.md`
- bundle slugs are derived from the compiled bundle short title
";

const STORY_BIBLE_README: &str = "\
# Story Bible

Use this folder to separate durable canon from draft prose.

- `Characters/`: cast files, role sheets, arc notes
- `World/`: locations, institutions, technology, cultures
- `Rules/`: invariants, style rules, motif rules, system constraints
- `Timeline/`: chronology, event sequence, date logic
- `Plot/`: arc outlines, bundle maps, structural plans
- `Voice/`: project-level style guides, tone notes, and genre voice rules

Keep these files readable in Git. They should be reference material for future drafting, not runtime logs.
Character files should explicitly capture voice notes, speech rhythm, taboo phrases, and invariants so dialogue does not collapse into one tone.
Use `Voice/` for safe style control through descriptive traits, genre expectations, and tone notes instead of named-author imitation.
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

## Bundle Structure Policy
- Default bundle scene target: 3
- Scene progression: incident -> escalation -> cliffhanger
- When to break the default:

## Story Engine
- What generates the next bundle naturally?
- What keeps pressure on the cast?

## Delivery Targets
- Bundle cadence:
- Scene density:
- Review cadence:
";

const SCENE_TEMPLATE: &str = "\
# Scene

## Scene ID

## Short Title

## Viewpoint

## Bundle Role
- incident / escalation / cliffhanger

## Purpose In Bundle

## Objective

## Conflict

## Turn

## Outcome

## Continuity Notes

## Draft
";

const BUNDLE_TEMPLATE: &str = "\
# Bundle

## Bundle ID

## Short Title

## Bundle Purpose

## Structure Policy
- Scene 1: incident
- Scene 2: escalation
- Scene 3: cliffhanger

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

## Speech Rhythm

## Favorite Diction

## Taboo Phrases

## Emotional Leakage

## Relationship Map

## Non-Negotiable Invariants

## Arc Pressure
";

const STYLE_GUIDE_TEMPLATE: &str = "\
# Style Guide

## Style Principles
- Keep sentences relatively concise:
- Show emotion through action and scene:
- Avoid direct explanation of feeling:

## Repetition Control
- Words or phrases to minimize:
- Sentence structures to vary:

## Description Policy
- How dense description should be:
- How much metaphor is acceptable:

## Dialogue Guidance
- Default dialogue texture:
- How much exposition dialogue can carry:

## Avoid
- Do not imitate named authors directly.
- Do not overuse decorative modifiers.
";

const TONE_GUIDE_TEMPLATE: &str = "\
# Tone Guide

## Tone Targets
- Primary tone:
- Secondary tone:
- Emotional temperature:

## Scene Pressure
- What should create tension:
- What should never deflate tension too early:

## Mood Anchors
- Images or atmospheres to revisit:
- What mood to avoid:

## Genre Style
- Pace expectations:
- Reader-facing promise:
";

const NARRATIVE_VOICE_TEMPLATE: &str = "\
# Narrative Voice

## Narrative Voice
- POV stance:
- Distance from the protagonist:
- Interior monologue density:

## Genre Style
- Contemporary webnovel / literary / thriller / fantasy expectations:
- Readability target:

## Dialogue Mode
- How natural or heightened dialogue should feel:
- How much subtext should sit under speech:

## Safe Style Note
- Use descriptive stylistic traits, tone, and genre expectations instead of named-author imitation.
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
