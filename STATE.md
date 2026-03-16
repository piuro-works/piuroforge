# STATE.md

기준 시각: 2026-03-16

## Current Status

- `heeforge` Rust 크레이트가 생성되어 있다.
- CLI command 집합이 `src/main.rs`와 `src/commands/`에 구현되어 있다.
- 워크스페이스 기반 저장 모델이 적용되어 사람용 소설 데이터는 워크스페이스 루트의 numbered folder에, 엔진 런타임 데이터는 `.novel/`에 분리 저장된다.
- `1 workspace = 1 novel` 정책을 문서와 초기화 로직에 반영했다.
- `heeforge init`은 소설 워크스페이스용 `.gitignore`를 생성해 엔진 Git과 소설 Git 분리를 돕는다.
- `heeforge init`은 `00_Inbox`, `02_Draft`, `03_StoryBible`, `06_Review`, `07_Archive`, `98_Templates`를 포함한 사람용 작업 스캐폴드를 생성한다.
- `heeforge init`은 루트/섹션 `README.md`와 `98_Templates` starter template 파일도 생성한다.
- 전역 `config.toml` 생성 내용이 작가용 first-run 설명과 codex/dummy fallback 안내를 포함하도록 보강됐다.
- scene markdown는 stable `scene_id`를 앞에 유지한 slugged filename으로 저장되며, slug는 scene `short_title` 기준으로 생성된다.
- chapter markdown는 compiled `short_title`을 포함하고 slugged filename으로 저장된다.
- 설정 계층이 `~/.config/heeforge/config.toml`, `<workspace>/novel.toml`, `<workspace>/.novel/workspace.json`으로 분리됐다.
- `heeforge init`은 필수 메타를 인터랙티브하게 수집할 수 있다.
- CLI 전역 옵션 `--format text|json`이 추가됐다.
- command 출력은 사람용 text와 에이전트용 JSON 계약으로 통일됐다.
- `--help`가 quickstart, 자동화 예시, subcommand 예시를 포함하도록 보강됐다.
- 저장소 루트 `install.sh`가 GitHub Release 자산 설치를 지원한다.
- `scripts/package-release.sh`가 target별 `heeforge-<target>.tar.gz`와 `.sha256`를 생성한다.
- `.github/workflows/release.yml`가 `v*` tag push 시 release 자산 발행을 담당한다.
- `next-scene`은 `premise`, `protagonist_name` 등 필수 `novel.toml` 값이 비어 있으면 실패한다.
- planner/writer/expand-world 프롬프트는 full memory 파일 대신 bounded prompt memory view를 사용해 `story_memory`와 `open_conflicts` 누적을 완화한다.
- dummy fallback은 기본 비활성화로 전환됐고, opt-in일 때만 허용된다.
- opt-in prompt logging이 추가되어, 켜면 `.novel/logs/llm_prompts/`에 agent label별 prompt/response JSON 로그를 남긴다.
- opt-in workspace Git auto-commit이 추가되어, 켜면 workspace repo를 자동 초기화하고 변경 명령 뒤에 자동 commit을 남긴다.
- `next-scene`, `review`, `rewrite`, `expand-world`는 dummy fallback이 발생하면 성공 응답에도 warning을 노출한다.
- codex 실패 에러 출력은 로그인 문제와 네트워크/transport 문제를 구분해 remediation을 안내하고, opt-in dummy fallback 설정 경로도 보여준다.
- `init` 출력과 workspace root `README.md`는 `codex login`, dummy fallback, workspace auto-commit 설정 순서를 비개발자 기준으로 안내한다.
- planner/writer/editor/critic 프롬프트 템플릿이 `src/prompts/`로 분리됐다.
- scene 생성 로그는 `.novel/logs/`에, review JSON과 rewrite snapshot은 `06_Review/`에 저장된다.
- `next-chapter`는 scene 번호 연속성을 검증한다.
- `CodexRunner`는 호출 실패 시 1회 재시도한다.
- `CodexRunner`는 응답 timeout을 넘기면 subprocess를 강제 종료한다.
- 바이너리 테스트가 하위 디렉터리 실행 시 nearest workspace 자동 탐색을 검증한다.
- 바이너리 테스트가 `rewrite`/`approve`의 JSON 출력, 산출물 보존, 상태 전이를 검증한다.
- 바이너리 테스트가 `init`, `status`, `next-scene`, `review`, `rewrite`, `approve`, `next-chapter`, `expand-world`, `memory`, `show` 전 커맨드의 JSON 계약을 검증한다.
- 바이너리 테스트가 `review`의 current scene 부재, `next-chapter`의 empty/gapped chapter, `show`의 미존재 scene 조회 같은 고위험 실패 경로의 JSON 에러 계약도 검증한다.
- 바이너리 테스트가 기본 codex 실패 시 `codex_unavailable` 에러와 opt-in dummy fallback warning 노출도 검증한다.
- `NovelEngine`가 scene 생성, 리뷰, 수정, 승인, chapter 컴파일, memory 조회를 오케스트레이션한다.
- `CodexRunner`는 `codex` CLI subprocess만 사용하도록 구현되어 있다.
- `StateManager`와 `MemoryManager`가 기본 파일 생성과 로드/저장을 처리한다.
- smoke test 파일 `tests/smoke.rs`가 존재한다.

## Verified

- `cargo test`가 통과했다.
- 더미 codex 경로로 `heeforge init <workspace>`와 `heeforge --workspace <workspace> next-scene` CLI smoke run이 통과했다.
- 실제 `codex` CLI가 PATH에 있는 로컬 환경에서 `heeforge --workspace <workspace> next-scene` smoke run이 통과했다.
- smoke test가 전역 설정 파일과 `novel.toml` 생성, 설정 계층 로딩을 검증한다.
- `HEEFORGE_CONFIG_DIR=<temp> heeforge init <workspace>`로 전역 `config.toml`, 워크스페이스 `novel.toml`, 내부 `workspace.json` 생성이 확인됐다.
- smoke test가 사람용 `02_Draft`, `03_StoryBible`, `06_Review` 스캐폴드와 워크스페이스 `README.md` 생성을 검증한다.
- smoke test가 섹션 `README.md`와 template 파일 생성도 검증한다.
- `cargo test`에서 생성 로그, review 저장, rewrite snapshot, chapter 순서 검증, codex retry가 확인됐다.
- unit test가 oversized `story_memory`의 prompt compaction과 `open_conflicts` recent-window 축약을 검증한다.
- codex runner test가 opt-in prompt logging 파일 생성과 로그 payload 저장을 검증한다.
- 바이너리 테스트가 workspace Git auto-commit이 `init`과 `next-scene` 뒤에 실제 commit을 남기는지 검증한다.
- `cargo test`에서 rewrite metadata가 workspace 상대경로를 저장하는지 확인됐다.
- smoke test와 CLI test가 `scene_001_001-...` 형식의 slugged scene filename 생성을 검증한다.
- planner dummy path와 scene markdown 저장 포맷이 `short_title` 필드를 포함하도록 갱신됐다.
- smoke test와 CLI test가 `chapter_001-securing-the-lead.md` 형식의 slugged chapter filename과 `Short Title` 섹션 생성을 검증한다.
- smoke test가 필수 메타 누락 시 scene 생성 차단도 검증한다.
- 인터랙티브 `heeforge init`로 `premise`, `protagonist_name` 입력 후 `novel.toml` 반영이 확인됐다.
- 바이너리 테스트에서 `--help`, `status --format json`, `next-scene --format json` 에러 payload가 검증됐다.
- 바이너리 테스트에서 워크스페이스 하위 디렉터리의 `status`, `next-scene` 실행 시 nearest workspace 자동 탐색이 검증됐다.
- 바이너리 테스트에서 `rewrite scene_001_001 --instruction ...`가 rewrite snapshot과 scene 갱신을 남기는지 검증됐다.
- 바이너리 테스트에서 `approve scene_001_001`가 scene markdown 상태와 `status`의 `scene_approved` 전이를 반영하는지 검증됐다.
- 바이너리 테스트에서 `init`, `review`, `show`, `memory`, `expand-world` JSON 출력과 관련 산출물 반영이 검증됐다.
- 바이너리 테스트에서 `review`의 `no_current_scene`, `next-chapter`의 `empty_chapter`/`invalid_scene_sequence`, `show`의 missing scene generic error payload가 검증됐다.
- 로컬 release asset을 만든 뒤 `HEEFORGE_DOWNLOAD_URL=file://... ./install.sh` 설치 smoke check가 통과했다.
- hang 재현 테스트에서 `codex exec` timeout과 no-retry 동작이 검증됐다.
- 요구된 문서 파일인 `README.md`, `.env.example`, `AGENTS.md`, `SPEC.md`, `STATE.md`가 존재한다.

## Not Yet Verified

- GitHub Release 원격 URL에서 실제 `install.sh` end-to-end 설치는 tag release 발행 후 추가 검증이 필요하다.

## Known Gaps / Risks

- opt-in dummy fallback 출력은 결정적이지만 문학적 품질은 낮다.
- planner/critic 출력 파서는 기본 fallback이 있으며 schema 강제 수준이 낮다.
- chapter 승인 정책과 open conflict 해소 정책은 아직 단순하다.
- 실제 codex 응답 형식이 JSON 규약을 어기면 fallback 동작에 의존하게 된다.
- JSON 출력 계약은 현재 안정화됐지만 아직 command별 세부 schema versioning은 없다.
- GitHub Actions runner 가용성에 따라 `ubuntu-24.04-arm` release job은 저장소 환경에서 추가 검증이 필요할 수 있다.
- 실제 codex 응답 시간이 긴 작업에서는 기본 120초 timeout이 짧을 수 있어 운영 환경에 맞춘 조정이 필요할 수 있다.

## Recommended Next Actions

1. planner/writer/editor/critic에 대한 fixture 기반 테스트를 추가한다.
2. `doctor` 명령을 추가해 사용자 친화적 진단과 remediation 출력을 강화한다.
3. tag release 한 번을 실제로 발행해 `install.sh`가 원격 Release 경로에서도 정상 동작하는지 검증한다.
