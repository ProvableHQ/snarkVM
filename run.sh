#!/bin/bash

set -e

BASE_CMD="cargo test test_transfer_private_execution --package snarkvm-synthesizer --lib --release"
SUFFIX="-- --nocapture"

# The AMM functions benchmarked by the `amm_swap` bench, in order.
AMM_FUNCTIONS=(
    swap
    swap_private
    claim_swap_output
    claim_swap_output_private
    swap_multi_hop
    swap_multi_hop_private
    claim_multi_hop_output
    claim_multi_hop_output_private
)

# Builds the `amm_swap` bench binary once, then runs it once per AMM function.
#
# Each function is benchmarked in a fresh process because cuVaruna keeps GPU memory state
# across proofs that is only safe for a single circuit shape; running heterogeneous AMM
# circuits in one process leads to a SIGSEGV. A process per function also keeps the (verified)
# on-chain setup on a clean GPU before any heavy measurement proving.
#
# We compile the binary a single time and invoke it directly (rather than calling `cargo bench`
# per function) so cargo does not re-evaluate the build graph and re-emit cached build-script
# warnings on every iteration.
run_amm_bench() {
    local feature="$1"

    echo "Building amm_swap bench binary (--features $feature)..."
    cargo bench --package snarkvm-synthesizer --bench amm_swap --features "$feature" --no-run

    # Find the most recently built bench binary.
    local binary
    binary=$(find ./target/release/deps -name 'amm_swap-*' -type f -executable ! -name '*.d' -printf '%T@ %p\n' \
        | sort -rn | head -1 | cut -d' ' -f2-)
    if [ -z "$binary" ]; then
        echo "Error: Could not find amm_swap bench binary"
        exit 1
    fi
    echo "Using bench binary: $binary"

    for f in "${AMM_FUNCTIONS[@]}"; do
        echo "=== Benchmarking amm::$f ($feature) ==="
        "$binary" --bench --exact "amm::$f"
    done
}

case "$1" in
    cpu)
        echo "Running CPU only (no CUDA)..."
        $BASE_CMD $SUFFIX
        ;;
    base)
        echo "Running baseline (cuda only)..."
        $BASE_CMD --features cuda $SUFFIX
        ;;
    cu)
        FEATURES="--features cuvaruna"
        shift
        for arg in "$@"; do
            case "$arg" in
                debug)
                    FEATURES="$FEATURES --features cuvaruna-debug"
                    ;;
                profile)
                    FEATURES="$FEATURES --features cuvaruna-profiling"
                    ;;
                *)
                    echo "Unknown option: $arg"
                    echo "Usage: ./run.sh cu [debug] [profile]"
                    exit 1
                    ;;
            esac
        done
        echo "Running cuVaruna with: $FEATURES"
        $BASE_CMD $FEATURES $SUFFIX
        ;;
    nsys)
        OUTPUT_NAME="${2:-snarkvm_test_profile}"
        OUTPUT_FILE="${OUTPUT_NAME}.nsys-rep"
        echo "Building with cuvaruna + profiling..."
        cargo test test_transfer_private_execution --package snarkvm-synthesizer --lib --release \
            --features cuvaruna --features cuvaruna-profiling --no-run

        # Find the test binary
        BINARY=$(find ./target/release/deps -name 'snarkvm_synthesizer-*' -type f -executable ! -name '*.d' | head -1)
        if [ -z "$BINARY" ]; then
            echo "Error: Could not find test binary"
            exit 1
        fi
        echo "Found binary: $BINARY"

        # if output file already exists, delete it
        if [ -f "$OUTPUT_FILE" ]; then
            echo "Output file already exists, deleting it..."
            rm "$OUTPUT_FILE"
        fi

        echo "Running nsys profile (output: ${OUTPUT_FILE})..."
        nsys profile --trace=cuda,nvtx,osrt --sample=cpu -o "$OUTPUT_NAME" \
            "$BINARY" test_transfer_private_execution --nocapture
        ;;
    nsys-amm)
        # Profile a single AMM function under nsys, using the amm_swap bench binary built
        # with cuvaruna + cuvaruna-profiling. We reuse the bench binary (no new binary needed)
        # and select one function via `--exact`. Criterion's `--profile-time` runs that function
        # in a plain loop for the given number of seconds (no warmup/statistics), which keeps the
        # nsys capture focused on the proof itself.
        FUNC="${2:?Usage: ./run.sh nsys-amm <function> [profile_seconds] [output_name]}"
        PROFILE_SECS="${3:-20}"
        OUTPUT_NAME="${4:-amm_${FUNC}_profile}"
        OUTPUT_FILE="${OUTPUT_NAME}.nsys-rep"

        # Validate the requested function.
        if [[ ! " ${AMM_FUNCTIONS[*]} " == *" ${FUNC} "* ]]; then
            echo "Error: unknown AMM function '${FUNC}'. Valid functions:"
            printf '  %s\n' "${AMM_FUNCTIONS[@]}"
            exit 1
        fi

        echo "Building amm_swap bench with cuvaruna + profiling..."
        cargo bench --package snarkvm-synthesizer --bench amm_swap \
            --features cuvaruna --features cuvaruna-profiling --no-run

        # Find the most recently built bench binary.
        BINARY=$(find ./target/release/deps -name 'amm_swap-*' -type f -executable ! -name '*.d' -printf '%T@ %p\n' \
            | sort -rn | head -1 | cut -d' ' -f2-)
        if [ -z "$BINARY" ]; then
            echo "Error: Could not find amm_swap bench binary"
            exit 1
        fi
        echo "Found binary: $BINARY"

        # If output file already exists, delete it.
        if [ -f "$OUTPUT_FILE" ]; then
            echo "Output file already exists, deleting it..."
            rm "$OUTPUT_FILE"
        fi

        echo "Running nsys profile for amm::${FUNC} (output: ${OUTPUT_FILE}, profile-time: ${PROFILE_SECS}s)..."
        nsys profile --trace=cuda,nvtx,osrt --sample=cpu -o "$OUTPUT_NAME" \
            "$BINARY" --bench --exact "amm::$FUNC" --profile-time "$PROFILE_SECS"
        ;;
    bench-base)
        echo "Running benchmark with cuda (baseline)..."
        cargo bench --package snarkvm-synthesizer --bench execute_authorization --features cuda
        ;;
    bench-cu)
        echo "Running benchmark with cuvaruna..."
        cargo bench --package snarkvm-synthesizer --bench execute_authorization --features cuvaruna
        ;;
    bench-amm-base)
        echo "Running AMM swap benchmark with cuda (baseline)..."
        run_amm_bench cuda
        ;;
    bench-amm-cu)
        echo "Running AMM swap benchmark with cuvaruna..."
        run_amm_bench cuvaruna
        ;;
    varuna-cpu)
        echo "Running Varuna tall-matrix test (CPU)..."
        cargo test -p snarkvm-algorithms --lib prove_and_verify_with_tall_matrix_big -- --nocapture
        ;;
    varuna-cu-profile)
        echo "Running Varuna tall-matrix test (cuVaruna + profiling)..."
        cargo test -p snarkvm-algorithms --lib prove_and_verify_with_tall_matrix_big --features cuvaruna --features cuvaruna-profiling -- --nocapture
        ;;
    *)
        echo "Usage:"
        echo "  ./run.sh cpu               - Run CPU only (no CUDA)"
        echo "  ./run.sh base              - Run baseline (cuda feature only)"
        echo "  ./run.sh cu                - Run cuVaruna"
        echo "  ./run.sh cu debug          - Run cuVaruna with debug output"
        echo "  ./run.sh cu profile        - Run cuVaruna with profiling"
        echo "  ./run.sh cu debug profile  - Run cuVaruna with both"
        echo "  ./run.sh nsys [name]       - Build and run nsys profile (default: snarkvm_test_profile)"
        echo "  ./run.sh nsys-amm <fn> [secs] [name] - nsys profile a single AMM function (cuvaruna + profiling, default 20s)"
        echo "  ./run.sh bench-base        - Run benchmark with cuda (baseline)"
        echo "  ./run.sh bench-cu          - Run benchmark with cuvaruna"
        echo "  ./run.sh bench-amm-base    - Run AMM swap benchmark with cuda (baseline)"
        echo "  ./run.sh bench-amm-cu      - Run AMM swap benchmark with cuvaruna"
        echo "  ./run.sh varuna-cpu        - Run Varuna tall-matrix test (CPU)"
        echo "  ./run.sh varuna-cu-profile - Run Varuna tall-matrix test (cuVaruna + profiling)"
        exit 1
        ;;
esac
