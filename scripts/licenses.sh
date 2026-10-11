#!/usr/bin/env bash
# Regenerate THIRD-PARTY-LICENSES.md: the notices of every crate in the shipped binaries, for all three platforms.
#
# Run it after changing dependencies (Cargo.toml or Cargo.lock), and commit the result with that change.
# `scripts/licenses.sh --check` fails when the committed file is out of date.
#
# Needs cargo-about: `cargo install cargo-about --locked --features cli`.

set -euo pipefail

root="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
cd "$root"

if ! cargo about --version >/dev/null 2>&1; then
    echo "licenses.sh: cargo-about is missing: cargo install cargo-about --locked --features cli" >&2
    exit 1
fi

out="THIRD-PARTY-LICENSES.md"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

# --locked also means --offline: only the license files the crates ship are read, so every run gives the same file.
cargo about generate --locked --fail -c scripts/licenses/about.toml -o "$tmp.raw" scripts/licenses/about.hbs
# Crewboard's own crates are in the graph too; their license is LICENSE, not a third-party notice.
grep -v '^- \[crewboard-' "$tmp.raw" >"$tmp"
rm -f "$tmp.raw"

if [[ "${1:-}" == "--check" ]]; then
    if ! cmp -s "$tmp" "$out"; then
        echo "licenses.sh: $out is out of date; run scripts/licenses.sh and commit it" >&2
        exit 1
    fi
    echo "licenses.sh: $out is up to date"
else
    mv "$tmp" "$out"
    trap - EXIT
    echo "licenses.sh: wrote $out"
fi
