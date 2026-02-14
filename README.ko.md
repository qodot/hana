# hana 🌸

**하나** — 코딩 에이전트 설정을 하나로 통합하는 CLI 도구

여러 AI 코딩 에이전트(Claude Code, Codex, Pi, OpenCode)의 스킬과 지침을 한 곳에서 관리하고 동기화한다.

## 왜 필요한가?

AI 코딩 에이전트마다 스킬과 지침의 저장 경로가 다르다:

| 에이전트 | 스킬 경로 | 지침 파일 |
|---------|----------|----------|
| Claude Code | `.claude/skills/` | `CLAUDE.md` |
| Codex | `.agents/skills/` | `AGENTS.md` |
| Pi | `.pi/skills/` | `PI.md` |
| OpenCode | `.opencode/skills/` | `AGENTS.md` |

하지만 모두 [Agent Skills](https://agentskills.io) 표준을 따른다. hana는 이 공통점을 이용해 하나의 소스에서 모든 에이전트로 동기화한다.

## 동작 방식

1. `.agents/skills/`를 소스 오브 트루스(source of truth)로 사용
2. 각 에이전트 경로에 **심볼릭 링크** 생성
3. 다른 에이전트에서 만든 새 스킬을 자동 감지해서 소스로 수집

```bash
hana init      # 설정 파일 생성
hana sync      # 스킬 & 지침 동기화
hana status    # 현재 동기화 상태 확인
```

## 지원 범위

- ✅ **스킬** — Agent Skills 표준 기반 심링크 동기화
- ✅ **지침** — 마크다운 기반 지침 파일 동기화
- 🚫 **명령어/훅** — 에이전트별 포맷이 달라 미지원

## 설치

> 🚧 개발 중

## 라이선스

MIT
