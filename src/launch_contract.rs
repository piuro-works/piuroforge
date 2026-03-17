use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::NovelSettings;
use crate::utils::files::list_markdown_files;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchContractSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchContractIssue {
    pub severity: LaunchContractSeverity,
    pub code: String,
    pub message: String,
    pub remediation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchContractReport {
    pub enabled: bool,
    pub primary_plot_path: Option<PathBuf>,
    pub required_beats: Vec<String>,
    pub issues: Vec<LaunchContractIssue>,
}

impl LaunchContractReport {
    pub fn has_blocking_issues(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == LaunchContractSeverity::Error)
    }

    pub fn status_label(&self) -> &'static str {
        if !self.enabled {
            "disabled"
        } else if self.required_beats.is_empty() {
            "empty"
        } else if self.has_blocking_issues() {
            "blocking_issues"
        } else if self
            .issues
            .iter()
            .any(|issue| issue.severity == LaunchContractSeverity::Warning)
        {
            "warnings"
        } else {
            "ok"
        }
    }

    pub fn required_beats_summary(&self) -> String {
        if self.required_beats.is_empty() {
            "none".to_string()
        } else {
            self.required_beats.join(", ")
        }
    }

    pub fn blocking_messages(&self) -> Vec<String> {
        self.issues
            .iter()
            .filter(|issue| issue.severity == LaunchContractSeverity::Error)
            .map(|issue| issue.message.clone())
            .collect()
    }
}

pub fn validate_launch_contract(
    workspace_dir: &Path,
    novel_settings: &NovelSettings,
) -> Result<LaunchContractReport> {
    let contract = &novel_settings.launch_contract;
    let required_beats = render_required_beats(contract);
    if !contract.enabled {
        return Ok(LaunchContractReport {
            enabled: false,
            primary_plot_path: None,
            required_beats,
            issues: Vec::new(),
        });
    }

    let mut issues = Vec::new();
    if contract.is_empty() {
        if novel_settings.serialized_workflow {
            issues.push(LaunchContractIssue {
                severity: LaunchContractSeverity::Warning,
                code: "empty_launch_contract".to_string(),
                message: "launch_contract is enabled but empty. PiuroForge cannot verify early serialized hook beats until you fill must_show_by_scene_*.".to_string(),
                remediation: format!("Edit {} and add must_show_by_scene_* beats.", workspace_dir.join("novel.toml").display()),
            });
        }

        return Ok(LaunchContractReport {
            enabled: true,
            primary_plot_path: None,
            required_beats,
            issues,
        });
    }

    let primary_spine = find_primary_launch_spine(&workspace_dir.join("03_StoryBible").join("Plot"))?;
    let primary_plot_path = primary_spine.as_ref().map(|spine| spine.path.clone());

    let Some(spine) = primary_spine else {
        issues.push(LaunchContractIssue {
            severity: LaunchContractSeverity::Error,
            code: "missing_launch_spine".to_string(),
            message: "launch_contract is enabled, but no numbered launch spine was found in 03_StoryBible/Plot.".to_string(),
            remediation: format!(
                "Add a `1. 2. 3.` episode spine to an early launch plot file under {}.",
                workspace_dir.join("03_StoryBible/Plot").display()
            ),
        });
        return Ok(LaunchContractReport {
            enabled: true,
            primary_plot_path,
            required_beats,
            issues,
        });
    };

    for (deadline_scene, beat_id) in contract_requirements(contract) {
        let Some(keywords) = beat_keywords(beat_id.as_str()) else {
            issues.push(LaunchContractIssue {
                severity: LaunchContractSeverity::Warning,
                code: "unknown_launch_beat".to_string(),
                message: format!(
                    "launch_contract beat `{}` is unknown. Known beats: escape, larzesh, forced_companionship, relic_hint, golem_hint.",
                    beat_id
                ),
                remediation: format!("Edit {} and use one of the known beat ids.", workspace_dir.join("novel.toml").display()),
            });
            continue;
        };

        let match_scene = spine
            .items
            .iter()
            .find(|item| line_matches_keywords(item.text.as_str(), keywords))
            .map(|item| item.scene_number);

        match match_scene {
            Some(scene_number) if scene_number <= deadline_scene => {}
            Some(scene_number) => issues.push(LaunchContractIssue {
                severity: LaunchContractSeverity::Error,
                code: "launch_beat_late".to_string(),
                message: format!(
                    "launch_contract requires `{}` by scene {}, but the current launch spine places it at scene {} in {}.",
                    beat_id,
                    deadline_scene,
                    scene_number,
                    spine.path.display()
                ),
                remediation: format!(
                    "Move `{}` into scene {} or earlier in {}.",
                    beat_id,
                    deadline_scene,
                    spine.path.display()
                ),
            }),
            None => issues.push(LaunchContractIssue {
                severity: LaunchContractSeverity::Error,
                code: "launch_beat_missing".to_string(),
                message: format!(
                    "launch_contract requires `{}` by scene {}, but the current launch spine does not mention it in {}.",
                    beat_id,
                    deadline_scene,
                    spine.path.display()
                ),
                remediation: format!(
                    "Add `{}` to scene {} or earlier in {}.",
                    beat_id,
                    deadline_scene,
                    spine.path.display()
                ),
            }),
        }
    }

    Ok(LaunchContractReport {
        enabled: true,
        primary_plot_path,
        required_beats,
        issues,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LaunchSpine {
    path: PathBuf,
    items: Vec<LaunchSpineItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LaunchSpineItem {
    scene_number: u32,
    text: String,
}

fn render_required_beats(contract: &crate::config::LaunchContract) -> Vec<String> {
    let mut rendered = Vec::new();
    if !contract.must_show_by_scene_3.is_empty() {
        rendered.push(format!(
            "scene<=3: {}",
            contract.must_show_by_scene_3.join(", ")
        ));
    }
    if !contract.must_show_by_scene_6.is_empty() {
        rendered.push(format!(
            "scene<=6: {}",
            contract.must_show_by_scene_6.join(", ")
        ));
    }
    rendered
}

fn contract_requirements(contract: &crate::config::LaunchContract) -> Vec<(u32, String)> {
    let mut requirements = Vec::new();
    for beat in &contract.must_show_by_scene_3 {
        requirements.push((3, normalize_beat_id(beat)));
    }
    for beat in &contract.must_show_by_scene_6 {
        requirements.push((6, normalize_beat_id(beat)));
    }
    requirements
}

fn normalize_beat_id(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn beat_keywords(beat_id: &str) -> Option<&'static [&'static str]> {
    match beat_id {
        "escape" | "escape_started" => Some(&["도주", "탈출", "이탈", "도망", "추격 회피", "탈출로"]),
        "larzesh" => Some(&["라르제쉬"]),
        "forced_companionship" => Some(&["강제 동행", "공동 도주", "임시 구속", "함께", "묶인"]),
        "relic_hint" => Some(&["유물", "유적 단서", "지도 조각", "오크 표식", "말 아닌 말"]),
        "golem_hint" => Some(&["골렘"]),
        _ => None,
    }
}

fn line_matches_keywords(line: &str, keywords: &[&str]) -> bool {
    let normalized = line.to_ascii_lowercase();
    keywords
        .iter()
        .any(|keyword| normalized.contains(&keyword.to_ascii_lowercase()))
}

fn find_primary_launch_spine(plot_dir: &Path) -> Result<Option<LaunchSpine>> {
    if !plot_dir.exists() {
        return Ok(None);
    }

    let mut docs = list_markdown_files(plot_dir)?;
    docs.sort_by_key(|path| {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        (
            !name.contains("plot-000") && !name.contains("launch"),
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string(),
        )
    });

    for path in docs {
        if let Some(items) = extract_episode_spine(&path)? {
            return Ok(Some(LaunchSpine { path, items }));
        }
    }

    Ok(None)
}

fn extract_episode_spine(path: &Path) -> Result<Option<Vec<LaunchSpineItem>>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read launch spine candidate {}", path.display()))?;
    let mut in_episode_spine = false;
    let mut items = Vec::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.starts_with("## ") {
            if line.eq_ignore_ascii_case("## Episode Spine") {
                in_episode_spine = true;
                continue;
            }
            if in_episode_spine {
                break;
            }
        }

        if !in_episode_spine {
            continue;
        }

        if let Some((scene_number, text)) = parse_spine_item(line) {
            items.push(LaunchSpineItem { scene_number, text });
        }
    }

    if items.len() >= 3 && items.first().map(|item| item.scene_number) == Some(1) {
        Ok(Some(items))
    } else {
        Ok(None)
    }
}

fn parse_spine_item(line: &str) -> Option<(u32, String)> {
    let digits_len = line.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits_len == 0 {
        return None;
    }

    let (digits, rest) = line.split_at(digits_len);
    let rest = rest.strip_prefix('.')?.trim();
    if rest.is_empty() {
        return None;
    }

    Some((digits.parse().ok()?, rest.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{validate_launch_contract, LaunchContractSeverity};
    use crate::config::{LaunchContract, NovelSettings};
    use anyhow::Result;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn launch_contract_detects_late_and_missing_beats() -> Result<()> {
        let temp_dir = tempdir()?;
        let workspace = temp_dir.path();
        fs::create_dir_all(workspace.join("03_StoryBible/Plot"))?;
        fs::write(
            workspace.join("03_StoryBible/Plot/PLOT-000-Launch.md"),
            "# Launch\n\n## Episode Spine\n1. 배급소 붕괴와 통행패 압수\n2. 배수 골목 추락과 첫 추격 회피\n3. 떠나기 위한 최소 짐과 유적 단서 확보\n4. 검문 강화 속 비공식 탈출로로 도시 이탈\n7. 사슬에 묶인 라르제쉬 조우\n",
        )?;

        let mut novel_settings = NovelSettings::default();
        novel_settings.launch_contract = LaunchContract {
            enabled: true,
            must_show_by_scene_3: vec![
                "larzesh".to_string(),
                "relic_hint".to_string(),
                "golem_hint".to_string(),
            ],
            must_show_by_scene_6: Vec::new(),
        };

        let report = validate_launch_contract(workspace, &novel_settings)?;
        assert!(report.has_blocking_issues());
        assert_eq!(report.status_label(), "blocking_issues");
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("places it at scene 7")));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("does not mention it")));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.severity == LaunchContractSeverity::Error));
        Ok(())
    }

    #[test]
    fn launch_contract_warns_when_enabled_but_empty() -> Result<()> {
        let temp_dir = tempdir()?;
        let mut novel_settings = NovelSettings::default();
        novel_settings.serialized_workflow = true;
        novel_settings.launch_contract = LaunchContract {
            enabled: true,
            must_show_by_scene_3: Vec::new(),
            must_show_by_scene_6: Vec::new(),
        };

        let report = validate_launch_contract(temp_dir.path(), &novel_settings)?;
        assert_eq!(report.status_label(), "empty");
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].severity, LaunchContractSeverity::Warning);
        Ok(())
    }
}
