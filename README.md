# Novel Engine MVP

Rust 기반 CLI-first AI 소설 생성 엔진 MVP다. 웹 UI 없이 `novel` 명령만으로 scene 생성, 리뷰, 수정, 승인, 상태 확인을 수행한다.

LLM 호출은 직접 OpenAI API 키/OAuth를 다루지 않고, 사용자가 미리 로그인한 `codex` CLI subprocess만 사용한다. 다만 현재 MVP와 테스트를 위해 `codex`가 없거나 로그인되지 않은 경우 dummy fallback을 기본 허용한다.

CLI UX는 두 가지 출력 모드를 제공한다.

- 기본 `text`: 사람이 읽기 쉬운 요약, 경로, 다음 추천 명령 출력
- `--format json`: Codex CLI, OpenClaw, 기타 LLM 에이전트가 안정적으로 해석할 수 있는 구조화 출력

실제 소설 데이터는 엔진 소스 디렉터리가 아니라 별도 워크스페이스에 저장된다. 각 워크스페이스 루트 아래에 숨김 디렉터리 `.novel/`이 생성되고, 상태/메모리/scene/chapter 산출물이 그 안에 쌓인다.
운영 원칙은 `1 workspace = 1 novel`이다. 엔진 프로젝트 Git과 소설 작업 Git은 분리한다.

설정은 3층으로 분리된다.

- 사용자 전역 설정: `~/.config/heeforge/config.toml`
- 소설별 설정: `<workspace>/novel.toml`
- 엔진 내부 메타: `<workspace>/.novel/workspace.json`

scene 생성을 시작하기 위한 필수 메타는 다음이다.

- `title`
- `genre`
- `tone`
- `premise`
- `protagonist_name`
- `language`

## 설치

1. Rust stable 설치
2. `codex` CLI 설치
3. `codex login` 실행

실제 codex 기반 생성/리뷰/수정을 쓰려면 먼저 `codex login`이 되어 있어야 한다.

## 빌드

```bash
cargo build
```

release 단일 바이너리 빌드:

```bash
cargo build --release
```

실행 파일:

```bash
./target/release/novel
```

## Release 배포

release 바이너리 배포 기준 권장 절차:

```bash
cargo build --release
mkdir -p dist
cp target/release/novel dist/novel
```

사용자 머신 설치 예시:

```bash
install -m 755 dist/novel /usr/local/bin/novel
novel init ~/novels/my-first-novel
```

설치 직후 도움말 확인:

```bash
novel --help
novel next-scene --help
```

배포 전 최소 확인:

- `cargo test`
- `./target/release/novel init <workspace>`
- `./target/release/novel --workspace <workspace> next-scene`
- `./target/release/novel --workspace <workspace> --format json status`
- `codex login` 이후 실제 codex 경로 검증

## 사용법

설치된 바이너리 기준으로 새 워크스페이스 생성:

```bash
novel init ~/novels/my-first-novel
```

`init`은 누락된 필수값을 인터랙티브하게 질문한다. 자동화가 필요하면 flag로 넘기거나 `--no-input`을 쓸 수 있다.

```bash
novel init ~/novels/my-first-novel \
  --title "기억 편집자" \
  --genre "Mystery" \
  --tone "Tense, atmospheric" \
  --premise "기억을 잃은 조사관이 사라진 동생의 흔적을 좇다가 도시 기록 체계의 조작을 발견한다." \
  --protagonist "윤서" \
  --language "ko"
```

원격 동기화가 필요하면 워크스페이스 루트에서 별도 Git 저장소를 초기화한다.

```bash
cd ~/novels/my-first-novel
git init
git add .
git commit -m "Initialize novel workspace"
```

`novel init`은 워크스페이스용 `.gitignore`를 생성해서 엔진 내부 상태 파일만 제외하고, scene/chapter/story memory 같은 실제 소설 산출물은 Git에 포함할 수 있게 준비한다.
처음 실행 시 전역 설정 파일 `~/.config/heeforge/config.toml`도 없으면 기본값으로 생성된다.
필수값이 비어 있는 상태로 `init --no-input`을 수행한 경우 워크스페이스는 생성되지만 `next-scene` 전에 `novel.toml`을 채워야 한다.

워크스페이스 안에서 작업:

```bash
cd ~/novels/my-first-novel
novel status
novel next-scene
novel review
novel rewrite scene_001_001 --instruction "대사를 더 날카롭게"
novel approve scene_001_001
novel next-chapter
novel expand-world
novel memory
novel show scene_001_001
```

현재 디렉터리가 워크스페이스가 아니거나 다른 위치에서 실행 중이면 `--workspace`로 명시할 수 있다.

```bash
novel --workspace ~/novels/my-first-novel status
```

LLM/자동화 연동 시에는 `--format json`을 권장한다.

```bash
novel --workspace ~/novels/my-first-novel --format json status
novel --workspace ~/novels/my-first-novel --format json next-scene
```

`--format json`에서 `init`을 실행하면 interactive prompt를 기다리지 않도록 자동으로 non-interactive 모드로 동작한다. 필요한 필드는 flag 또는 `novel.toml` 편집으로 채운다.

개발 중에는 소스 트리에서 그대로 실행할 수도 있다.

```bash
cargo run -- init ~/novels/my-first-novel
cargo run -- --workspace ~/novels/my-first-novel next-scene
```

권장: 소설 워크스페이스는 엔진 소스 저장소 밖에 둔다.

## 산출물과 로그

- scene 생성 로그: `<workspace>/.novel/logs/scene_generation/<scene_id>.json`
- review 결과 JSON: `<workspace>/.novel/logs/reviews/<scene_id>.json`
- rewrite 원본/수정본 보존: `<workspace>/.novel/logs/rewrites/<scene_id>/`
- chapter 생성 시 scene 번호가 1부터 연속인지 검증한다.

## 환경 변수

`.env.example` 참고.

- `NOVEL_ENGINE_CODEX_CMD`: 기본값 `codex`
- `NOVEL_ENGINE_ALLOW_DUMMY`: 기본값 `true`

`NOVEL_ENGINE_ALLOW_DUMMY=false`로 두면 codex CLI가 없거나 로그인되지 않았을 때 명확한 에러를 반환한다.
환경 변수 우선순위는 전역 설정 파일보다 높다.

## 현재 MVP 범위

- 별도 워크스페이스 생성 및 `.novel/` 저장소 관리
- 전역 `config.toml`, 소설별 `novel.toml`, 내부 `workspace.json` 분리
- `init` 시 필수 메타 인터랙티브 입력 유도
- `--help` 예시 보강 및 `--format text|json` 이중 출력
- `next-scene` 전 필수 novel config 검증
- state JSON 관리
- core/story/active memory markdown 관리
- scene markdown 저장 및 조회
- planner/writer/editor/critic 프롬프트 템플릿 분리
- scene 생성 로그 저장
- rewrite 원본/수정본 snapshot 보존
- review 결과 JSON 저장
- chapter 컴파일 전 scene 순서 검증
- `codex` subprocess 재시도 1회
- 최소 smoke test 및 runner retry test

## 다음 확장 포인트

- planner 출력 schema 검증 강화
- chapter/arc 단위 요약 memory 자동화
- 승인 이력 및 diff 로그
- agent별 prompt 템플릿 분리
- trait 기반 mock runner 주입
