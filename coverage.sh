#!/usr/bin/env bash
set -euo pipefail

# From the CI definitions:
RUST_MIN_STACK="${RUST_MIN_STACK:-67108864}"
JOBS="${NEXTEST_JOBS:-8}"
OUT_DIR="${OUT_DIR:-coverage}"

# Optional build args applied to every run (array)
# For very custom runs. Added so if somethig is new or skipped by new code a new run/pseudo-group can be created,
# through the CLI. Can be ignored for normal runs.
#
# Examples:
#   GLOBAL_BUILD_ARGS=(--all-features)
#   GLOBAL_BUILD_ARGS=(--features rocksdb,test)
GLOBAL_BUILD_ARGS=()

if [ -n "${GLOBAL_FEATURES:-}" ]; then
  GLOBAL_BUILD_ARGS=(${GLOBAL_FEATURES})
fi

FAILURES=()

# Check that everything needed is installed:
need_cmd() { command -v "$1" >/dev/null 2>&1 || { echo "Missing: $1" >&2; exit 1; }; }
need_cmd cargo
need_cmd cargo-nextest
need_cmd cargo-llvm-cov

if ! rustup component list --installed | grep -q '^llvm-tools-preview'; then
  echo "Installing rustup component llvm-tools-preview..."
  rustup component add llvm-tools-preview
fi

mkdir -p "$OUT_DIR" "coverage/_state"

# Decided to move the state under coverage (not target) as target is sometimes deleted, so these functions copy it
# and restore it.
restore_cov_state() {
  local group="$1"
  local src="coverage/_state/${group}"
  local dst="coverage/_merge_state/${group}"

  if [ -d "$src" ]; then
    echo "==> Restoring cached cov state: $src -> $dst"
    mkdir -p "$(dirname "$dst")"
    rm -rf "$dst"
    cp -a "$src" "$dst"
  fi
}

save_cov_state() {
  local group="$1"
  local src="target/llvm-cov/${group}"
  local dst="coverage/_state/${group}"

  echo "==> Saving cov state: $src -> $dst"
  mkdir -p "$(dirname "$dst")"
  rm -rf "$dst"
  if [ -d "$src" ]; then
    cp -a "$src" "$dst"
  else
    echo "WARN: no cov state dir found at $src" >&2
  fi
}

maybe_clean_cov_state() {
  local group="$1"
  if [ "${COV_CLEAN:-0}" = "1" ]; then
    echo "==> Cleaning cov state for group '$group'"
    rm -rf "target/llvm-cov/${group}" "coverage/_state/${group}"
  fi
}

# Usage:
#   run_cov_variant <group> <pkg> <variant> [build-args...] [-- libtest-args...]
#
# Examples:
#   run_cov_variant ledger snarkvm-ledger default
#   run_cov_variant ledger-slow snarkvm-ledger with-rocks --features rocks
#   run_cov_variant ledger-slow snarkvm-ledger-store -- --ignored --test-threads 2
run_cov_variant() {
  local group="$1"; shift
  local pkg="$1"; shift
  local variant="$1"; shift

  local cov_target_dir="target/llvm-cov/${group}"
  mkdir -p "$cov_target_dir"

  # Split args around optional "--" Similar to the run code in the CI:
  local build_args=()
  local test_args=()
  local seen_sep=0
  while (($#)); do
    if [ "$1" = "--" ]; then
      seen_sep=1
      shift
      continue
    fi
    if [ $seen_sep -eq 0 ]; then
      build_args+=("$1")
    else
      test_args+=("$1")
    fi
    shift
  done

  echo ""
  echo "=== [${group}] ${variant} ==="
  echo "pkg: ${pkg}"
  echo "cov_target_dir: ${cov_target_dir}"
  if ((${#build_args[@]})); then
    echo "build_args: ${build_args[*]}"
  else
    echo "build_args: <none>"
  fi
  if ((${#test_args[@]})); then
    echo "libtest_args: ${test_args[*]}"
  else
    echo "libtest_args: <none>"
  fi
  echo ""

  local cmd=(
    cargo llvm-cov nextest
    --no-report
    -p "$pkg"
    --no-fail-fast
    -j "$JOBS"
  )

  local -a global_build_args=()

  if [ -n "${GLOBAL_FEATURES:-}" ]; then
    global_build_args=(${GLOBAL_FEATURES})
  fi

  if ((${#global_build_args[@]})); then
    cmd+=("${global_build_args[@]}")
  fi

  # Only add "--" if we actually have libtest args
  if ((${#test_args[@]})); then
    cmd+=(-- "${test_args[@]}")
  fi

  if ! LLVM_COV_TARGET_DIR="$cov_target_dir" \
      RUST_MIN_STACK="$RUST_MIN_STACK" \
      "${cmd[@]}"
  then
    FAILURES+=("${group}:${variant}:${pkg}")
  fi
}

finalize_group_report() {
  local group="$1"
  local cov_target_dir="target/llvm-cov/${group}"
  local group_out="${OUT_DIR}/${group}"

  mkdir -p "$group_out"

  echo ""
  echo "=== [${group}] generating report ==="
  echo "raw data: ${cov_target_dir}"
  echo "html: ${group_out}/index.html"
  echo "lcov: ${group_out}/lcov.info"
  echo ""

  LLVM_COV_TARGET_DIR="$cov_target_dir" \
    cargo llvm-cov report --html --output-dir "$group_out"

  LLVM_COV_TARGET_DIR="$cov_target_dir" \
    cargo llvm-cov report --lcov --output-path "${group_out}/lcov.info"
}

# ===== Groups (Like CI workflows) =====

run_circuit_group() {
  local group="circuit"
  maybe_clean_cov_state "$group"
  restore_cov_state "$group"

  local pkgs=(
    snarkvm-circuit
    snarkvm-circuit-account
    snarkvm-circuit-algorithms
    snarkvm-circuit-collections
    snarkvm-circuit-environment
    snarkvm-circuit-network
    snarkvm-circuit-program
    snarkvm-circuit-types
    snarkvm-circuit-types-address
    snarkvm-circuit-types-boolean
    snarkvm-circuit-types-field
    snarkvm-circuit-types-group
    snarkvm-circuit-types-integers
    snarkvm-circuit-types-scalar
    snarkvm-circuit-types-string
  )

  for p in "${pkgs[@]}"; do
    run_cov_variant "$group" "$p" "default"
  done

  finalize_group_report "$group"
  save_cov_state "$group"
}

run_console_group() {
  local group="console"
  maybe_clean_cov_state "$group"
  restore_cov_state "$group"

  local pkgs=(
    snarkvm-console
    snarkvm-console-account
    snarkvm-console-algorithms
    snarkvm-console-collections
    snarkvm-console-network
    snarkvm-console-network-environment
    snarkvm-console-program
    snarkvm-console-types
    snarkvm-console-types-address
    snarkvm-console-types-boolean
    snarkvm-console-types-field
    snarkvm-console-types-group
    snarkvm-console-types-integers
    snarkvm-console-types-scalar
    snarkvm-console-types-string
  )

  for p in "${pkgs[@]}"; do
    run_cov_variant "$group" "$p" "default"
  done

  finalize_group_report "$group"
  save_cov_state "$group"
}

# Fast ledger: matches ledger-workflow (excluding heavy merge/release extras)
run_ledger_group() {
  local group="ledger"
  maybe_clean_cov_state "$group"
  restore_cov_state "$group"

  local pkgs=(
    snarkvm-ledger
    snarkvm-ledger-authority
    snarkvm-ledger-committee
    snarkvm-ledger-narwhal
    snarkvm-ledger-narwhal-batch-certificate
    snarkvm-ledger-narwhal-batch-header
    snarkvm-ledger-narwhal-data
    snarkvm-ledger-narwhal-subdag
    snarkvm-ledger-narwhal-transmission
    snarkvm-ledger-narwhal-transmission-id
    snarkvm-ledger-puzzle
    snarkvm-ledger-puzzle-epoch
    snarkvm-ledger-query
    snarkvm-ledger-store
    snarkvm-ledger-test-helpers
  )

  for p in "${pkgs[@]}"; do
    run_cov_variant "$group" "$p" "default"
  done

  finalize_group_report "$group"
  save_cov_state "$group"
}

# Slow ledger: rocks partitions, ignored-only, ledger-block (merge-workflow + release-workflow)
run_ledger_slow_group() {
  local group="ledger-slow"
  maybe_clean_cov_state "$group"
  restore_cov_state "$group"

  # Heavy: ledger-block
  run_cov_variant "$group" "snarkvm-ledger-block" "ledger-block"

  # Heavy-ish: rocks feature
  run_cov_variant "$group" "snarkvm-ledger" "rocks" --release --features rocks
  run_cov_variant "$group" "snarkvm-ledger-store" "store-rocks" --features rocks

  # Very heavy: ignored-only
  run_cov_variant "$group" "snarkvm-ledger-store" "store-ignored" \
    -- --run-ignored ignored-only --test-threads 2

  run_cov_variant "$group" "snarkvm-ledger" "rocks-partition-1" \
    --release --features rocks --partition count:1/2 -- --test-threads 10
  run_cov_variant "$group" "snarkvm-ledger" "rocks-partition-2" \
    --release --features rocks --partition count:2/2 -- --test-threads 10

  finalize_group_report "$group"
  save_cov_state "$group"
}

# Fast synthesizer: synthesizer-workflow
run_synthesizer_group() {
  local group="synthesizer"
  maybe_clean_cov_state "$group"
  restore_cov_state "$group"

  local pkgs=(
    snarkvm-synthesizer
    snarkvm-synthesizer-process
    snarkvm-synthesizer-program
    snarkvm-synthesizer-snark
  )

  for p in "${pkgs[@]}"; do
    run_cov_variant "$group" "$p" "default"
  done

  run_cov_variant "$group" "snarkvm-synthesizer" "lib-bins" --lib --bins -- --test-threads 16

  finalize_group_report "$group"
  save_cov_state "$group"
}

# Slow synthesizer: partitions, integration, program integration shards, process rocks
run_synthesizer_slow_group() {
  local group="synthesizer-slow"
  maybe_clean_cov_state "$group"
  restore_cov_state "$group"

  run_cov_variant "$group" "snarkvm-synthesizer" "ignored-only" \
    --features test \
    -- --run-ignored ignored-only --test-threads 2

  run_cov_variant "$group" "snarkvm-synthesizer" "test-partition-1" \
    --lib --bins --features test --partition count:1/2 -- --test-threads 8
  run_cov_variant "$group" "snarkvm-synthesizer" "test-partition-2" \
    --lib --bins --features test --partition count:2/2 -- --test-threads 8

  run_cov_variant "$group" "snarkvm-synthesizer" "integration" \
    --test '*' --features test -- --test-threads 4

  run_cov_variant "$group" "snarkvm-synthesizer-process" "with-rocksdb" \
    --features rocks -- --test-threads 4

  run_cov_variant "$group" "snarkvm-synthesizer-program" "integration" \
    -- --skip keccak --skip psd --skip sha --skip instruction::is --skip instruction::equal --skip instruction::commit --test-threads 8

  run_cov_variant "$group" "snarkvm-synthesizer-program" "integration-keccak" keccak
  run_cov_variant "$group" "snarkvm-synthesizer-program" "integration-psd" psd
  run_cov_variant "$group" "snarkvm-synthesizer-program" "integration-sha" sha
  run_cov_variant "$group" "snarkvm-synthesizer-program" "integration-instruction-is" instruction::is
  run_cov_variant "$group" "snarkvm-synthesizer-program" "integration-instruction-equal" instruction::equal
  run_cov_variant "$group" "snarkvm-synthesizer-program" "integration-instruction-commit" instruction::commit

  finalize_group_report "$group"
  save_cov_state "$group"
}

run_misc_group() {
  local group="misc"
  maybe_clean_cov_state "$group"
  restore_cov_state "$group"

  local pkgs=(
    snarkvm-algorithms
    snarkvm-curves
    snarkvm-fields
    snarkvm-parameters
    snarkvm-utilities
    snarkvm-utilities-derives
  )

  for p in "${pkgs[@]}"; do
    run_cov_variant "$group" "$p" "default"
  done

  finalize_group_report "$group"
  save_cov_state "$group"
}

run_snarkvm_group() {
  local group="snarkvm"
  maybe_clean_cov_state "$group"
  restore_cov_state "$group"

  run_cov_variant "$group" "snarkvm" "default"

  finalize_group_report "$group"
  save_cov_state "$group"
}

merge_restore_group_state() {
  local group="$1"
  local src="coverage/_state/${group}"
  local dst="target/llvm-cov/_merge/${group}"

  if [ ! -d "$src" ]; then
    echo "WARN: no saved cov state for group '$group' at $src (skipping)" >&2
    return 1
  fi

  echo "==> Restoring group cov state for merge: $src -> $dst"
  mkdir -p "$(dirname "$dst")"
  rm -rf "$dst"
  cp -a "$src" "$dst"
}

merge_generate_report() {
  local merge_groups_raw="$1"
  local merge_out="${OUT_DIR}/merged"
  local merge_root="coverage/_merge_state"

  mkdir -p "$merge_out"

  echo ""
  echo "=== [merge] generating combined report ==="
  echo "groups: ${merge_groups_raw}"
  echo "merge_root: ${merge_root}"
  echo "html: ${merge_out}/index.html"
  echo "lcov: ${merge_out}/lcov.info"
  echo ""

  LLVM_COV_TARGET_DIR="$merge_root" \
    cargo llvm-cov report --html --output-dir "$merge_out"

  LLVM_COV_TARGET_DIR="$merge_root" \
    cargo llvm-cov report --lcov --output-path "${merge_out}/lcov.info"
}

run_merge() {
  local merge_groups_raw="${COV_MERGE_GROUPS:-${COV_GROUPS:-}}"
  if [ -z "$merge_groups_raw" ]; then
    echo "ERROR: merge requested but no groups specified (set COV_MERGE_GROUPS or COV_GROUPS)" >&2
    exit 2
  fi

  echo "[debug] MERGE_GROUPS='${merge_groups_raw}'"

  # Clean merge root unless disabled
  if [ "${COV_MERGE_CLEAN:-1}" = "1" ]; then
    rm -rf "coverage/_merge_state"
  fi
  mkdir -p "coverage/_merge_state"

  local IFS=','
  read -r -a mg <<< "${merge_groups_raw}"

  local restored_any=0
  for g in "${mg[@]}"; do
    g="$(printf '%s' "$g" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
    [ -z "$g" ] && continue
    if merge_restore_group_state "$g"; then
      restored_any=1
    fi
  done

  if [ "$restored_any" -eq 0 ]; then
    echo "ERROR: no groups could be restored for merge; did you run coverage for them first?" >&2
    exit 2
  fi

  merge_generate_report "$merge_groups_raw"
}

main() {
  if [ "${COV_MERGE:-0}" = "1" ]; then
    run_merge
    return 0
  fi

  local groups_raw="${COV_GROUPS:-circuit,console,ledger,synthesizer,misc}"
  echo "[debug] GROUPS='${groups_raw}'"

  local IFS=','
  read -r -a gs <<< "${groups_raw}"

  for g in "${gs[@]}"; do
    g="$(printf '%s' "$g" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
    [ -z "$g" ] && continue

    case "$g" in
      circuit)           run_circuit_group ;;
      console)           run_console_group ;;
      ledger)            run_ledger_group ;;
      ledger-slow)       run_ledger_slow_group ;;
      synthesizer)       run_synthesizer_group ;;
      synthesizer-slow)  run_synthesizer_slow_group ;;
      misc)              run_misc_group ;;
      snarkvm)           run_snarkvm_group ;;
      *)
        echo "WARN: ignoring unknown group token: '$g'" >&2
        ;;
    esac
  done

  echo ""
  echo "==== Coverage complete ===="
  echo "Reports in: ${OUT_DIR}/"
  echo ""

  if ((${#FAILURES[@]} > 0)); then
    echo "Some variants failed (coverage still generated from successful runs):"
    printf ' - %s\n' "${FAILURES[@]}"
    exit 1
  fi
}

main "$@"
