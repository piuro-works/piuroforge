# AGENTS.md

## Purpose

이 프로젝트는 Rust 기반 CLI-first AI 소설 엔진 MVP다. CLI 실행 이름은 `heeforge`이며, 모든 변경은 웹 UI가 아니라 CLI 워크플로를 우선해야 한다.

## Hard Constraints

- OpenAI API를 직접 호출하지 않는다.
- API 키 기반 구현을 추가하지 않는다.
- OAuth 토큰을 직접 처리하지 않는다.
- LLM 호출은 `codex` CLI subprocess만 사용한다.
- 단일 바이너리 빌드를 유지한다.
- 동기식 구조를 기본으로 유지한다. 비동기 전환은 명확한 병목이 확인되기 전에는 하지 않는다.

## Architecture Rules

- 핵심 오케스트레이션은 `src/engine.rs`에 둔다.
- CLI 진입점과 command dispatch는 `src/main.rs` 및 `src/commands/`에 둔다.
- 에이전트 역할 분리는 `src/agents/`에서 유지한다.
- 워크스페이스 정책은 `1 workspace = 1 novel`이다.
- 엔진 프로젝트 Git과 소설 워크스페이스 Git은 별개로 취급한다.
- 전역 런타임 설정은 `~/.config/heeforge/config.toml`에 둔다.
- 소설별 설정은 워크스페이스 루트의 `novel.toml`에 둔다.
- `.novel/workspace.json`은 엔진 내부 메타만 담는다.
- 실제 작업 데이터는 워크스페이스 루트 아래 `.novel/`에 저장한다.
- 상태는 `.novel/state/project_state.json`으로 관리한다.
- 메모리는 `.novel/memory/*.md`로 관리한다.
- scene/chapter 산출물은 `.novel/` 아래 markdown 파일로 유지한다.
- 테스트 가능성을 위해 dummy fallback 경로를 유지한다.
- `CodexRunner`는 subprocess 경계다. 다른 모듈에서 직접 `codex`를 호출하지 않는다.

## Change Checklist

- 새 CLI 동작을 추가하면 `README.md`와 `SPEC.md`를 같이 갱신한다.
- 동작 상태가 바뀌면 `STATE.md`를 갱신한다.
- 저장 포맷을 바꾸면 기존 markdown/json과의 호환성을 검토한다.
- scene 생성, rewrite, review 흐름을 바꾸면 smoke test 또는 관련 테스트를 추가한다.

## Validation

- 가능하면 `cargo test`와 최소 `cargo run -- init`, `cargo run -- next-scene`를 확인한다.
- 툴체인이 없어서 검증을 못 하면 그 사실을 `STATE.md`와 작업 보고에 명시한다.
