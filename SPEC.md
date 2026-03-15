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

`novel init`은 누락된 필드를 인터랙티브하게 질문할 수 있어야 한다.
`novel next-scene`은 위 필드가 비어 있으면 실패해야 한다.

## Required Commands

- `novel init [PATH]`
- `novel status`
- `novel next-scene`
- `novel review`
- `novel rewrite SCENE_ID --instruction "..."`
- `novel approve SCENE_ID`
- `novel next-chapter`
- `novel expand-world`
- `novel memory`
- `novel show SCENE_ID`

## CLI Output Contract

- 기본 출력은 사람이 읽기 쉬운 `text` 모드다.
- 모든 명령은 `--format json`을 지원해야 한다.
- JSON 출력은 최소한 다음 필드를 포함해야 한다.
  - 성공: `status`, `command`, `workspace`, `summary`, `details`, `artifacts`, `next_steps`
  - 실패: `status`, `command`, `error_code`, `reason`, `remediation`
- `novel init --format json`은 interactive prompt를 기다리지 않고 non-interactive로 동작해야 한다.
- `--help`에는 사용 예시와 JSON 자동화 예시가 포함되어야 한다.

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
- 워크스페이스 메타데이터: `.novel/workspace.json`
- 소설 설정 파일: `novel.toml`
- 상태 파일: `.novel/state/project_state.json`
- 메모리 파일:
  - `.novel/memory/core_memory.md`
  - `.novel/memory/story_memory.md`
  - `.novel/memory/active_memory.md`
- scene 파일: `.novel/scenes/scene_<chapter>_<scene>.md`
- chapter 파일: `.novel/chapters/chapter_<chapter>.md`
- scene 생성 로그: `.novel/logs/scene_generation/<scene_id>.json`
- review 결과: `.novel/logs/reviews/<scene_id>.json`
- rewrite snapshot: `.novel/logs/rewrites/<scene_id>/rewrite_<n>_{original|rewritten}.md`

scene markdown 형식:

```md
# Scene scene_001_001

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

## Engine Flow

### generate_next_scene

1. state와 memory를 로드한다.
2. planner가 goal/conflict/outcome을 만든다.
3. writer가 scene 본문을 쓴다.
4. editor가 문체와 반복을 정리한다.
5. scene markdown를 저장한다.
6. scene 생성 로그를 저장한다.
7. state를 갱신한다.
8. active/story memory를 갱신한다.

### review_current_scene

1. current scene을 로드한다.
2. critic이 리뷰 이슈를 생성한다.
3. review JSON 파일을 저장한다.
4. `Vec<ReviewIssue>`로 반환한다.

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

실패 시 에러 메시지에는 반드시 `먼저 codex login 실행` 문구가 포함되어야 한다.
실패 시 `run_prompt`는 1회 재시도한다.

## Test Floor

최소 smoke test 범위:

- `init_project` 동작
- state 파일 생성
- memory 파일 생성
- dummy scene 저장 가능
