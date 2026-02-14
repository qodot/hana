# hana 🌸

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

> 🚧 Under development

## License

MIT
