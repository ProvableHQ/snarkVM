# coverage.sh — local SnarkVM coverage runner

This repository includes `coverage.sh`, a **local** helper for running `cargo llvm-cov nextest` coverage **for groups** (circuit, ledger, synthesizer, etc.) and generating per-group HTML/LCOV reports. It also supports a **merge mode** that combines previously-saved group coverage state into one combined report.

> This is intended for **local runs** (not CI). It mirrors the CI job “shape” but avoids `--workspace` all-at-once failures by running crate groups separately.
> It is a very heavy and time taking script (for some of the groups), that's why it is not OK to be ran on a CI. Requires a very powerful machine/VM for the heavier groups.

---

## Prerequisites

You need:

- Rust toolchain via `rustup`
- `cargo-nextest`
- `cargo-llvm-cov`
- LLVM coverage tools: `llvm-tools-preview` rustup component

The script checks for:
- `cargo`
- `cargo-nextest`
- `cargo-llvm-cov`

…and will install `llvm-tools-preview` automatically if missing.

---

## Quick start

From the repo root:

```bash
chmod +x ./coverage.sh
./coverage.sh
```

By default, it runs these groups:

- `circuit,console,ledger,synthesizer,misc`

and writes reports to:

- `./coverage/<group>/index.html` (HTML)
- `./coverage/<group>/lcov.info` (LCOV)

---

## Running specific groups

Use `COV_GROUPS` (comma-separated):

```bash
COV_GROUPS=ledger ./coverage.sh
COV_GROUPS=circuit,ledger,synthesizer ./coverage.sh
COV_GROUPS=ledger-slow ./coverage.sh
```

### Available groups

- `circuit`
- `console`
- `ledger`
- `ledger-slow` (rocks feature, partitions, ignored-only, ledger-block)
- `synthesizer`
- `synthesizer-slow` (partitions, integration, program shards, process rocks)
- `misc` (algorithms, curves, fields, parameters, utilities, derives)
- `snarkvm`

---

## Reports output directory

Reports go under `OUT_DIR`:

```bash
OUT_DIR=coverage ./coverage.sh
OUT_DIR=/tmp/snarkvm-coverage ./coverage.sh
```

Default:

- `OUT_DIR=coverage`

---

## Parallelism / resource knobs

### Nextest concurrency

```bash
NEXTEST_JOBS=8 ./coverage.sh
NEXTEST_JOBS=16 COV_GROUPS=synthesizer ./coverage.sh
```

Default:

- `NEXTEST_JOBS=8`

### Stack size

```bash
RUST_MIN_STACK=67108864 ./coverage.sh
```

Default:

- `RUST_MIN_STACK=67108864` (64 MiB)

---

## Global feature flags

If you want to apply the same cargo build args to *every* run, set `GLOBAL_FEATURES`.

Examples:

```bash
GLOBAL_FEATURES="--all-features" ./coverage.sh
GLOBAL_FEATURES="--features rocks" COV_GROUPS=ledger-slow ./coverage.sh
```

> Note: `GLOBAL_FEATURES` is split on whitespace. Provide it as you would type it on the command line.

---

## Caching behavior

To make it possible to run groups separately and later merge them, the script keeps a saved “coverage state” directory per group:

- Saved state: `coverage/_state/<group>/`
- Working state: `target/llvm-cov/<group>/`

### What happens on each group run

1. **Restore** previous state (if any): `coverage/_state/<group>` → `target/llvm-cov/<group>`
2. Run coverage for the group (writes into `target/llvm-cov/<group>`)
3. Generate the group report into `coverage/<group>/`
4. **Save** state back: `target/llvm-cov/<group>` → `coverage/_state/<group>`

### Cleaning state

To force a clean slate for a group run:

```bash
COV_CLEAN=1 COV_GROUPS=ledger ./coverage.sh
```

This removes both:
- `target/llvm-cov/<group>`
- `coverage/_state/<group>`

---

## Merge mode (combine groups)

Merge mode generates a combined report from previously-saved group states.

### Typical workflow

Run groups separately:

```bash
COV_GROUPS=circuit ./coverage.sh
COV_GROUPS=ledger  ./coverage.sh
```

Then merge them:

```bash
COV_MERGE=1 COV_MERGE_GROUPS=circuit,ledger ./coverage.sh
```

Outputs:

- `coverage/merged/index.html`
- `coverage/merged/lcov.info`

### Merge variables

- `COV_MERGE=1`
  Enables merge mode (the script will **only** merge and exit).

- `COV_MERGE_GROUPS=<comma-separated>`
  Which groups to merge. If omitted, it defaults to `COV_GROUPS`.

- `COV_MERGE_CLEAN=1` (default)
  Controls whether merge mode clears the merge working directory first.

---

## Interpreting failures

The script tries to continue through variants. If a variant fails, it is recorded in `FAILURES` and the script exits non-zero at the end while still producing whatever reports it could.

You will see a summary like:

- `ledger:default:snarkvm-ledger`

---

## Troubleshooting

### “Coverage report is empty” / merge only shows one group
This merge is **heuristic**: it relies on the contents of each group’s `LLVM_COV_TARGET_DIR` state being sufficient for a combined report.

If the merged report is incomplete, the robust approach is:

1. Force a stable `LLVM_PROFILE_FILE` output location per group (e.g. `coverage/profiles/<group>/*.profraw`)
2. Merge with `llvm-profdata merge` into a single `merged.profdata`
3. Generate HTML/LCOV from that merged profdata

If you hit this, inspect what got saved:

```bash
find coverage/_state/ledger -maxdepth 3 -type f | head -n 50
```

### Tooling not found
Install the required tools:

```bash
cargo install cargo-nextest --locked
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview
```

---

## Examples

### Fast per-group run
```bash
COV_GROUPS=circuit OUT_DIR=coverage ./coverage.sh
```

### Heavy group with more concurrency
```bash
COV_GROUPS=synthesizer-slow NEXTEST_JOBS=16 ./coverage.sh
```

### Clean + run + merge
```bash
COV_CLEAN=1 COV_GROUPS=circuit,ledger ./coverage.sh
COV_MERGE=1 COV_MERGE_GROUPS=circuit,ledger ./coverage.sh
```
