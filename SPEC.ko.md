# hana 스펙

## 개요

hana는 여러 AI 코딩 에이전트의 스킬과 지침을 하나의 소스에서 관리하고 심볼릭 링크로 동기화하는 CLI 도구다.

## 지원 에이전트

| 에이전트 | 스킬 경로 (프로젝트) | 스킬 경로 (글로벌) | 지침 파일 |
|---------|-------------------|------------------|----------|
| Claude Code | `.claude/skills/` | `~/.claude/skills/` | `CLAUDE.md` |
| Codex | `.agents/skills/` | `~/.agents/skills/` | `AGENTS.md` |
| Pi | `.pi/skills/` | `~/.pi/agent/skills/` | `PI.md` |
| OpenCode | `.opencode/skills/` | `~/.config/opencode/skills/` | `AGENTS.md` |

### 호환성 참고

- OpenCode는 `.claude/skills/`와 `.agents/skills/`도 자동 스캔한다.
- Pi는 설정에서 다른 에이전트의 스킬 경로를 추가할 수 있다.
- Codex와 OpenCode는 지침 파일명이 동일하다(`AGENTS.md`).

## 소스 오브 트루스

### 스킬
- 프로젝트 레벨: `.agents/skills/`
- 글로벌 레벨: `~/.agents/skills/`

Agent Skills 표준(`agentskills.io`)의 경로이자 Codex의 기본 경로를 소스로 사용한다.

### 지침
- 프로젝트 레벨: `AGENTS.md` (프로젝트 루트)

`AGENTS.md`는 [오픈 표준](https://agents.md/)으로, Linux Foundation 산하 Agentic AI Foundation에서 관리한다. OpenAI Codex, Google Jules, Cursor, Amp, Factory 등이 공동으로 만들었고, 60k 이상의 오픈소스 프로젝트가 사용 중이다.

`AGENTS.md`를 소스 오브 트루스로 쓰고, 다른 에이전트 지침 파일은 심링크로 만든다.

## 동기화 방식: 심볼릭 링크

모든 동기화는 심볼릭 링크로 수행한다. 파일 복사는 하지 않는다.

### 정방향 동기화 (소스 → 에이전트)

소스의 스킬 디렉토리를 각 에이전트 경로에 심링크로 만든다.

```
.agents/skills/my-skill/  ← 소스 (실제 디렉토리)
.claude/skills/my-skill   → .agents/skills/my-skill (심링크)
.pi/skills/my-skill       → .agents/skills/my-skill (심링크)
.opencode/skills/my-skill → .agents/skills/my-skill (심링크)
```

Codex는 소스 경로와 동일하므로 심링크를 만들지 않는다.

### 역방향 수집 (에이전트 → 소스)

각 에이전트 경로에서 심링크가 아닌 실제 디렉토리를 새 스킬로 감지한다.

```
.pi/skills/new-skill/  ← 실제 디렉토리 (Pi가 생성)
  1. .agents/skills/new-skill/로 이동 (mv)
  2. .pi/skills/new-skill → .agents/skills/new-skill 심링크 생성
  3. 다른 에이전트 경로에도 심링크 생성
```

### 지침 동기화 (프로젝트 레벨)

```
AGENTS.md   ← 소스 (실제 파일, 오픈 표준)
CLAUDE.md   → AGENTS.md (심링크)
```

Codex, OpenCode, Pi는 `AGENTS.md`를 직접 읽으므로 심링크가 필요 없다.
모노레포에서는 하위 디렉토리의 `AGENTS.md`도 같은 방식으로 처리한다.

### 지침 동기화 (글로벌 레벨)

글로벌 지침의 소스 오브 트루스는 `~/.agents/AGENTS.md`다. 스킬 소스 경로(`~/.agents/skills/`)와 일관된 위치를 사용한다.

| 에이전트 | 글로벌 지침 파일 | 동기화 방식 |
|---------|----------------|-----------|
| Claude Code | `~/.claude/CLAUDE.md` | 심링크 (파일명 다름) |
| Codex | `~/.codex/AGENTS.md` | 심링크 |
| OpenCode | `~/.config/opencode/AGENTS.md` | 심링크 |
| Pi | `~/.pi/agent/AGENTS.md` | 심링크 |

```
~/.agents/AGENTS.md              ← 소스 (실제 파일)
~/.claude/CLAUDE.md              → ~/.agents/AGENTS.md (심링크)
~/.codex/AGENTS.md               → ~/.agents/AGENTS.md (심링크)
~/.config/opencode/AGENTS.md     → ~/.agents/AGENTS.md (심링크)
~/.pi/agent/AGENTS.md            → ~/.agents/AGENTS.md (심링크)
```

#### Pi 글로벌 지침 참고

Pi는 `~/.pi/agent/AGENTS.md`를 글로벌 지침으로 자동 로드한다. 추가로 다음도 지원한다:
- `~/.pi/agent/SYSTEM.md`: 시스템 프롬프트 전체 교체
- `~/.pi/agent/APPEND_SYSTEM.md`: 시스템 프롬프트에 추가
- 참고: [Pi README](https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent)

## 충돌 처리

### 스킬 이름 충돌
같은 이름의 스킬이 여러 에이전트 경로에서 발견될 경우:
- 경고를 출력한다.
- 어느 쪽을 소스로 쓸지 사용자에게 묻는다.

### 기존 파일 충돌
심링크를 만들 위치에 이미 심링크가 아닌 파일/디렉토리가 있을 경우:
- 경고를 출력한다.
- `--force` 없이는 덮어쓰지 않는다.

## 상태 추적

별도의 상태 파일(lock file)을 사용하지 않는다. 파일시스템 자체가 상태다.

- 심링크 → 이미 동기화됨
- 실제 디렉토리 → 새 스킬 (역방향 수집 대상)
- 깨진 심링크 → 소스가 삭제됨 (정리 대상)

## CLI 명령어

### `hana init`

현재 디렉토리에 `.agents/hana.toml` 설정 파일을 만든다.

프로젝트 레벨은 `.agents/hana.toml`, 글로벌 레벨은 `~/.agents/hana.toml`에 저장한다.

```toml
# .agents/hana.toml (프로젝트 레벨)

[skills]
source = ".agents/skills"

[instructions]
source = "AGENTS.md"

[targets.claude]
skills = true
instructions = true

[targets.codex]
skills = true
instructions = true

[targets.pi]
skills = true
instructions = true

[targets.opencode]
skills = true
instructions = true
```

기본값은 모두 `true`다. 특정 에이전트의 스킬이나 지침 동기화를 끄려면 `false`로 설정한다.

`hana init --global`은 `~/.agents/hana.toml`을 만든다. 구조는 같고 경로만 글로벌 기준이다.

### `hana sync`

1. 설정 파일(`.agents/hana.toml`) 읽기
2. 역방향 수집: 각 에이전트 경로에서 새 스킬 감지 → 소스로 이동
3. 정방향 동기화: 소스에서 각 에이전트 경로로 심링크 생성
4. 지침 동기화: 소스 지침 파일을 각 에이전트 지침 파일명으로 심링크
5. 정리: 깨진 심링크 제거
6. 결과 요약 출력

```
$ hana sync
🌸 hana sync

스킬 동기화:
  ✅ my-skill → claude, pi, opencode
  🆕 new-skill (pi에서 수집) → claude, codex, opencode
  🔗 심링크 3개 생성

지침 동기화:
  ✅ CLAUDE.md → AGENTS.md
  ℹ️  AGENTS.md (Codex/OpenCode/Pi 직접 사용)

완료!
```

### `hana status`

현재 동기화 상태를 보여준다.

```
$ hana status
🌸 hana status

스킬:
  my-skill    ✅ claude ✅ codex ✅ pi ✅ opencode
  new-skill   ✅ claude ✅ codex ⚠️ pi(실제) ❌ opencode

지침:
  AGENTS.md   ✅ 소스
  CLAUDE.md   ✅ 심링크 → AGENTS.md
```

### 옵션

| 옵션 | 설명 |
|------|------|
| `--force` | 기존 파일 덮어쓰기 허용 |
| `--dry-run` | 실제 변경 없이 계획만 출력 |
| `--global` | `~/.agents/hana.toml` 기준으로 글로벌 동기화 |
| `--verbose` | 상세 로그 출력 |

## 범위 밖 (비지원)

- **명령어(commands)**: 에이전트별 포맷이 완전히 다름
- **훅(hooks)**: 공통 표준 없음
- **MCP 설정**: 에이전트별 JSON 스키마가 다름
- **클라우드 동기화**: git이나 클라우드 스토리지 사용 권장
- **양방향 실시간 감시**: `hana sync`를 명시적으로 실행하는 방식

## 기술 스택

- **언어**: Rust
- **크레이트 이름**: `hanacli` (crates.io)
- **바이너리 이름**: `hana`
- **배포**: `cargo install hanacli`, macOS는 Homebrew tap 추가 제공
- **CI**: `cargo-dist`로 멀티 플랫폼 빌드 + 릴리스 자동화
