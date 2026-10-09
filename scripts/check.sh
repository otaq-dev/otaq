#!/usr/bin/env bash
# Crewboard local checks: fmt, clippy, tests.
#
# Run this before every commit. A green run is the definition of done for every ticket (decision 0056,
# SDD §23.2). The repository has no remote yet, so there are no GitHub Actions: this script is the CI.
#
# Works from any folder inside the repository. Stops at the first failing step and exits non-zero.

set -uo pipefail

if ! command -v git >/dev/null 2>&1; then
    echo "check.sh: git is required (core tests run against temporary git repositories)" >&2
    exit 1
fi

root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)" || {
    echo "check.sh: not inside the Crewboard repository" >&2
    exit 1
}
cd "$root" || exit 1

run_step() {
    local name="$1"
    shift
    echo "==> $name: $*"
    if ! "$@"; then
        echo "check.sh: FAILED at step '$name'" >&2
        exit 1
    fi
}

run_step fmt cargo fmt --all --check
run_step clippy cargo clippy --workspace --all-targets -- -D warnings
run_step test cargo test --workspace

echo "check.sh: all checks passed"
