#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-1}"
if [ -z "${BUNTRAIL_STATE_DIR:-}" ]; then
  state=$(mktemp -d "${TMPDIR:-/tmp}/buntrail-check.XXXXXXXX")
  trap 'rm -rf "$state"' EXIT HUP INT TERM
  export BUNTRAIL_STATE_DIR="$state"
fi
cargo fmt --all --check
cargo test --all-features --offline
cargo test --examples --all-features --offline
cargo clippy --all-targets --all-features --offline -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --offline
