#!/usr/bin/env bash
# Otaq local checks: the frontend steps (Node version, npm ci, ts-rs freshness, tsc, ESLint, Prettier, Vitest,
# the page build), then fmt, clippy, tests.
#
# Run this before every commit. A green run is the definition of done for every ticket (decision 0056,
# SDD §23.2). There are no GitHub Actions yet: this script is the CI.
#
# Works from any folder inside the repository. Stops at the first failing step and exits non-zero. Needs Node 24 (the
# version in crates/otaq-desktop/.nvmrc) and npm on PATH; setup: scripts/README.md.

set -uo pipefail

if ! command -v git >/dev/null 2>&1; then
    echo "check.sh: git is required (core tests run against temporary git repositories)" >&2
    exit 1
fi

root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)" || {
    echo "check.sh: not inside the Otaq repository" >&2
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

# The frontend steps of SDD section 5.5 (decision 0075 point 7). The page lives inside the desktop crate; every npm
# command runs there.
desktop=crates/otaq-desktop
node_wanted="$(tr -d '[:space:]v' < "$desktop/.nvmrc")"
node_wanted="${node_wanted%%.*}"
echo "==> node: Node $node_wanted (from $desktop/.nvmrc)"
if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
    echo "check.sh: Node $node_wanted is required (node or npm not found on PATH); see $desktop/.nvmrc" >&2
    exit 1
fi
node_found="$(node --version)"
node_major="${node_found#v}"
node_major="${node_major%%.*}"
if [ "$node_major" != "$node_wanted" ]; then
    echo "check.sh: Node $node_wanted is required (found $node_found); see $desktop/.nvmrc" >&2
    exit 1
fi

run_step npm-ci npm --prefix "$desktop" ci --no-audit --no-fund
# Before tsc, so a stale generated type is reported as stale rather than as a type error.
run_step types cargo test -p otaq-desktop generated_types_are_fresh
run_step tsc npm --prefix "$desktop" run typecheck
run_step eslint npm --prefix "$desktop" run lint
run_step prettier npm --prefix "$desktop" run format:check
run_step vitest npm --prefix "$desktop" run test
# Plain cargo builds don't embed ui/dist (only the Tauri CLI's release build does), but building the page here catches
# what tsc doesn't, such as an import Vite can't resolve, before the cargo steps.
run_step page npm --prefix "$desktop" run build:page

run_step fmt cargo fmt --all --check
run_step clippy cargo clippy --workspace --all-targets -- -D warnings
run_step test cargo test --workspace

echo "check.sh: all checks passed"
