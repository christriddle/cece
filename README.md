# cece

A fast CLI for managing workspaces of Git repositories and AI agents. Inspired by [Vibe Kanban](https://vibekanban.com/), but without the web interface.

Each workspace is a named collection of Git worktrees (one per repo) and Claude Code agents working on them. Cece keeps track of everything in a local SQLite database and optionally integrates with [Cmux](https://www.cmux.dev) to manage terminal sessions.

## Features

- **Workspaces** — group multiple repos together under a named workspace, each checked out as a Git worktree on a shared branch
- **Branch templates** — configure a naming convention once (`{initials}-{ticket}-{desc}`), fill in the blanks at creation time
- **Agents** — attach Claude Code agents to a workspace, track their session and last activity
- **Status dashboard** — see all workspaces, repos, and agent activity at a glance
- **Workspace templates** — save repo sets and branch patterns as reusable named templates
- **Cmux integration** — switch workspaces and open agent tabs directly in Cmux
- **Shell completions** — tab-complete workspace and agent names in bash, zsh, or fish

## Requirements

- macOS
- Rust 1.75+ (for building from source)
- Git
- [Cmux](https://www.cmux.dev) (optional)

## Installation

```bash
cargo install --git https://github.com/christriddle/cece
```

To update to the latest version:

```bash
cargo install --git https://github.com/christriddle/cece --force
```

Or build from source:

```bash
git clone https://github.com/christriddle/cece
cd cece
cargo build --release
# binary at ./target/release/cece
```

### Shell completions

```bash
# zsh
cece completions zsh > ~/.zfunc/_cece

# bash
cece completions bash > /etc/bash_completion.d/cece

# fish
cece completions fish > ~/.config/fish/completions/cece.fish
```

## Getting started

```bash
cece init
```

This creates `~/.cece/cece.db` and prompts for:
- **Branch template** — e.g. `{initials}-{ticket}-{desc}` (placeholders are filled in interactively when creating a workspace)
- **Cmux integration** — whether to use Cmux for workspace and agent tab switching

## Usage

### Workspaces

```bash
cece ws create my-feature          # interactive: select repos, fill in branch template
cece ws create my-feature \
  --repos ~/dev/api ~/dev/web \
  --branch cr-OPEN-123-auth-fix    # non-interactive

cece ws list                       # list all workspaces
cece ws switch my-feature          # cd to workspace dir, or switch Cmux workspace
cece ws delete my-feature          # remove worktrees and DB record
```

### Agents

```bash
cece agent create my-agent --workspace my-feature   # create agent, open Cmux tab if configured
cece agent list --workspace my-feature              # list agents and last activity
cece agent switch my-agent --workspace my-feature   # focus Cmux tab
cece agent logs my-agent --workspace my-feature     # show recent session history
cece agent watch my-agent --workspace my-feature    # block until agent goes idle (30 min timeout)
cece agent delete my-agent --workspace my-feature   # remove agent record
```

### Templates

```bash
cece template create feature-ticket   # save repos + branch pattern as a reusable template
cece template list
cece template delete feature-ticket
```

### Status

```bash
cece status   # show all workspaces, their repos, agents, and last activity
```

## How it works

- All state is stored in `~/.cece/cece.db` (SQLite)
- Each workspace repo is a [Git worktree](https://git-scm.com/docs/git-worktree) checked out to `~/.cece/workspaces/<workspace>/<repo>/`
- Agent activity is tracked by reading Claude Code's session files at `~/.claude/projects/`
- Cmux integration works via the `cmux` CLI (`cmux select-workspace`, `cmux new-tab`, `cmux select-tab`)

## License

MIT — see [LICENSE](LICENSE)
