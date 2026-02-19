# Repository Guidelines

## Project Structure & Module Organization
`hana` is a Rust CLI crate. Core code lives in `src/`:
- `src/main.rs`: CLI entrypoint and command routing (`init`, `sync`, `status`)
- `src/init.rs`, `src/sync.rs`, `src/status.rs`, `src/config.rs`, `src/agents.rs`: command logic and config/domain types
- `src/helper/`: shared filesystem/symlink helpers used by sync flows

Documentation lives in `README.md`, `README.ko.md`, `SPEC.md`, and `SPEC.ko.md`. Build artifacts are generated in `target/`.

## Build, Test, and Development Commands
Use Cargo from the repository root:
- `cargo run -- --help`: show CLI usage
- `cargo run -- init --dry-run`: preview generated config without writing files
- `cargo test`: run all unit tests (currently in-module `#[cfg(test)]` blocks)
- `cargo fmt`: apply standard Rust formatting
- `cargo clippy --all-targets --all-features -- -D warnings`: enforce lint-clean code
- `cargo build --release`: produce optimized binary

## Coding Style & Naming Conventions
Follow idiomatic Rust and keep code `rustfmt`-clean.
- Indentation: 4 spaces (Rust default)
- Naming: `snake_case` for files/functions/modules, `PascalCase` for structs/enums, `SCREAMING_SNAKE_CASE` for constants
- Keep command result/error types explicit and colocated with their module when possible
- Prefer small helper functions in `src/helper/` for shared filesystem behavior

## Testing Guidelines
Tests are colocated with implementation (no separate `tests/` directory currently). Use `#[cfg(test)] mod tests` and descriptive test names like `sync_collects_new_skill_from_pi`.
- Use `tempfile::TempDir` for filesystem scenarios
- Cover both normal and `--dry-run`/`--force` paths
- Run `cargo test` before opening a PR

## Commit & Pull Request Guidelines
Git history favors Conventional Commit-style prefixes:
- `feat: ...`, `fix: ...`, `refactor: ...`, `test: ...`, `docs: ...`, `rename: ...`

Keep commits focused and include behavior impact in the subject (e.g., conflict handling, dry-run behavior). For PRs:
- Provide a concise summary and rationale
- Link related issue/spec when available
- List validation steps run (`cargo test`, `cargo fmt`, `cargo clippy`)
- Include CLI output snippets when changing user-facing behavior

## Configuration & Safety Notes
`hana` manages symlinks across agent directories. Prefer `--dry-run` first for `init`/`sync`, then apply with explicit `--force` only when replacing existing files is intended.
