# cece

A fast CLI for managing workspaces of Git repositories and AI agents. Inspired by [Vibe Kanban](https://vibekanban.com/), but without the web interface.

Each workspace is a named collection of Git worktrees (one per repo) and Claude Code agents working on them. Cece keeps track of everything in a local SQLite database and optionally integrates with [Cmux](https://www.cmux.dev) to manage terminal sessions.

## Features

- **Workspaces** — group multiple repos together under a named workspace, each checked out as a Git worktree with per-repo branch selection
- **Branch templates** — configure a naming convention once (`{initials}-{ticket}-{desc}`), fill in the blanks at creation time
- **Per-repo branches** — each repo in a workspace can use a different branch, with "new from main" (fetches origin) or "new from current" options
- **Agents** — attach Claude Code agents to a workspace, track their session and last activity
- **Status dashboard** — see all workspaces, repos, and agent activity at a glance
- **Workspace templates** — save repo sets and branch patterns as reusable named templates, use them with `--template`
- **Workspace CLAUDE.md** — auto-generated instructions for Claude Code agents explaining the repo layout, worktree mechanics, and plans directory
- **Permissive agent settings** — auto-generated `.claude/settings.json` with sensible tool permissions (skip with `--no-settings`)
- **Editor integration** — open worktrees in IntelliJ IDEA, Zed, VS Code, or Cursor
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
# zsh (oh-my-zsh)
mkdir -p ~/.oh-my-zsh/completions
cece completions zsh > ~/.oh-my-zsh/completions/_cece
# then restart your shell or run: exec zsh

# zsh (without oh-my-zsh) — add ~/.zfunc to fpath in .zshrc first:
#   fpath=(~/.zfunc $fpath); autoload -Uz compinit && compinit
mkdir -p ~/.zfunc
cece completions zsh > ~/.zfunc/_cece

# bash
cece completions bash > /etc/bash_completion.d/cece

# fish
cece completions fish > ~/.config/fish/completions/cece.fish
```

## Quick start

```bash
# 1. Initialize cece (one-time setup)
cece init
#    → prompts for branch template (e.g. {initials}-{ticket}-{desc})
#    → prompts for Cmux integration (optional)

# 2. Create a workspace for your ticket
cece ws create auth-fix
#    → select repos (e.g. ~/dev/api, ~/dev/web)
#    → pick "new branch - from main" (fetches origin first)
#    → fill in template: initials=cr, ticket=OPEN-456, desc=auth-fix
#    → creates worktrees at ~/.cece/workspaces/auth-fix/api/ and .../web/
#    → generates CLAUDE.md and .claude/settings.json for agents

# 3. Open the workspace in your editor
cd ~/.cece/workspaces/auth-fix
cece code   # or: cece zed, cece idea, cece cursor

# 4. Spin up Claude Code agents to work on the repos
cece agent create backend
cece agent create frontend
#    → each agent gets its own Claude Code session (and Cmux tab if enabled)

# 5. Check on progress
cece status
#    auth-fix
#    Repo   Branch
#    api    cr-OPEN-456-auth-fix
#    web    cr-OPEN-456-auth-fix
#    Agent     Last Request              Last Response
#    backend   Fix the token refresh…    Done. Updated refresh_token()…
#    frontend  —                         —

# 6. When you're done, clean up
cece ws delete auth-fix
#    → removes worktrees, deletes branches, cleans up DB
```

## Reference

### Workspaces

```bash
cece ws create my-feature          # interactive: select repos, fill in branch template
cece ws create my-feature \
  --repos ~/dev/api ~/dev/web \
  --branch cr-OPEN-123-auth-fix    # non-interactive
cece ws create my-feature \
  --template feature-ticket        # use a saved template for repos + branch pattern
cece ws create my-feature \
  --no-settings                    # skip generating .claude/settings.json

cece ws info my-feature            # show workspace details (repos, branches, agents)
cece ws list                       # list all workspaces
cece ws switch my-feature          # cd to workspace dir, or switch Cmux workspace
cece ws add-repo --workspace my-feature  # add repos to an existing workspace
cece ws remove-repo --workspace my-feature  # remove a repo from a workspace
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

### Editors

```bash
cece idea     # open current worktree in IntelliJ IDEA
cece zed      # open current worktree in Zed
cece code     # open current worktree in VS Code
cece cursor   # open current worktree in Cursor
```

### Status

```bash
cece status   # show all workspaces, their repos, agents, and last activity
```

## How it works

- All state is stored in `~/.cece/cece.db` (SQLite)
- Each workspace repo is a [Git worktree](https://git-scm.com/docs/git-worktree) checked out to `~/.cece/workspaces/<workspace>/<repo>/`
- Each workspace gets a `CLAUDE.md` describing the repo layout and a `plans/` directory for working documents
- Each workspace gets `.claude/settings.json` with permissive tool permissions for agents (disable with `--no-settings`)
- Agent activity is tracked via Claude Code hooks (SessionStart, UserPromptSubmit, Stop)
- Cmux integration works via the `cmux` CLI (`cmux select-workspace`, `cmux new-tab`, `cmux select-tab`)

## License

MIT — see [LICENSE](LICENSE)
