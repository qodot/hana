# hana(하나) 🌸

[![Crates.io](https://img.shields.io/crates/v/hanacli)](https://crates.io/crates/hanacli)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**hana** — A CLI tool that unifies coding agent configurations into a single source.

Manage and sync skills and instructions across multiple AI coding agents (Claude Code, Codex, Pi, OpenCode) from one place.

## Why?

Each AI coding agent stores skills and instructions in different paths:

| Agent | Skills Path | Instructions File |
|-------|-------------|-------------------|
| Claude Code | `.claude/skills/` | `CLAUDE.md` |
| Codex | `.agents/skills/` | `AGENTS.md` |
| Pi | `.pi/skills/` | `PI.md` |
| OpenCode | `.opencode/skills/` | `AGENTS.md` |

They all follow the [Agent Skills](https://agentskills.io) standard. hana uses this common ground to sync everything from a single source.

Without hana, you have to manually keep each agent's skills and instructions in sync. hana solves this with:

- **Single source of truth** — `.agents/skills/` and `AGENTS.md` are the canonical source. All other agent paths are symlinks pointing back to it.
- **Reverse collection** — When an agent creates a new skill in its own directory (e.g., `.claude/skills/new-skill/`), hana detects it and collects it back to the source automatically.

## How It Works

1. Uses `.agents/skills/` as the source of truth
2. Creates **symlinks** to each agent's path
3. Detects new skills created by other agents and collects them back to the source

```bash
hana init      # Create config file
hana sync      # Sync skills & instructions
hana status    # Check current sync state
```

## Scope

- ✅ **Skills** — Symlink-based sync following the Agent Skills standard
- ✅ **Instructions** — Markdown-based instruction file sync
- 🚫 **Commands/Hooks** — Not supported due to incompatible formats across agents

## Install

Install from crates.io:

```bash
cargo install hanacli
```

After installation, use the `hana` command:

```bash
hana --help
```

## License

MIT
