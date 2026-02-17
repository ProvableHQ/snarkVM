#!/bin/bash

set -e

BASE_CMD="cargo test test_transfer_private_execution --package snarkvm-synthesizer --lib --release"
SUFFIX="-- --nocapture"

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
    bench-base)
        echo "Running benchmark with cuda (baseline)..."
        cargo bench --package snarkvm-synthesizer --bench execute_authorization --features cuda
        ;;
    bench-cu)
        echo "Running benchmark with cuvaruna..."
        cargo bench --package snarkvm-synthesizer --bench execute_authorization --features cuvaruna
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
        echo "  ./run.sh bench-base        - Run benchmark with cuda (baseline)"
        echo "  ./run.sh bench-cu          - Run benchmark with cuvaruna"
        echo "  ./run.sh varuna-cpu        - Run Varuna tall-matrix test (CPU)"
        echo "  ./run.sh varuna-cu-profile - Run Varuna tall-matrix test (cuVaruna + profiling)"
        exit 1
        ;;
esac
