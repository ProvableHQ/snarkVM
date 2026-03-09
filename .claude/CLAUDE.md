@../AGENTS.md

# Workspace

Use `.claude/workspace/` for scratch files (gitignored).

```
.claude/workspace/
├── context-{type}-{id}.json    # Structured data (PR metadata, files list)
├── state-{type}-{id}.md        # Human-readable state + progress log
└── handoff-{type}-{id}.md      # Cross-skill communication
```

## Memory Rules

- **Summarize, don't retain** — extract findings to scratch files, discard raw data.
- **Fetch incrementally** — file list first, then per-file diffs as needed.
- **Rotate context** — write findings before switching to a new area.
- **Re-read scratch files** — use workspace state instead of scrolling back.
- **Index what you've done** — track progress in structured scratch files.

## Large PR Protocol

For PRs with 30+ files or 1000+ lines: fetch metadata + file list only, triage by risk (consensus > circuit > crypto), deep-dive selectively, write findings to workspace after each file, synthesize at end.

## Pagination

Always paginate: review threads (100+), comments (10+), commits (30+), PR comments (30+).
Use `--paginate` for REST, loop on `hasNextPage` for GraphQL.

## Session Recovery

1. Check workspace: `ls .claude/workspace/`
2. Read state files for progress — continue from last completed step.
3. Re-fetch if context is stale.


<claude-mem-context>
# Recent Activity

### Feb 6, 2026

| ID | Time | T | Title | Read |
|----|------|---|-------|------|
| #92 | 1:20 PM | 🔵 | settings.local.json uses granular permission system for allowing specific bash commands and skills | ~472 |
</claude-mem-context>