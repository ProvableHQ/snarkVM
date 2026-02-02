---
description: Fix a GitHub issue with TDD
allowed-tools: Bash, Read, Write, Grep, Glob, Task
---

# Fix: $ARGUMENTS

Follow CLAUDE.md for validation commands and code patterns.

## 1. Understand
- [ ] `gh issue view $ARGUMENTS --json title,body,comments`
- [ ] Identify affected crate(s)
- [ ] Locate relevant code
- [ ] Articulate root cause and proposed fix

Do not proceed until you can explain the root cause. Think very hard.

## 2. Baseline
- [ ] Run validation on affected crate (see CLAUDE.md)
- [ ] All checks pass before changes

## 3. Test First
- [ ] Write failing test that reproduces the bug
- [ ] Confirm test fails: `cargo test -p <crate> <test_name>`

## 4. Fix
- [ ] Smallest change that makes test pass
- [ ] No refactoring, no extras

## 5. Verify
- [ ] `cargo check -p <crate>`
- [ ] `cargo clippy -p <crate> -- -D warnings`
- [ ] `cargo fmt --check`
- [ ] `cargo test -p <crate>`

All must pass. If not, fix and repeat.

## 6. Report
- Root cause
- What changed
- Test added
- All checks pass

Do not commit.
