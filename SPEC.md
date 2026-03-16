# SPEC.md

## Product Goal

Rust로 구현된 CLI-first AI 소설 생성 엔진 MVP를 제공한다. 사용자는 CLI 명령만으로 소설 생성, 리뷰, 수정, 승인, 상태 확인을 수행한다.

## Non-Negotiable Constraints

- 웹 UI는 만들지 않는다.
- OpenAI API를 직접 호출하지 않는다.
- API 키 기반 구현을 추가하지 않는다.
- OAuth 토큰을 직접 처리하지 않는다.
- LLM 호출은 사용자가 미리 로그인한 `codex` CLI subprocess만 사용한다.
- 워크스페이스 정책은 `1 workspace = 1 novel`이다.
- 엔진 프로젝트 Git과 소설 워크스페이스 Git은 분리한다.

## Config Layers

- 사용자 전역 설정: `~/.config/heeforge/config.toml`
- 소설별 설정: `<workspace>/novel.toml`
- 엔진 내부 메타: `<workspace>/.novel/workspace.json`

우선순위:

1. 환경 변수
2. 사용자 전역 설정
3. 기본값

## Required Novel Config

- `title`
- `genre`
- `tone`
- `premise`
- `protagonist_name`
- `language`

`heeforge init`은 누락된 필드를 인터랙티브하게 질문할 수 있어야 한다.
`heeforge next-scene`은 위 필드가 비어 있으면 실패해야 한다.

## Required Commands

- `heeforge init [PATH]`
- `heeforge status`
- `heeforge doctor`
- `heeforge next-scene`
- `heeforge review`
- `heeforge rewrite SCENE_ID --instruction "..."`
- `heeforge approve SCENE_ID`
- `heeforge next-chapter`
- `heeforge expand-world`
- `heeforge memory`
- `heeforge show SCENE_ID`

## CLI Output Contract

- 기본 출력은 사람이 읽기 쉬운 `text` 모드다.
- 모든 명령은 `--format json`을 지원해야 한다.
- JSON 출력은 최소한 다음 필드를 포함해야 한다.
  - 성공: `status`, `command`, `workspace`, `summary`, `details`, `artifacts`, `next_steps`
  - 실패: `status`, `command`, `error_code`, `reason`, `remediation`
- `heeforge init --format json`은 interactive prompt를 기다리지 않고 non-interactive로 동작해야 한다.
- `--help`에는 사용 예시와 JSON 자동화 예시가 포함되어야 한다.

## Distribution Contract

- 저장소 루트에 `install.sh`를 둔다.
- 사용자는 `curl -fsSL https://raw.githubusercontent.com/johwanghee/heeforge/main/install.sh | bash` 형태로 설치할 수 있어야 한다.
- `install.sh`는 GitHub Releases에서 현재 플랫폼에 맞는 `heeforge-<target>.tar.gz` 자산을 내려받아 설치한다.
- release 자산은 최소 다음 target을 지원한다.
  - `x86_64-unknown-linux-gnu`
  - `aarch64-unknown-linux-gnu`
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
- release 자산은 대응하는 `.sha256` 파일을 함께 제공해야 한다.

## Core Components

- `Planner`
- `Writer`
- `Editor`
- `Critic`
- `MemoryManager`
- `StateManager`
- `CodexRunner`
- `NovelEngine`
- `CLI commands`

## Data Models

### StoryState

- `current_arc: u32`
- `current_chapter: u32`
- `current_scene: u32`
- `stage: String`
- `current_goal: Option<String>`
- `open_conflicts: Vec<String>`
- `current_scene_id: Option<String>`

### Scene

- `id: String`
- `chapter: u32`
- `scene_number: u32`
- `short_title: String`
- `goal: String`
- `conflict: String`
- `outcome: String`
- `text: String`
- `status: String`

### ReviewIssue

- `issue_type: String`
- `description: String`
- `line_start: Option<u32>`
- `line_end: Option<u32>`

## Storage Format

- 각 소설은 별도 워크스페이스 디렉터리로 관리한다.
- 워크스페이스 루트에는 소설 전용 `.gitignore`를 생성한다.
- workspace Git auto-commit은 설정 기반 opt-in이어야 하며 기본값은 비활성화다.
- dummy fallback은 설정 기반 opt-in이어야 하며 기본값은 비활성화다.
- 사람용 작업물은 워크스페이스 루트의 numbered folder 구조에 저장한다.
- 워크스페이스 메타데이터: `.novel/workspace.json`
- 소설 설정 파일: `novel.toml`
- 상태 파일: `.novel/state/project_state.json`
- 메모리 파일:
  - `.novel/memory/core_memory.md`
  - `.novel/memory/story_memory.md`
  - `.novel/memory/active_memory.md`
- opt-in prompt 로그:
  - `.novel/logs/llm_prompts/*.json`
  - 기본값은 비활성화이며, 전역 설정 또는 env로만 켠다.
- opt-in dummy fallback:
  - 기본값은 비활성화다.
  - 전역 설정 `allow_dummy_fallback = true` 또는 `HEEFORGE_ALLOW_DUMMY=true`일 때만 허용한다.
  - fallback이 사용된 성공 응답에는 warning을 포함해야 한다.
- 디스크의 memory 파일은 전체 이력을 유지하되, planner/writer/expand-world용 story memory 주입은 최근/high-signal 섹션 중심의 bounded prompt view를 사용해야 한다.
- draft scene 파일: `02_Draft/Scenes/scene_<chapter>_<scene>-<slug>.md`를 기본으로 하되, slug는 `short_title` 기준이어야 한다. slug를 만들 수 없으면 `scene_<chapter>_<scene>.md`도 허용한다.
- draft chapter 파일: `02_Draft/Chapters/chapter_<chapter>-<slug>.md`를 기본으로 하되, slug는 compiled chapter `short_title` 기준이어야 한다. slug를 만들 수 없으면 `chapter_<chapter>.md`도 허용한다.
- story bible 폴더:
  - `03_StoryBible/Characters/`
  - `03_StoryBible/World/`
  - `03_StoryBible/Rules/`
  - `03_StoryBible/Timeline/`
  - `03_StoryBible/Plot/`
- research 폴더:
  - `04_Research/Sources/`
  - `04_Research/Notes/`
  - `04_Research/References/`
- review 폴더:
  - `06_Review/Feedback/`
  - `06_Review/Revisions/`
- `init`은 workspace root `README.md`, 주요 섹션 `README.md`, 그리고 `98_Templates/` starter template 파일을 생성해야 한다.
- scene 생성 로그: `.novel/logs/scene_generation/<scene_id>.json`
- review 결과: `06_Review/Feedback/<scene_id>.json`
- rewrite snapshot: `06_Review/Revisions/<scene_id>/rewrite_<n>_{original|rewritten}.md`
- rewrite metadata record는 workspace root 기준 상대경로를 저장해야 한다.

scene markdown 형식:

```md
# Scene scene_001_001

## Short Title
...

## Goal
...

## Conflict
...

## Outcome
...

## Status
draft

## Text
...
```

chapter markdown 형식:

```md
# Chapter 001

## Short Title
...

Compiled from ...
```

## Engine Flow

### generate_next_scene

1. state와 memory를 로드한다.
   - planner/writer에는 prompt-sized memory view를 사용한다.
2. planner가 short_title/goal/conflict/outcome을 만든다.
3. writer가 scene 본문을 쓴다.
4. editor가 문체와 반복을 정리한다.
5. scene markdown를 저장한다.
6. scene 생성 로그를 저장한다.
7. state를 갱신한다.
8. active/story memory를 갱신한다.
9. fallback이 개입된 경우 warning을 반환한다.

### review_current_scene

1. current scene을 로드한다.
2. critic이 리뷰 이슈를 생성한다.
3. review JSON 파일을 저장한다.
4. fallback이 개입된 경우 warning과 함께 `Vec<ReviewIssue>`를 반환한다.

### rewrite_scene

1. 대상 scene을 로드한다.
2. 기존 scene 원본 snapshot을 저장한다.
3. instruction을 반영해 editor 또는 writer를 다시 실행한다.
4. 수정본 snapshot과 metadata를 저장한다.
5. 수정된 text를 현재 scene 파일에 반영한다.

### approve_scene

1. scene status를 `approved`로 바꾼다.

### generate_next_chapter

1. 현재 chapter의 scene markdown를 모은다.
2. scene 번호가 1부터 연속인지 검증한다.
3. chapter markdown를 생성한다.

## CodexRunner Requirements

- `healthcheck() -> Result<bool>`
- `ensure_available() -> Result<()>`
- `run_prompt(prompt: &str) -> Result<String>`
- prompt logging이 켜진 경우 prompt, response/error, label, attempt, duration을 `.novel/logs/llm_prompts/`에 JSON으로 남겨야 한다.

## Workspace Git Automation

- 전역 설정 또는 env로 workspace Git auto-commit을 켤 수 있어야 한다.
- auto-commit이 켜지면 workspace root에 `.git`이 없을 경우 자동으로 `git init`을 수행해야 한다.
- 다음 변경 명령 성공 후에는 workspace repo에 자동 commit을 남겨야 한다:
  - `init`
  - `next-scene`
  - `review`
  - `rewrite`
  - `approve`
  - `next-chapter`
  - `expand-world`
- auto-commit 실패가 본 명령 자체를 실패시키면 안 된다. 본 작업 결과는 유지하고 warning으로만 드러내야 한다.

실패 시 에러 메시지에는 반드시 `먼저 codex login 실행` 문구가 포함되어야 한다.
기본 설정에서는 codex 실패가 dummy output으로 자동 대체되면 안 된다.
실패 시 `run_prompt`는 1회 재시도한다.
응답이 지정된 timeout 안에 오지 않으면 subprocess를 종료하고 timeout 에러를 반환해야 한다.

## Test Floor

최소 smoke test 범위:

- `init_project` 동작
- state 파일 생성
- memory 파일 생성
- dummy scene 저장 가능
