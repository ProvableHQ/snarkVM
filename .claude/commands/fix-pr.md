---
description: Address open PR comments
allowed-tools: Bash, Read, Write, Grep, Glob, Task
---

# Fix PR: $ARGUMENTS

Follow CLAUDE.md for validation commands and code patterns.

## 1. Gather
- [ ] `gh pr view --json title,body,state,isDraft,mergeable,statusCheckRollup,url,author,assignees,reviewRequests,reviews,labels,milestone,createdAt,additions,deletions,changedFiles,headRefName,baseRefName,commits`
- [ ] `gh pr diff $ARGUMENTS`
- [ ] List all unresolved comments
- [ ] Read full files for context

## 2. Analyze
For each open comment:
- [ ] What is the reviewer asking?
- [ ] Is it valid?
- [ ] What's the fix? Think very hard.

## 3. Propose
Present before implementing:

| # | File:Line | Comment | Proposed Fix |
|---|-----------|---------|--------------|
| 1 | ... | ... | ... |

Ask: "Approve / Modify / Skip for each?"

Wait for approval.

## 4. Baseline
- [ ] Run validation on affected crate(s)
- [ ] All checks pass before changes

## 5. Implement
For each approved fix:
- [ ] Make change
- [ ] `cargo check -p <crate>`
- [ ] `cargo clippy -p <crate> -- -D warnings`
- [ ] Next fix only when green

## 6. Verify
- [ ] `cargo fmt --check`
- [ ] `cargo test -p <crate>`
- [ ] All checks pass

## 7. Report
| # | Comment | Status |
|---|---------|--------|
| 1 | ... | Done |

Do not commit.
