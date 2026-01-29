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

# Clean sibling Zed AL Extension if it exists
search_dir="../Zed AL Extension"
if [ -d "$search_dir" ]; then
    echo "🧹 Cleaning Zed AL Extension artifacts in $search_dir..."
    rm -rf "$search_dir/target"
    rm -f "$search_dir/extension.wasm"
    # Ensure no manual grammar copies exist
    rm -f "$search_dir/languages/al/highlights.scm"
    rm -rf "$search_dir/queries"
    echo "   ✅ Removed target/, extension.wasm, and potential stale queries."
else
    echo "⚠️  Sibling 'Zed AL Extension' directory not found at $search_dir"
fi

echo "✅ Clean complete."
