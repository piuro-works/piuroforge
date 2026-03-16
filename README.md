# HeeForge MVP

Rust 기반 CLI-first AI 소설 생성 엔진 MVP다. 웹 UI 없이 `heeforge` 명령만으로 scene 생성, 리뷰, 수정, 승인, 상태 확인을 수행한다.

LLM 호출은 직접 OpenAI API 키/OAuth를 다루지 않고, 사용자가 미리 로그인한 `codex` CLI subprocess만 사용한다. `codex`가 실패한 경우 기본 동작은 즉시 에러를 반환하는 것이고, dummy fallback은 명시적으로 opt-in 했을 때만 허용한다.

HeeForge CLI UX는 두 가지 출력 모드를 제공한다.

- 기본 `text`: 사람이 읽기 쉬운 요약, 경로, 다음 추천 명령 출력
- `--format json`: Codex CLI, OpenClaw, 기타 LLM 에이전트가 안정적으로 해석할 수 있는 구조화 출력

실제 소설 데이터는 엔진 소스 디렉터리가 아니라 별도 워크스페이스에 저장된다. 워크스페이스 루트에는 사람용 작업 폴더가 펼쳐지고, 숨김 디렉터리 `.novel/`에는 상태/로그/메모리 같은 엔진 런타임 데이터가 저장된다.
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

scene 자체에는 manuscript-friendly `short_title`이 추가로 저장되며, scene 파일명 slug는 기본적으로 이 값을 사용한다.
chapter도 compiled `short_title`을 가지며 chapter 파일명 slug는 이 값을 사용한다.
디스크의 `story_memory.md`는 전체 이력을 유지하지만, planner/writer/expand-world 프롬프트에는 최근성과 신호가 높은 섹션 위주로 제한된 prompt view를 사용해 장편 누적 시 컨텍스트 폭주를 완화한다.

## 설치

빠른 설치:

```bash
curl -fsSL https://raw.githubusercontent.com/johwanghee/heeforge/main/install.sh | bash
```

기본 설치 경로는 `~/.local/bin/heeforge`다. 다른 경로에 설치하려면:

```bash
curl -fsSL https://raw.githubusercontent.com/johwanghee/heeforge/main/install.sh | \
  HEEFORGE_INSTALL_DIR=/usr/local/bin bash
```

특정 릴리스를 설치하려면:

```bash
curl -fsSL https://raw.githubusercontent.com/johwanghee/heeforge/main/install.sh | \
  HEEFORGE_VERSION=v0.1.0 bash
```

설치 스크립트는 현재 플랫폼을 감지하고 GitHub Releases에서 `heeforge-<target>.tar.gz` 자산을 내려받는다.

소스에서 직접 빌드하려면:

1. Rust stable 설치
2. `codex` CLI 설치
3. `codex login` 실행

실제 codex 기반 생성/리뷰/수정을 쓰려면 먼저 `codex login`이 되어 있어야 한다.
기본적으로 `codex exec` 호출은 120초 timeout이 걸려 있으며, 응답이 없으면 subprocess를 종료하고 명확한 에러를 반환한다.
기본값에서는 `codex` 실패가 조용히 placeholder prose로 바뀌지 않는다. placeholder output이 필요하면 전역 설정 `allow_dummy_fallback = true` 또는 `HEEFORGE_ALLOW_DUMMY=true`를 사용자가 직접 켜야 하고, 그 경우에도 CLI 출력과 로그에 fallback warning이 남는다.

작가용 최소 설정 순서:

1. 터미널에서 한 번 `codex login` 실행
2. `heeforge init <workspace>` 실행
3. 워크스페이스에서 `heeforge doctor` 실행
4. 첫 장면 전에 `01_Brief`, `03_StoryBible/Plot`, `03_StoryBible/Characters` 또는 `03_StoryBible/World`에 최소 문서 하나씩 채우기
5. `doctor`가 `ready`를 보여주면 HeeForge 설정은 끝
6. 필요하면 `~/.config/heeforge/config.toml` 열기
7. 실제 집필은 `allow_dummy_fallback = false` 유지
8. 폴더 흐름만 시험하고 싶을 때만 `allow_dummy_fallback = true` 사용

`codex_unavailable`가 나오면 대부분은 로그인 미완료 또는 인터넷/DNS/VPN/프록시 문제다. 이 경우 개발 지식보다도 먼저 `codex login`과 네트워크 상태를 확인하는 쪽이 맞다.

중요한 구분도 있다. Codex 같은 에이전트, IDE agent, 샌드박스 실행기 안에서 `heeforge`를 대신 호출하면 그 호스트가 자체 보안 승인창을 띄울 수 있다. 그 승인창은 HeeForge의 추가 설정 단계가 아니라, 그 호스트 환경의 정책이다. 사용자 본인 터미널에서 직접 `heeforge`를 실행할 때는 HeeForge가 별도의 승인 UX를 요구하지 않는다.

`status`와 `next-scene`은 이제 workspace의 브리프/바이블/플롯 문서를 읽어 준비도를 계산한다. 준비도가 낮으면 경고를 보여주고, planner/writer/editor/critic도 그 문서들을 실제 프롬프트에 사용한다.

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
./target/release/heeforge
```

## Release 배포

release 바이너리 배포 기준 권장 절차:

```bash
./scripts/package-release.sh x86_64-unknown-linux-gnu
./scripts/package-release.sh aarch64-unknown-linux-gnu
./scripts/package-release.sh x86_64-apple-darwin
./scripts/package-release.sh aarch64-apple-darwin
```

생성되는 자산 이름은 다음 규약을 따른다.

- `dist/heeforge-x86_64-unknown-linux-gnu.tar.gz`
- `dist/heeforge-aarch64-unknown-linux-gnu.tar.gz`
- `dist/heeforge-x86_64-apple-darwin.tar.gz`
- `dist/heeforge-aarch64-apple-darwin.tar.gz`
- 각 자산에 대응하는 `.sha256`

사용자 머신 수동 설치 예시:

```bash
tar -xzf dist/heeforge-x86_64-unknown-linux-gnu.tar.gz
install -m 755 heeforge /usr/local/bin/heeforge
heeforge init ~/novels/my-first-novel
```

GitHub tag를 `v*` 형식으로 푸시하면 `.github/workflows/release.yml`이 위 자산을 빌드해 Release에 첨부한다. `install.sh`는 이 자산 규약을 전제로 동작한다.

설치 직후 도움말 확인:

```bash
heeforge --help
heeforge next-scene --help
```

배포 전 최소 확인:

- `cargo test`
- `./scripts/package-release.sh <target>`
- `./target/<target>/release/heeforge init <workspace>`
- `./target/<target>/release/heeforge --workspace <workspace> next-scene`
- `./target/<target>/release/heeforge --workspace <workspace> --format json status`
- `HEEFORGE_DOWNLOAD_URL=file:///... ./install.sh` 로 로컬 설치 smoke check
- `codex login` 이후 실제 codex 경로 검증

## 사용법

설치된 바이너리 기준으로 새 워크스페이스 생성:

```bash
heeforge init ~/novels/my-first-novel
```

`init`은 누락된 필수값을 인터랙티브하게 질문한다. 자동화가 필요하면 flag로 넘기거나 `--no-input`을 쓸 수 있다.

```bash
heeforge init ~/novels/my-first-novel \
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

`heeforge init`은 워크스페이스용 `.gitignore`를 생성해서 엔진 내부 상태 파일만 제외하고, scene/chapter/story memory 같은 실제 소설 산출물은 Git에 포함할 수 있게 준비한다.
처음 실행 시 전역 설정 파일 `~/.config/heeforge/config.toml`도 없으면 기본값으로 생성된다.
필수값이 비어 있는 상태로 `init --no-input`을 수행한 경우 워크스페이스는 생성되지만 `next-scene` 전에 `novel.toml`을 채워야 한다.

생성되는 워크스페이스 골격은 다음과 같다.

```text
<workspace>/
├── 00_Inbox/
├── 01_Brief/
├── 02_Draft/
│   ├── Scenes/
│   ├── Chapters/
│   ├── Fragments/
│   └── Illustrations/
├── 03_StoryBible/
│   ├── Characters/
│   ├── World/
│   ├── Rules/
│   ├── Timeline/
│   └── Plot/
├── 04_Research/
│   ├── Sources/
│   ├── Notes/
│   └── References/
├── 05_LLM/
│   ├── Prompts/
│   ├── Outputs/
│   └── Sessions/
├── 06_Review/
│   ├── Feedback/
│   └── Revisions/
├── 07_Archive/
│   ├── Snapshots/
│   └── Deprecated/
├── 98_Templates/
├── README.md
├── novel.toml
└── .novel/
```

Git 확인용으로는 위 바깥 구조와 `novel.toml`을 커밋하고, `.novel/state/`, `.novel/logs/`, `.novel/memory/active_memory.md` 같은 런타임 데이터는 ignore하는 흐름을 권장한다.
`init`은 루트 `README.md`, 각 주요 섹션 `README.md`, 그리고 `98_Templates/`의 starter template 파일도 함께 생성한다.
Git을 잘 모르는 사용자라면 전역 설정 `workspace_auto_commit = true` 또는 `HEEFORGE_WORKSPACE_AUTO_COMMIT=true`로 workspace Git 자동 초기화/자동 커밋을 켤 수 있다. 이 경우 `init`, `next-scene`, `review`, `rewrite`, `approve`, `next-chapter`, `expand-world` 같은 변경 명령 뒤에 workspace repo가 자동으로 commit된다.
내부 구현은 workspace 제어 엔진과 Codex 소설 생성 backend를 분리했지만, 사용자는 계속 같은 `heeforge` 명령만 쓰면 된다. backend 분리는 사용자 절차를 늘리기 위한 것이 아니라, 향후 worker/bridge 구조를 숨긴 채 안정성을 높이기 위한 내부 경계다.

워크스페이스 안에서 작업:

```bash
cd ~/novels/my-first-novel
heeforge status
heeforge doctor
heeforge next-scene
heeforge review
heeforge rewrite scene_001_001 --instruction "대사를 더 날카롭게"
heeforge approve scene_001_001
heeforge next-chapter
heeforge expand-world
heeforge memory
heeforge show scene_001_001
```

현재 디렉터리가 워크스페이스가 아니거나 다른 위치에서 실행 중이면 `--workspace`로 명시할 수 있다.

```bash
heeforge --workspace ~/novels/my-first-novel status
```

LLM/자동화 연동 시에는 `--format json`을 권장한다.

```bash
heeforge --workspace ~/novels/my-first-novel --format json status
heeforge --workspace ~/novels/my-first-novel --format json next-scene
```

`--format json`에서 `init`을 실행하면 interactive prompt를 기다리지 않도록 자동으로 non-interactive 모드로 동작한다. 필요한 필드는 flag 또는 `novel.toml` 편집으로 채운다.

개발 중에는 소스 트리에서 그대로 실행할 수도 있다.

```bash
cargo run -- init ~/novels/my-first-novel
cargo run -- --workspace ~/novels/my-first-novel next-scene
```

권장: 소설 워크스페이스는 엔진 소스 저장소 밖에 둔다.

## 산출물과 로그

- scene markdown: `<workspace>/02_Draft/Scenes/<scene_id>-<slug>.md` (slug는 scene의 `short_title` 기준, 만들 수 없으면 `<scene_id>.md`)
- chapter markdown: `<workspace>/02_Draft/Chapters/chapter_<chapter>-<slug>.md` (slug는 compiled chapter `short_title` 기준, 만들 수 없으면 `chapter_<chapter>.md`)
- review 결과 JSON: `<workspace>/06_Review/Feedback/<scene_id>.json`
- rewrite 원본/수정본과 revision record: `<workspace>/06_Review/Revisions/<scene_id>/`
- scene 생성 로그: `<workspace>/.novel/logs/scene_generation/<scene_id>.json`
- opt-in prompt 로그: `<workspace>/.novel/logs/llm_prompts/*.json` (`HEEFORGE_LOG_PROMPTS=true` 또는 전역 `log_prompts = true`일 때만)
- chapter 생성 시 scene 번호가 1부터 연속인지 검증한다.

## 환경 변수

`.env.example` 참고.

- `HEEFORGE_CODEX_CMD`: 기본값 `codex`
- `HEEFORGE_CODEX_TIMEOUT_SECS`: 기본값 `120`
- `HEEFORGE_ALLOW_DUMMY`: 기본값 `false`
- `HEEFORGE_LOG_PROMPTS`: 기본값 `false`, `true`면 prompt/response 로그를 `.novel/logs/llm_prompts/`에 저장
- `HEEFORGE_WORKSPACE_AUTO_COMMIT`: 기본값 `false`, `true`면 workspace Git repo를 자동 초기화하고 변경 명령마다 auto-commit
- `HEEFORGE_INSTALL_DIR`: install script 기본값 `~/.local/bin`
- `HEEFORGE_VERSION`: install script 기본값 `latest`
- `HEEFORGE_REPO`: install script 기본값 `johwanghee/heeforge`
- `HEEFORGE_DOWNLOAD_URL`: install script 테스트/override용 직접 자산 URL

`HEEFORGE_ALLOW_DUMMY=false`로 두면 codex CLI가 없거나 로그인되지 않았을 때 명확한 에러를 반환한다.
`HEEFORGE_CODEX_TIMEOUT_SECS`로 실제 codex 응답 대기 시간을 조절할 수 있다.
환경 변수 우선순위는 전역 설정 파일보다 높다.

## 현재 MVP 범위

- 별도 워크스페이스 생성 및 `.novel/` 저장소 관리
- 전역 `config.toml`, 소설별 `novel.toml`, 내부 `workspace.json` 분리
- `init` 시 필수 메타 인터랙티브 입력 유도
- `--help` 예시 보강 및 `--format text|json` 이중 출력
- `curl ... | bash` 형태를 위한 `install.sh`와 release asset 규약
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
- `codex` subprocess timeout 및 hang 방지 강제 종료
- 최소 smoke test 및 runner retry test

## 다음 확장 포인트

- planner 출력 schema 검증 강화
- chapter/arc 단위 요약 memory 자동화
- 승인 이력 및 diff 로그
- agent별 prompt 템플릿 분리
- trait 기반 mock runner 주입
