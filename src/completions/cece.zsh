#compdef cece

_cece_workspace_names() {
    local -a names
    names=(${(f)"$(cece _complete workspaces 2>/dev/null)"})
    _describe 'workspace' names
}

_cece_agent_names() {
    local ws="$1"
    local -a names
    names=(${(f)"$(cece _complete agents "$ws" 2>/dev/null)"})
    _describe 'agent' names
}

_cece_ws() {
    local -a ws_commands
    ws_commands=(
        'create:Create a new workspace'
        'list:List all workspaces'
        'info:Show details of a specific workspace'
        'delete:Delete a workspace and its worktrees'
        'switch:Switch to a workspace'
        'add-repo:Add repos to an existing workspace'
        'remove-repo:Remove a repo from an existing workspace'
    )

    if (( CURRENT == 2 )); then
        _describe 'ws command' ws_commands
        return
    fi

    case "$words[2]" in
        create)
            if (( CURRENT == 3 )); then
                _message 'workspace name'
            else
                _arguments \
                    '--repos[Repo paths]:*:repo path:_files -/' \
                    '--branch[Branch name override]:branch:' \
                    '--template[Use a saved template]:template:' \
                    '--no-settings[Skip generating .claude/settings.json]'
            fi
            ;;
        delete|switch)
            if (( CURRENT == 3 )); then
                _cece_workspace_names
            fi
            ;;
        info)
            if (( CURRENT == 3 )); then
                _cece_workspace_names
            fi
            ;;
        add-repo|remove-repo)
            _arguments \
                '--workspace[Workspace name]:workspace:_cece_workspace_names'
            ;;
    esac
}

_cece_agent() {
    local -a agent_commands
    agent_commands=(
        'create:Create a new agent'
        'list:List agents in a workspace'
        'delete:Delete an agent'
        'switch:Switch to an agent'
        'logs:Show agent session history'
        'watch:Block until agent is idle'
    )

    if (( CURRENT == 2 )); then
        _describe 'agent command' agent_commands
        return
    fi

    case "$words[2]" in
        create|delete|switch|logs|watch)
            if (( CURRENT == 3 )); then
                # Try to get workspace from --workspace flag, otherwise use cwd
                local ws_flag="${${(M)words:#--workspace}:+${words[$words[(i)--workspace]+1]}}"
                if [[ -n "$ws_flag" ]]; then
                    _cece_agent_names "$ws_flag"
                else
                    _message 'agent name'
                fi
            else
                _arguments \
                    '--workspace[Workspace name]:workspace:_cece_workspace_names'
            fi
            ;;
        list)
            _arguments \
                '--workspace[Workspace name]:workspace:_cece_workspace_names'
            ;;
    esac
}

_cece_template() {
    local -a template_commands
    template_commands=(
        'create:Create a new workspace template'
        'list:List workspace templates'
        'delete:Delete a workspace template'
    )

    if (( CURRENT == 2 )); then
        _describe 'template command' template_commands
        return
    fi
}

_cece() {
    local -a commands
    commands=(
        'init:Initialize cece in your home directory'
        'ws:Manage workspaces'
        'agent:Manage agents in the current workspace'
        'template:Manage workspace templates'
        'list:List all workspaces and their agents'
        'status:Show status of all workspaces and agents'
        'idea:Open a worktree in IntelliJ IDEA'
        'zed:Open a worktree in Zed'
        'code:Open a worktree in VS Code'
        'cursor:Open a worktree in Cursor'
        'completions:Generate shell completions'
    )

    if (( CURRENT == 2 )); then
        _describe 'cece command' commands
        return
    fi

    case "$words[2]" in
        ws) shift words; (( CURRENT-- )); _cece_ws ;;
        agent) shift words; (( CURRENT-- )); _cece_agent ;;
        template) shift words; (( CURRENT-- )); _cece_template ;;
        idea|zed|code|cursor)
            if (( CURRENT == 3 )); then
                _cece_workspace_names
            fi
            ;;
        completions)
            if (( CURRENT == 3 )); then
                _values 'shell' bash zsh fish
            fi
            ;;
    esac
}

_cece "$@"
