# hana 스펙 문서

## 개요

hana는 여러 AI 코딩 에이전트의 스킬과 지침을 하나의 소스에서 관리하고 심볼릭 링크로 동기화하는 CLI 도구이다.

## 지원 에이전트

| 에이전트 | 스킬 경로 (프로젝트) | 스킬 경로 (글로벌) | 지침 파일 |
|---------|-------------------|------------------|----------|
| Claude Code | `.claude/skills/` | `~/.claude/skills/` | `CLAUDE.md` |
| Codex | `.agents/skills/` | `~/.agents/skills/` | `AGENTS.md` |
| Pi | `.pi/skills/` | `~/.pi/agent/skills/` | `PI.md` |
| OpenCode | `.opencode/skills/` | `~/.config/opencode/skills/` | `AGENTS.md` |

### 호환성 참고

- OpenCode는 `.claude/skills/`와 `.agents/skills/`도 자동 스캔한다.
- Pi는 settings에서 다른 에이전트의 스킬 경로를 추가할 수 있다.
- Codex와 OpenCode는 지침 파일명이 동일(`AGENTS.md`)하다.

## 소스 오브 트루스

### 스킬
- 프로젝트 레벨: `.agents/skills/`
- 글로벌 레벨: `~/.agents/skills/`

Agent Skills 표준(`agentskills.io`)의 경로이자 Codex의 기본 경로를 소스로 사용한다.

### 지침
- 프로젝트 레벨: `.agents/instructions.md`

하나의 마크다운 파일을 각 에이전트의 지침 파일명으로 심링크한다.

## 동기화 방식: 심볼릭 링크

모든 동기화는 심볼릭 링크로 수행한다. 파일 복사를 하지 않는다.

### 정방향 동기화 (소스 → 에이전트)

소스의 스킬 디렉토리를 각 에이전트 경로에 심링크로 생성한다.

```
.agents/skills/my-skill/  ← 소스 (실제 디렉토리)
.claude/skills/my-skill   → .agents/skills/my-skill (심링크)
.pi/skills/my-skill       → .agents/skills/my-skill (심링크)
.opencode/skills/my-skill → .agents/skills/my-skill (심링크)
```

Codex는 소스 경로와 동일하므로 심링크를 생성하지 않는다.

### 역방향 수집 (에이전트 → 소스)

각 에이전트 경로에서 심링크가 아닌 실제 디렉토리를 새로운 스킬로 감지한다.

```
.pi/skills/new-skill/  ← 실제 디렉토리 (Pi가 생성)
  1. .agents/skills/new-skill/로 이동 (mv)
  2. .pi/skills/new-skill → .agents/skills/new-skill 심링크 생성
  3. 다른 에이전트 경로에도 심링크 생성
```

### 지침 동기화

```
.agents/instructions.md  ← 소스 (실제 파일)
CLAUDE.md                → .agents/instructions.md (심링크)
AGENTS.md                → .agents/instructions.md (심링크)
PI.md                    → .agents/instructions.md (심링크)
```

## 충돌 처리

### 스킬 이름 충돌
동일한 이름의 스킬이 여러 에이전트 경로에서 발견된 경우:
- 경고를 출력한다.
- 사용자에게 어떤 것을 소스로 채택할지 선택을 요청한다.

### 기존 파일 충돌
심링크를 생성할 위치에 이미 심링크가 아닌 파일/디렉토리가 존재하는 경우:
- 경고를 출력한다.
- `--force` 플래그 없이는 덮어쓰지 않는다.

## 상태 추적

별도의 상태 파일(lock file)을 사용하지 않는다. 파일시스템 자체가 상태이다.

- 심링크 → 이미 동기화됨
- 실제 디렉토리 → 새로운 스킬 (역방향 수집 대상)
- 깨진 심링크 → 소스가 삭제됨 (정리 대상)

## CLI 명령어

### `hana init`

현재 디렉토리에 hana 설정 파일(`hana.toml`)을 생성한다.

```toml
# hana 설정 파일

# 스킬 소스 디렉토리
[skills]
source = ".agents/skills"

# 지침 소스 파일
[instructions]
source = ".agents/instructions.md"

# 동기화 대상 에이전트
[targets]
claude = true
codex = true       # 소스와 동일하면 스킵
pi = true
opencode = true

# 글로벌 동기화
[global]
enabled = false
source = "~/.agents/skills"
```

### `hana sync`

1. 설정 파일(`hana.toml`) 읽기
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
  🔗 3개 심링크 생성

지침 동기화:
  ✅ CLAUDE.md → .agents/instructions.md
  ✅ AGENTS.md → .agents/instructions.md
  ✅ PI.md → .agents/instructions.md

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
  CLAUDE.md   ✅ 심링크
  AGENTS.md   ✅ 심링크
  PI.md       ❌ 없음
```

### 옵션

| 옵션 | 설명 |
|------|------|
| `--force` | 기존 파일 덮어쓰기 허용 |
| `--dry-run` | 실제 변경 없이 계획만 출력 |
| `--global` | 글로벌 스킬도 동기화 |
| `--verbose` | 상세 로그 출력 |

## 범위 밖 (비지원)

- **명령어(commands)**: 에이전트별 포맷이 완전히 다름
- **훅(hooks)**: 공통 표준 없음
- **MCP 설정**: 에이전트별 JSON 스키마가 다름
- **클라우드 동기화**: git 또는 클라우드 스토리지 사용 권장
- **양방향 실시간 감시**: `hana sync`를 명시적으로 실행하는 방식

## 기술 스택

> 미정. 후보:
> - TypeScript (npm 배포 용이, 에이전트 도구 생태계와 일관)
> - Rust (빠르고 단일 바이너리)
> - Go (단일 바이너리, 심플)
