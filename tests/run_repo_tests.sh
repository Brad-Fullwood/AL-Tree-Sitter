#!/usr/bin/env bash
set -euo pipefail

# Runs focused fixtures and the real-repository validation suite against the
# committed generated parser (clones into ./tests/.repos/).
# Requires only: git, tree-sitter CLI, Rust toolchain. Full regeneration from
# Microsoft's proprietary AL extension is a separate profile.

cd "$(dirname "$0")/.."

tests/check_cli_version.sh
cargo run --release --manifest-path generator/Cargo.toml -- --repo-tests-only
