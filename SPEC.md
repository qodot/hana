# hana Spec

## Overview

hana is a CLI tool that manages skills and instructions for multiple AI coding agents from a single source, syncing them via symlinks.

## Supported Agents

| Agent | Skills (Project) | Skills (Global) | Instructions |
|-------|-----------------|-----------------|-------------|
| Claude Code | `.claude/skills/` | `~/.claude/skills/` | `CLAUDE.md` |
| Codex | `.agents/skills/` | `~/.agents/skills/` | `AGENTS.md` |
| Pi | `.pi/skills/` | `~/.pi/agent/skills/` | `PI.md` |
| OpenCode | `.opencode/skills/` | `~/.config/opencode/skills/` | `AGENTS.md` |

### Compatibility Notes

- OpenCode also scans `.claude/skills/` and `.agents/skills/` automatically.
- Pi lets you add other agents' skill paths in settings.
- Codex and OpenCode share the same instructions filename (`AGENTS.md`).

## Source of Truth

### Skills
- Project level: `.agents/skills/`
- Global level: `~/.agents/skills/`

This is the path defined by the Agent Skills standard (`agentskills.io`) and also Codex's default path.

### Instructions
- Project level: `AGENTS.md` (project root)

`AGENTS.md` is an [open standard](https://agents.md/) managed by the Agentic AI Foundation under the Linux Foundation. It was co-created by OpenAI Codex, Google Jules, Cursor, Amp, Factory, and others. Over 60k open-source projects use it.

`AGENTS.md` serves as the source of truth. Other agent instruction files are created as symlinks.

## Sync Method: Symlinks

All sync is done through symlinks. No file copying.

### Forward Sync (Source → Agents)

Symlink each skill directory from the source to each agent's path.

```
.agents/skills/my-skill/  ← source (real directory)
.claude/skills/my-skill   → .agents/skills/my-skill (symlink)
.pi/skills/my-skill       → .agents/skills/my-skill (symlink)
.opencode/skills/my-skill → .agents/skills/my-skill (symlink)
```

Codex uses the same path as the source, so no symlink is needed.

### Collection (Agents → Source)

Detects real directories (not symlinks) in each agent's path as new skills.

```
.pi/skills/new-skill/  ← real directory (created by Pi)
  1. Move to .agents/skills/new-skill/ (mv)
  2. Create symlink .pi/skills/new-skill → .agents/skills/new-skill
  3. Create symlinks in other agent paths too
```

### Instruction Sync (Project Level)

```
AGENTS.md   ← source (real file, open standard)
CLAUDE.md   → AGENTS.md (symlink)
```

Codex, OpenCode, and Pi read `AGENTS.md` directly, so no symlinks are needed for them.
In monorepos, subdirectory `AGENTS.md` files are handled the same way.

### Instruction Sync (Global Level)

The global source of truth is `~/.agents/AGENTS.md`, consistent with the skill source path (`~/.agents/skills/`).

| Agent | Global Instructions | Sync |
|-------|-------------------|------|
| Claude Code | `~/.claude/CLAUDE.md` | symlink (different filename) |
| Codex | `~/.codex/AGENTS.md` | symlink |
| OpenCode | `~/.config/opencode/AGENTS.md` | symlink |
| Pi | `~/.pi/agent/AGENTS.md` | symlink |

```
~/.agents/AGENTS.md              ← source (real file)
~/.claude/CLAUDE.md              → ~/.agents/AGENTS.md (symlink)
~/.codex/AGENTS.md               → ~/.agents/AGENTS.md (symlink)
~/.config/opencode/AGENTS.md     → ~/.agents/AGENTS.md (symlink)
~/.pi/agent/AGENTS.md            → ~/.agents/AGENTS.md (symlink)
```

#### Pi Global Instructions Note

Pi auto-loads `~/.pi/agent/AGENTS.md` as global instructions. It also supports:
- `~/.pi/agent/SYSTEM.md`: Full system prompt replacement
- `~/.pi/agent/APPEND_SYSTEM.md`: Append to system prompt
- See: [Pi README](https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent)

## Conflict Handling

### Skill Name Conflicts
When the same skill name exists in multiple agent paths:
- Print a warning.
- Ask the user which one to keep as the source.

### Existing File Conflicts
When a non-symlink file/directory already exists at a symlink target:
- Print a warning.
- Don't overwrite without `--force`.

## State Tracking

No lock files or state files. The filesystem is the state.

- Symlink → already synced
- Real directory → new skill (collection target)
- Broken symlink → source was deleted (cleanup target)

## CLI Commands

### `hana init`

Creates a `.agents/hana.toml` config file in the current directory.

Project-level config goes in `.agents/hana.toml`, global in `~/.agents/hana.toml`.

```toml
# .agents/hana.toml (project level)

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

All values default to `true`. Set to `false` to disable sync for a specific agent's skills or instructions.

`hana init --global` creates `~/.agents/hana.toml` with the same structure but global paths.

### `hana sync`

1. Read config (`.agents/hana.toml`)
2. Collection: detect new skills in agent paths → move to source
3. Forward sync: create symlinks from source to agent paths
4. Instruction sync: symlink source instructions to each agent's filename
5. Cleanup: remove broken symlinks
6. Print summary

```
$ hana sync
🌸 hana sync

Skills:
  ✅ my-skill → claude, pi, opencode
  🆕 new-skill (collected from pi) → claude, codex, opencode
  🔗 3 symlinks created

Instructions:
  ✅ CLAUDE.md → AGENTS.md
  ℹ️  AGENTS.md (used directly by Codex/OpenCode/Pi)

Done!
```

### `hana status`

Shows current sync state.

```
$ hana status
🌸 hana status

Skills:
  my-skill    ✅ claude ✅ codex ✅ pi ✅ opencode
  new-skill   ✅ claude ✅ codex ⚠️ pi(real) ❌ opencode

Instructions:
  AGENTS.md   ✅ source
  CLAUDE.md   ✅ symlink → AGENTS.md
```

### Options

| Option | Description |
|--------|------------|
| `--force` | Allow overwriting existing files |
| `--dry-run` | Print plan without making changes |
| `--global` | Use `~/.agents/hana.toml` for global sync |
| `--verbose` | Print detailed logs |

## Out of Scope

- **Commands**: Formats differ completely across agents
- **Hooks**: No common standard
- **MCP config**: Different JSON schemas per agent
- **Cloud sync**: Use git or cloud storage instead
- **Live file watching**: `hana sync` runs explicitly

## Tech Stack

- **Language**: Rust
- **Crate name**: `hanacli` (crates.io)
- **Binary name**: `hana`
- **Distribution**: `cargo install hanacli`, Homebrew tap for macOS
- **CI**: Multi-platform builds + automated releases via `cargo-dist`
