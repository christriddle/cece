# Cece Plan 1

Cece is a commandline application written in Rust that allows you to manage workspaces of Git repositories and AI agents.

It is based on [https://vibekanban.com/](Vibe) Kanban, but relies on a simple CLI rather than a web interface.

## Features
- A simple and fast CLI
- Create, manage, and delete workspaces
- A workspace can contain multiple repositories, created as Git worktrees.
- A workspace can contain multiple AI agents acting on those repositories.
- The first version will support Claude Code only.
- The first version will support only MacOS.
- Git workspaces are created with configurable branch names, based on a template. For example, it must support the format <user_initials>-<jira-ticket-number>-<short-description>.
- AI agents are created with configurable names.
- It can integrate with [https://www.cmux.dev/docs/api](Cmux) for creating workspaces and agent tabs.
- It will allow the user to create workspaces, query the active workspaces, switch to workspaces, and delete workspaces. 
- It will rely on Claude skills to perform tasks on the agent, unlike vibe Kanban which has ways to perform certain tasks like pull requests.
- It will remember the list of repositories you can add to workspaces as you add them.
- When creating a workspace, you can interactively specify (or via paramaters) the repositories you want to include in the workspace. These will consist of paths to the repos checked out on your machine.
- When adding a repo to the workspace, it will defaultly use master or main, but you can specify a different branch.
- Listing agents in a workspace will show the session id of the agent, and the last request it was processing (if any). Short version only.
- Cmux integration will allow you to open workspaces (in Cmux workspaces) and agents in Cmux tabs (one per tab).
- This will be an open source project. So create the repo in the standard way for such an open source project. Licence is MIT.
- The code should be easily extendable to support new commands and other agents or integrations.
- The code should be easily testable, and have sufficient test coverage. 
- Practice modern and idiomatic Rust.
- CI pipelines should be able to build and release the CLI in GitHub.
- Github repo is here: https://github.com/christriddle/cece

## Example usage

```bash
cece init # Creates a directory and database for your Cece development environment.

cece ws create my-workspace # Creates a new workspace interactively. This will allow you to select the repositories you want to include in the workspace.
cece ws switch my-workspace # Switches to the workspace directory. If using Cmux, it will open the workspace in the existing Cmux workspace.
cece ws delete my-workspace # Deletes the workspace.
cece ws list # Lists all workspaces.

cece agent create my-agent # Creates a new agent in the current workspace. If using Cmux, it will open the agent in a new Cmux tab.
cece agent switch my-agent # Opens the Claude Code agent in the current workspace. If using Cmux, it will open the agent in the existing Cmux tab.
cece agent delete my-agent # Deletes the agent in the current workspace.
cece agent list # Lists all agents in the current workspace.


```
