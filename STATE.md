# STATE.md

기준 시각: 2026-03-15

## Current Status

- `novel_engine` Rust 크레이트가 생성되어 있다.
- CLI command 집합이 `src/main.rs`와 `src/commands/`에 구현되어 있다.
- 워크스페이스 기반 저장 모델이 적용되어 실제 소설 데이터는 워크스페이스 루트 아래 `.novel/`에 저장된다.
- `1 workspace = 1 novel` 정책을 문서와 초기화 로직에 반영했다.
- `novel init`은 소설 워크스페이스용 `.gitignore`를 생성해 엔진 Git과 소설 Git 분리를 돕는다.
- 설정 계층이 `~/.config/heeforge/config.toml`, `<workspace>/novel.toml`, `<workspace>/.novel/workspace.json`으로 분리됐다.
- `novel init`은 필수 메타를 인터랙티브하게 수집할 수 있다.
- CLI 전역 옵션 `--format text|json`이 추가됐다.
- command 출력은 사람용 text와 에이전트용 JSON 계약으로 통일됐다.
- `--help`가 quickstart, 자동화 예시, subcommand 예시를 포함하도록 보강됐다.
- `next-scene`은 `premise`, `protagonist_name` 등 필수 `novel.toml` 값이 비어 있으면 실패한다.
- planner/writer/editor/critic 프롬프트 템플릿이 `src/prompts/`로 분리됐다.
- scene 생성 로그, review JSON, rewrite snapshot 저장이 추가됐다.
- `next-chapter`는 scene 번호 연속성을 검증한다.
- `CodexRunner`는 호출 실패 시 1회 재시도한다.
- `NovelEngine`가 scene 생성, 리뷰, 수정, 승인, chapter 컴파일, memory 조회를 오케스트레이션한다.
- `CodexRunner`는 `codex` CLI subprocess만 사용하도록 구현되어 있다.
- `StateManager`와 `MemoryManager`가 기본 파일 생성과 로드/저장을 처리한다.
- smoke test 파일 `tests/smoke.rs`가 존재한다.

## Verified

- `cargo test`가 통과했다.
- 더미 codex 경로로 `novel init <workspace>`와 `novel --workspace <workspace> next-scene` CLI smoke run이 통과했다.
- smoke test가 전역 설정 파일과 `novel.toml` 생성, 설정 계층 로딩을 검증한다.
- `HEEFORGE_CONFIG_DIR=<temp> novel init <workspace>`로 전역 `config.toml`, 워크스페이스 `novel.toml`, 내부 `workspace.json` 생성이 확인됐다.
- `cargo test`에서 생성 로그, review 저장, rewrite snapshot, chapter 순서 검증, codex retry가 확인됐다.
- smoke test가 필수 메타 누락 시 scene 생성 차단도 검증한다.
- 인터랙티브 `novel init`로 `premise`, `protagonist_name` 입력 후 `novel.toml` 반영이 확인됐다.
- 바이너리 테스트에서 `--help`, `status --format json`, `next-scene --format json` 에러 payload가 검증됐다.
- 요구된 문서 파일인 `README.md`, `.env.example`, `AGENTS.md`, `SPEC.md`, `STATE.md`가 존재한다.

## Not Yet Verified

- `codex exec --skip-git-repo-check` 호출 형식이 실제 사용자 환경의 codex CLI와 완전히 일치하는지는 런타임 검증이 필요하다.

## Known Gaps / Risks

- dummy fallback 출력은 결정적이지만 문학적 품질은 낮다.
- planner/critic 출력 파서는 기본 fallback이 있으며 schema 강제 수준이 낮다.
- chapter 승인 정책과 open conflict 해소 정책은 아직 단순하다.
- 실제 codex 응답 형식이 JSON 규약을 어기면 fallback 동작에 의존하게 된다.
- JSON 출력 계약은 현재 안정화됐지만 아직 command별 세부 schema versioning은 없다.

## Recommended Next Actions

1. `codex login` 이후 실제 워크스페이스에서 `novel next-scene`로 subprocess 경로를 검증한다.
2. 워크스페이스 자동 탐색과 하위 디렉터리 실행 시나리오에 대한 회귀 테스트를 추가한다.
3. planner/writer/editor/critic에 대한 fixture 기반 테스트를 추가한다.
4. scene rewrite와 approve 시나리오에 대한 회귀 테스트를 추가한다.
5. `doctor` 명령을 추가해 사용자 친화적 진단과 remediation 출력을 강화한다.
