# Cece - Claude Code Instructions

## Autonomy

Operate autonomously. Do not ask for confirmation before taking actions unless something is genuinely irreversible or destructive (e.g., dropping data, force-pushing to main). Prefer action over clarification.

## General

- Read `docs/plans/cece-1.md` for the project plan and feature requirements.
- Keep code simple, focused, and easily testable.

## Rust Style

### Error Handling
- Use `thiserror` to define typed domain errors in library code.
- Use `anyhow` for application-level error propagation (CLI entry points, command handlers).
- Never use `.unwrap()` or `.expect()` in production code paths. Reserve them for tests and truly impossible states (with a comment explaining why).
- Propagate errors with `?`. Avoid `match` on `Result` just to re-wrap.

### Types and Design
- Prefer enums over stringly-typed state. Model domain concepts explicitly.
- Use the newtype pattern to give primitive values semantic meaning (e.g., `WorkspaceId(String)`).
- Derive `Debug`, `Clone`, `PartialEq` where sensible. Don't derive what you don't need.
- Avoid `clone()` in hot paths; use references and lifetimes where it makes sense.
- Use `Option` for absent values, not sentinel values like `""` or `-1`.

### Async
- Use `tokio` as the async runtime if async is needed.
- Prefer `async`/`await` over manual `Future` implementations.
- Keep async boundaries at the edges (CLI handlers, I/O). Core logic should be sync where possible.

### CLI
- Use `clap` with the derive API (`#[derive(Parser, Subcommand, Args)]`).
- Generate shell completions via Clap's built-in completion support.
- Print user-facing output with `println!`/`eprintln!`. Use `eprintln!` for errors and diagnostics.
- Use a consistent output style: plain text for machine-readable output, human-friendly formatting for interactive use.

### Database
- Use `rusqlite` for SQLite access.
- Run migrations on startup via embedded SQL files or `include_str!`.
- Keep SQL in dedicated `.sql` files or clearly labelled `const` strings, not inline ad-hoc strings scattered through logic.

### Modules and Structure
- One concept per module. Keep modules small and focused.
- Separate concerns: `db/`, `cmux/`, `workspace/`, `agent/`, `config/`, `cli/`.
- Expose a minimal public API from each module. Default to `pub(crate)`.
- Put integration tests in `tests/`. Put unit tests in `#[cfg(test)]` blocks in the same file as the code under test.

### Tooling
- All code must pass `cargo clippy -- -D warnings` with no suppressions unless justified with a comment.
- All code must be formatted with `cargo fmt`.
- Run `cargo test` before considering any task complete.
