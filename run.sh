#!/bin/bash

set -e

BASE_CMD="cargo test test_transfer_private_execution --package snarkvm-synthesizer --lib --release"
SUFFIX="-- --nocapture"

case "$1" in
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
    *)
        echo "Usage:"
        echo "  ./run.sh base              - Run baseline (cuda feature only)"
        echo "  ./run.sh cu                - Run cuVaruna"
        echo "  ./run.sh cu debug          - Run cuVaruna with debug output"
        echo "  ./run.sh cu profile        - Run cuVaruna with profiling"
        echo "  ./run.sh cu debug profile  - Run cuVaruna with both"
        echo "  ./run.sh nsys [name]       - Build and run nsys profile (default: snarkvm_test_profile)"
        exit 1
        ;;
esac
