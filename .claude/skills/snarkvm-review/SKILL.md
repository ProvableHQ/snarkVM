---
name: snarkvm-review
description: |
  Security-focused PR review for snarkVM codebase.
  WHEN: User says "review PR", "audit PR", "security review", "check PR changes",
  or wants thorough analysis of PR changes for bugs/vulnerabilities.
  WHEN NOT: Fixing review feedback (use snarkvm-fix pr), fetching context only
  (use snarkvm-github), or fixing issues (use snarkvm-fix).
context: fork
agent: general-purpose
model: opus
allowed-tools: Bash, Read, Write, Grep, Glob, Task
disable-model-invocation: true
---

# Security-Focused PR Review

**Mindset: Assume there is a bug. Find it. Ultrathink.**

This skill runs in a forked context to keep exploration out of your main conversation.

## Usage

```
/snarkvm-review <pr_number>
```

## Setup

```bash
PR=$ARGUMENTS
WS=".claude/workspace"
SKILL_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../snarkvm-github" && pwd)"
```

## 1. Load Context

```bash
if [[ ! -f "$WS/state-pr-$PR.md" ]]; then
  "$SKILL_DIR/scripts/fetch-pr.sh" "$PR"
fi
```

Read: `$WS/state-pr-$PR.md`, `$WS/files-pr-$PR.txt`

## 2. Triage by Risk

Categorize changed files by risk:
- **CRITICAL**: algorithms/ (crypto primitives), circuit/ (constraint generation), synthesizer/src/vm/ (consensus logic)
- **HIGH**: console/ (types, serialization), synthesizer/process/ (execution), ledger/ (state management)
- **MEDIUM**: synthesizer/program/ (instructions, logic), fields/, curves/
- **LOW**: utilities/, test files, docs, CI config

Update `$WS/state-pr-$PR.md` with risk assessment.

**For large PRs (30+ files):** Focus CRITICAL/HIGH risk areas first. Use parallel subagents by category (crypto/circuit, synthesizer, other).

## 3. Understand Intent

Before diving into code:
1. Read PR description: `jq -r .body "$WS/context-pr-$PR.json"`
2. Check linked issues: `cat "$WS/linked-issues-pr-$PR.txt"`
3. What consensus invariants must hold?
4. What could go wrong?

## 4. Analyze Code

**Small/medium PRs (< 30 files):** Sequential. For each CRITICAL/HIGH risk file — read full file, trace logic, check boundaries, write findings to state file, release from memory.

**Large PRs (30+ CRITICAL/HIGH files):** Spawn parallel Task subagents by category (crypto/circuit, synthesizer, other). Each reads assigned files, applies AGENTS.md checklists, returns findings table.

Apply **AGENTS.md Review Checklist** (Correctness, Crypto-Specific, Memory & Performance, Security) to each file.

## 5. Validate

Validate affected crates per AGENTS.md.

## 6. Report

Update `$WS/state-pr-$PR.md` with findings:

| Sev | Location | Issue | Suggested Fix |
|-----|----------|-------|---------------|

**Severity:** BLOCKER (must fix), BUG (should fix), ISSUE (consider), NIT (optional).

**Recommendation:** Approve, Request changes, or Needs discussion.

## 7. Handoff

If requesting changes, create handoff for `/snarkvm-fix pr`:

```bash
sed -e "s/{{NUM}}/$PR/g" "$SKILL_DIR/templates/handoff.md" > "$WS/handoff-pr-$PR.md"
```

## Common Vulnerability Patterns in snarkVM

- **Consensus divergence**: Changes that produce different outputs for same inputs across versions
- **Circuit/console mismatch**: Circuit constraints don't match console logic, producing invalid proofs
- **Serialization breaking**: Format changes that break backwards compatibility
- **Field overflow**: Arithmetic on field elements without proper bounds checking
- **Randomness misuse**: Non-cryptographic or predictable randomness in proof generation
- **Unchecked deserialization**: Accepting malformed data that violates type invariants
