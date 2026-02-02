---
description: Review a PR - find the bug
allowed-tools: Bash, Read, Write, Grep, Glob, Task
---

# Review: $ARGUMENTS

**Assume there is a bug. Your job is to find it. Do not approve until you have proven otherwise. Think very hard.**

## 1. Gather
- [ ] `gh pr view --json title,body,state,isDraft,mergeable,statusCheckRollup,url,author,assignees,reviewRequests,reviews,labels,milestone,createdAt,additions,deletions,changedFiles,headRefName,baseRefName,commits`
- [ ] `gh pr diff $ARGUMENTS`
- [ ] Read full files (not just diff) for all modified code
- [ ] Read files that import/are imported by modified files
- [ ] Identify affected crates: `gh pr diff $ARGUMENTS --name-only`

## 2. Understand
- [ ] What problem is this solving?
- [ ] What is the approach?
- [ ] Write down your understanding before proceeding

## 3. Analyze
For each change, check:

**Correctness**
- [ ] Logic traced step-by-step
- [ ] Boundary conditions: zero, empty, max
- [ ] Error handling correct
- [ ] No panics possible
- [ ] No race conditions

**Crypto (if applicable)**
- [ ] Field operations safe
- [ ] Checked arithmetic used
- [ ] Randomness appropriate

**Memory**
- [ ] Pre-allocation where needed
- [ ] No unnecessary `.collect()`
- [ ] `into_iter()` over `iter().cloned()`

## 4. Verify
- [ ] Run tests: `cargo test -p <crate>`
- [ ] For each non-trivial change: write a test that would catch a bug, run it
- [ ] If you cannot prove correctness, it's a bug

## 5. Report

**Format:**
```
[SEVERITY] file:line — Issue. Suggested fix.
```

**Severities:** BLOCKER > BUG > ISSUE > NIT > QUESTION

**Summary table:**
| Severity | Location | Issue |
|----------|----------|-------|

**Recommendation:**
- **Approve** — You proved no bugs exist
- **Request changes** — You found bugs or cannot prove correctness
- **Reject** — Fundamental flaws

Do not commit. Leave repo unchanged.
