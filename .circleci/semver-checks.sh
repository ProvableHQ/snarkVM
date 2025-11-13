#! /bin/bash

set -x

# Ensure that the command is installed.
cargo install cargo-semver-checks@0.43.0 --locked

BASELINE_REV=5eb528f # UPDATE ME ON NECESSARY BREAKING CHANGES

# Exclude CLI as it has been removed
cargo semver-checks --workspace --default-features --baseline-rev $BASELINE_REV
