default:
    @just --list

build:
    cargo build

# build if stale, then run
run *ARGS:
    cargo run -- {{ARGS}}

# run the last debug build as is, no build step
exec *ARGS:
    ./target/debug/nm {{ARGS}}

test:
    cargo test

ci:
    cargo test
    cargo clippy --all-targets -- -D warnings
    cargo fmt --check

# regenerate every tests/spec/cases/*.jsonl from the current `nm eval`;
# review the diff before committing: each change is a change in behaviour
spec-update: build
    #!/usr/bin/env bash
    set -euo pipefail
    bin=${CARGO_TARGET_DIR:-target}/debug/nm
    for f in tests/spec/cases/*.nm; do
        "$bin" eval <"$f" >"${f%.nm}.jsonl"
    done

# check the cargo-dist release config without pushing a tag
release-plan:
    dist plan

install:
    cargo install --path . --locked --force
