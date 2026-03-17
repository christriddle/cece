# Cece - Claude Code Instructions

## Autonomy

Operate autonomously. Do not ask for confirmation before taking actions unless something is genuinely irreversible or destructive (e.g., dropping data, force-pushing to main). Prefer action over clarification.

## General

- Read `docs/plans/cece-1.md` for the original project plan. The CLI has grown beyond that doc — always check `src/cli/mod.rs` for the current command tree.
- Keep code simple, focused, and easily testable.

## CLI Commands

```
cece init                          # First-time setup (~/.cece, SQLite, hooks)
cece ws create <name>              # Create workspace (interactive repo/branch selection)
cece ws list                       # List all workspaces
cece ws delete <name>              # Delete workspace and its worktrees
cece ws switch <name>              # Switch to workspace (Cmux or prints path)
cece ws add-repo [--workspace X]   # Add repos to an existing workspace
cece agent create <name>           # Create agent in current workspace
cece agent list                    # List agents in current workspace
cece agent delete <name>           # Delete agent
cece agent switch <name>           # Switch to agent (Cmux tab)
cece agent logs <name>             # Show agent session history
cece agent watch <name>            # Block until agent is idle
cece template create <name>        # Create workspace template
cece template list                 # List templates
cece template delete <name>        # Delete template
cece list                          # List all workspaces and agents
cece status                        # Dashboard of workspaces, repos, agents
cece idea                          # Open current worktree in IntelliJ IDEA
cece completions <shell>           # Generate shell completions
cece hook ...                      # Internal hooks (called by Claude Code, hidden)
```

## Key Patterns

### Interactive prompts
The CLI uses `dialoguer` for all interactive input: `FuzzySelect` for single selection, `MultiSelect` for multi-selection, `Input` for text, `Confirm` for yes/no. Keep this consistent — don't mix in raw stdin reads.

### Workspace CLAUDE.md
`cece ws create` and `cece ws add-repo` auto-generate a `CLAUDE.md` in the workspace directory (`~/.cece/workspaces/<name>/CLAUDE.md`). This tells Claude Code agents about the repo layout, worktree mechanics, and where to put plans. The template lives in `write_workspace_claude_md()` in `src/cli/workspace.rs`. Update it when workspace structure changes.

### Workspace resolution
Workspaces are resolved from the current directory by matching against worktree paths in the DB. This works from inside a worktree **or** from the workspace root directory (parent of worktrees). See `find_by_worktree()` in `src/db/workspace.rs`.

### Git worktrees and branches
The `git` module (`src/git/mod.rs`) wraps all git operations. `BranchTarget` is the core type — `New { name, start_point }` or `Existing(name)`. New branches can specify a start point (e.g. `origin/main` after a fetch). Per-repo branch selection happens during workspace creation.

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
