# CI overview

snarkVM makes use of [CircleCI](.circleci) and [Github Actions](.github/workflows).

When a PR is opened from a feature branch, several CircleCI workflows are
triggered. Tests are spread across workflows for readability in Github's UI, and
are targeting a 15 minute max runtime.
- `cargo-workflow`
- `circuit-workflow`
- `console-workflow`
- `ledger-workflow`
- `synthesizer-workflow`

When a PR is merged, `merge-workflow` runs additional expensive or slow tests,
e.g. those marked as `ignore` or which download paramters. Moreover, benchmarks
are run from github actions.s

When a PR is opened from a release branch (`canary`,`testnet`,`mainnet`), the
`release-workflow` is ran which tests windows.