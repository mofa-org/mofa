# Obsidian Git Sync Agent

A MoFA-powered agent that keeps your [Obsidian](https://obsidian.md/) vault
synchronized across multiple devices using Git.

## How it Works

The agent exposes six Git-backed tools and wires them into a ReAct reasoning
loop. On each sync cycle it:

1. **Pulls** the latest changes from the remote (`git pull --rebase`)  
2. **Stages** all local changes (`git add -A`)  
3. **Commits** with an auto-generated timestamp message  
4. **Pushes** to the remote (`git push`)

Two operation modes are available:

| Mode | Description |
|------|-------------|
| **once** (default) | Run a single sync cycle and exit |
| **auto** | Run the sync cycle periodically (default: every 5 min) |
| **direct** | Call Git tools directly without the LLM — no API key required |

```
┌──────────────────────────────────────────────────────────┐
│              ObsidianSyncAgent                           │
│                                                          │
│  ┌─────────────┐   ┌─────────────┐   ┌──────────────┐   │
│  │  GitStatus  │   │  GitPull    │   │  GitStage    │   │
│  │   Tool      │   │   Tool      │   │   Tool       │   │
│  └─────────────┘   └─────────────┘   └──────────────┘   │
│                                                          │
│  ┌─────────────┐   ┌─────────────┐   ┌──────────────┐   │
│  │  GitCommit  │   │  GitPush    │   │  GitSync     │   │
│  │   Tool      │   │   Tool      │   │   Tool       │   │
│  └─────────────┘   └─────────────┘   └──────────────┘   │
└──────────────────────────────────────────────────────────┘
```

## Prerequisites

### 1 — Turn your vault into a Git repository

```bash
cd /path/to/your/obsidian/vault

git init
git remote add origin git@github.com:yourname/vault.git   # or HTTPS

git add .
git commit -m "initial commit"
git push -u origin main
```

> **Tip — mobile devices**: on iOS/Android use [Working Copy](https://workingcopyapp.com/)
> or [MGit](https://github.com/maks/MGit) to clone the same remote, then
> point Obsidian at the cloned folder.

### 2 — Configure SSH keys or HTTPS credentials

The sync agent calls `git push` non-interactively, so authentication must be
stored in advance:

* **SSH** – add your public key to GitHub/GitLab and configure `~/.ssh/config`.
* **HTTPS** – use a [credential helper](https://git-scm.com/docs/gitcredentials)
  or a personal-access token via the remote URL
  (`https://TOKEN@github.com/yourname/vault.git`).

### 3 — (LLM mode only) Set your OpenAI API key

```bash
export OPENAI_API_KEY=sk-...
```

Or point the agent at a local model:

```bash
export OPENAI_BASE_URL=http://localhost:11434/v1
export OPENAI_MODEL=llama3.2
```

## Running

```bash
# Single sync (LLM mode)
cargo run -p obsidian_git_sync -- --vault /path/to/vault

# Auto-sync every 5 minutes (LLM mode)
cargo run -p obsidian_git_sync -- --vault /path/to/vault --auto --interval 300

# Single sync without LLM (no API key needed)
cargo run -p obsidian_git_sync -- --vault /path/to/vault --direct

# Show help
cargo run -p obsidian_git_sync -- --help
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENAI_API_KEY` | *(required for LLM mode)* | OpenAI API key |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | API endpoint (Ollama, LM Studio, etc.) |
| `OPENAI_MODEL` | `gpt-4o-mini` | Model to use for reasoning |

## Typical Workflow

```
Device A (desktop)         Remote (GitHub)        Device B (mobile)
─────────────────          ───────────────         ─────────────────
Write notes in Obsidian
       │
       ▼
obsidian_git_sync          ──── git push ──▶        git pull
(auto every 5 min)                                  (Working Copy / MGit)
```

## Available Git Tools

| Tool | Git command | Description |
|------|-------------|-------------|
| `git_status` | `git status --short` | Show uncommitted changes |
| `git_pull` | `git pull --rebase` | Fetch & rebase from remote |
| `git_stage` | `git add -A` | Stage all changes |
| `git_commit` | `git commit -m <msg>` | Commit staged changes |
| `git_push` | `git push` | Push to remote |
| `git_sync` | all of the above | Full sync cycle in one step |

## Handling Conflicts

If `git pull --rebase` fails due to a conflict, the sync agent will report the
error and stop. To resolve:

```bash
cd /path/to/vault
git status           # see conflicting files
# edit the conflicting notes manually
git add .
git rebase --continue
```

After resolving, re-run the sync agent.

## Running Tests

```bash
cargo test -p obsidian_git_sync
```

The tests create temporary Git repositories and exercise each tool without
requiring a remote or an API key.
