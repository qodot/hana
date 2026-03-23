# hana 설계 리뷰

날짜: 2026-02-24
소스: Claude Opus 분석 + 독립 리뷰(REVIEW_FP_INTERFACE) + Codex 검증을 통합 정리

---

## 운영 결함

### 1. sync 실패가 exit code에 반영되지 않음
- `sync::run`이 항상 `SyncOk`를 반환, I/O 실패를 `warnings`로만 누적 (sync.rs:88)
- `run_sync`는 무조건 exit code `0` 반환 (main.rs:170)
- CI/자동화에서 실제 실패를 감지할 수 없음

### 2. I/O 에러 유실 (`let _ = ...`)
- `clean_broken_symlinks`: 삭제 실패 무시 후 `cleaned` 목록에 포함 — 사용자에게 거짓 출력 (sync.rs:258-264)
- `broadcast_target_symlink`: `create_dir_all`, `remove_dir_all`, `remove_file` 실패 무시 (broadcast_target_symlink.rs:96-113)
- `collect_instruction`: `rename`/`symlink` 실패 시 `eprintln!` 후 소거, `SyncWarning`에 미반영 (sync.rs:363-379)

### 3. 디렉토리 스캔 엔트리 에러 소거
- `filter_map(|e| e.ok())`로 항목별 읽기 실패 무시 (collect_target_skills.rs:16, collect_source_skills.rs:12)

---

## 타입/인터페이스 설계

### 4. Result 구조 부적합 — 부분 성공 표현
- `BroadcastOk`/`BroadcastErr`, `MoveOk`/`MoveErr`가 같은 필드(`linked`, `tasks`)를 공유
- 호출부에서 Ok/Err를 동일하게 처리 (sync.rs:146-149)
- 의도적 설계이긴 하나, 단일 struct(`BroadcastResult { linked, conflicts, failed }`)가 더 정직함

### 5. `HashMap<String, TargetConfig>` 타입 안전성 누수
- `AgentName` enum이 있는데 key가 `String` (config.rs:129)
- `[target.cluade]` 같은 설정 오타가 파서에서 조용히 무시됨 (config.rs:189-216)

### 6. 익명 튜플 `(String, String)` 남용
- `SyncOk`의 `skills_linked`, `skills_collected` 등 — 어느 쪽이 skill이고 agent인지 타입으로 알 수 없음 (sync.rs:25-29)
- `status.rs`의 `Vec<(String, SkillState)>`도 `AgentName` 사용이 자연스러움

### 7. `Disabled` 상태 불일치
- instruction은 `Disabled` 상태가 있으나, skills는 비활성 타깃을 `Missing`으로 표기 (status.rs:82-83 vs 37-43)

---

## 모듈/구조 설계

### 8. helper → sync 역방향 의존
- `move_target_skills`와 `collect_source_skills`가 `crate::sync::SyncWarning`에 의존
- helper가 상위 모듈 타입에 의존하는 계층 위반

### 9. `global: bool` 오염
- Config의 8개 메서드가 모두 `global: bool`을 받고, `SourceConfig`/`TargetConfig`도 `*_path`와 `*_path_global` 쌍 반복
- 모드 조기 해석(resolved config)으로 제거 가능

### 10. `SyncOk` 평탄 구조 & 중복 계산
- 7개 필드를 평탄하게 나열, 내부 결과를 풀어서 재조립 (sync.rs:92-104)
- `resolve_target_destinations`가 `sync_instructions` 내에서 조건부 2회 호출 (sync.rs:286, 345)

---

## 아키텍처 개선 방향

### plan/apply 분리
- 순수 함수에서 `Vec<Operation>`(이동/링크/삭제/skip)을 산출
- 부수효과 함수에서 실행하고 `AppliedOutcome`을 수집
- dry-run과 실제 실행의 로직 공유, 테스트 단순화

### 에러 채널 통일
- `sync::run` → `Result<SyncOk, SyncError>` 또는 `SyncOk { fatal_errors, warnings }`
- `eprintln!` 직접 출력 전면 제거, 구조화된 반환으로 통일
- exit code 정책: `IoFailed` 존재 시 exit 1 여부 결정 필요

### 타입 정규화
- `HashMap<AgentName, TargetConfig>`, 익명 튜플 → 명명 struct
- skills/instructions 공통 상태 어휘(`Synced`, `Disabled`, `Conflict`, `Missing`, `Broken`)

---

## 우선순위

1. **exit code 미반영** (1번) — CI에서 실패 감지 불가, 실질적 버그
2. **I/O 에러 유실** (2번) — 사용자 출력 신뢰도 저하
3. **역방향 의존** (8번) — 모듈 구조 위반
4. **타입 안전성** (5번) — 설정 오류 조기 검출
5. 나머지 — 점진적 개선 가능
