#!/bin/bash
set -e

echo "🧹 Cleaning generated artifacts..."

# Clean root artifacts
rm -f grammar.js grammar.json node-types.json
rm -rf src/
rm -rf queries/
rm -rf target/
rm -rf build/
rm -f parser.so

# Clean generator artifacts
if [ -d "generator" ]; then
    echo "🧹 Cleaning generator..."
    cd generator
    cargo clean
    rm -rf target/
    cd ..
fi

echo "✅ Clean complete."
