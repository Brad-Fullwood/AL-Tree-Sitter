#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_FILE="${ROOT}/.tree-sitter-cli-version"

[ -s "${VERSION_FILE}" ] || {
    echo "ERROR: missing canonical tree-sitter CLI version file: ${VERSION_FILE}" >&2
    exit 1
}

command -v tree-sitter >/dev/null 2>&1 || {
    echo "ERROR: tree-sitter CLI is not installed" >&2
    exit 1
}

required="$(tr -d '[:space:]' < "${VERSION_FILE}")"
actual="$(tree-sitter --version 2>/dev/null | awk '{print $2}')"

[ -n "${actual}" ] || {
    echo "ERROR: could not determine the installed tree-sitter CLI version" >&2
    exit 1
}

if [ "${actual}" != "${required}" ]; then
    echo "ERROR: tree-sitter CLI ${actual} is installed; this repository requires exactly ${required}" >&2
    echo "Install it with: cargo install tree-sitter-cli --version ${required} --locked" >&2
    exit 1
fi

# The generator's corpus runner relies on the 0.26+ prebuilt-library and JSON
# summary interface. Checking the flags makes a repackaged/incompatible binary
# fail before it can rewrite generated sources.
help="$(tree-sitter parse --help)"
grep -q -- '--lib-path' <<< "${help}" || {
    echo "ERROR: tree-sitter ${actual} lacks parse --lib-path" >&2
    exit 1
}
grep -q -- '--json-summary' <<< "${help}" || {
    echo "ERROR: tree-sitter ${actual} lacks parse --json-summary" >&2
    exit 1
}

echo "OK: tree-sitter CLI ${actual}"
