# snarkVM Claude Skills

Drop-in Claude Code skills for snarkVM development workflows.

## Skills Overview

| Skill | Purpose | Invoke |
|-------|---------|--------|
| **snarkvm-github** | Fetch PR/issue context from GitHub | `/snarkvm-github pr 123` |
| **snarkvm-fix** | Fix GitHub issues or PR feedback with TDD | `/snarkvm-fix issue 456` or `/snarkvm-fix pr 123` |
| **snarkvm-review** | Security-focused PR review | `/snarkvm-review 123` |

## Prerequisites

1. **GitHub CLI** authenticated: `gh auth status || gh auth login`
2. **Rust toolchain** with nightly for formatting: `rustup install nightly`

## Usage

```
/snarkvm-review 3129        # Review a PR
/snarkvm-fix pr 3129        # Fix PR review feedback
/snarkvm-fix issue 2990     # Fix a GitHub issue
/snarkvm-github pr 3129     # Just fetch context
```

Skills auto-fetch context when needed. Use `--force` to refresh stale data.

## Workspace Structure

All context is stored in `.claude/workspace/`:

```
.claude/workspace/
├── context-pr-123.json      # PR metadata
├── files-pr-123.txt         # Changed files
├── commits-pr-123.json      # Commits
├── comments-pr-123.json     # PR comments
├── checks-pr-123.json       # CI status
├── threads-pr-123.jsonl     # Review threads
├── unresolved-pr-123.json   # Unresolved comments
├── resolved-pr-123.json     # Resolved comments
├── state-pr-123.md          # Working state (findings, log)
├── handoff-pr-123.md        # Review→fix handoff
│
├── context-issue-456.json   # Issue metadata
├── comments-issue-456.jsonl # Issue comments
├── timeline-issue-456.json  # Cross-references
└── state-issue-456.md       # Investigation state
```

## Skill Structure

```
snarkvm-github/
├── SKILL.md               # Fetch context workflow
├── scripts/
│   ├── fetch-pr.sh         # Fetch PR context (paginated GitHub API + GraphQL)
│   ├── fetch-issue.sh      # Fetch issue context
│   └── refresh-threads.sh  # Quick thread refresh
└── templates/
    ├── state-pr.md         # PR state template
    ├── state-issue.md      # Issue state template
    └── handoff.md          # Review handoff template
```

## Troubleshooting

- **"gh CLI not authenticated"**: Run `gh auth login`
- **"Context missing"**: Skills auto-fetch, or manually: `/snarkvm-github pr 123`
- **Skills not appearing**: Run `/context` and check for warnings about excluded skills

See AGENTS.md for snarkVM-specific domain knowledge (crates, testing, validation, review checklists).
