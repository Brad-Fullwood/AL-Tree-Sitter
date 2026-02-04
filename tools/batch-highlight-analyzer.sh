#!/bin/bash
#
# Batch Highlight Analyzer
# Analyzes multiple AL files and generates aggregate statistics
#
# Usage:
#   ./batch-highlight-analyzer.sh <directory> [output_dir]
#
# Examples:
#   ./batch-highlight-analyzer.sh "/home/bradf/Dev/AL/Hawco - Tasklet"
#   ./batch-highlight-analyzer.sh "/home/bradf/Dev/AL/Hawco - Tasklet" ./analysis
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANALYZER="$SCRIPT_DIR/highlight-analyzer.py"

if [ -z "$1" ]; then
    echo "Usage: $0 <directory> [output_dir]"
    exit 1
fi

INPUT_DIR="$1"
OUTPUT_DIR="${2:-$SCRIPT_DIR/../analysis}"

mkdir -p "$OUTPUT_DIR"

echo "=== Batch Highlight Analysis ==="
echo "Input: $INPUT_DIR"
echo "Output: $OUTPUT_DIR"
echo ""

# Find all AL files
AL_FILES=$(find "$INPUT_DIR" -name "*.al" -type f 2>/dev/null)
FILE_COUNT=$(echo "$AL_FILES" | grep -c "." || echo 0)

echo "Found $FILE_COUNT AL files"
echo ""

# Analyze each file
TOTAL_CAPTURES=0
declare -A CAPTURE_COUNTS

echo "Analyzing files..."
for file in $AL_FILES; do
    echo -n "."

    # Get summary in JSON format
    OUTPUT=$(python "$ANALYZER" "$file" --json --summary 2>/dev/null || echo '{"summary":{}}')

    # Extract capture counts using Python
    python3 -c "
import json
import sys
try:
    data = json.loads('''$OUTPUT''')
    for capture, info in data.get('summary', {}).items():
        print(f'{capture}:{info[\"count\"]}')
except:
    pass
" | while read line; do
        capture=$(echo "$line" | cut -d: -f1)
        count=$(echo "$line" | cut -d: -f2)
        if [ -n "$capture" ] && [ -n "$count" ]; then
            current=${CAPTURE_COUNTS[$capture]:-0}
            CAPTURE_COUNTS[$capture]=$((current + count))
        fi
    done
done

echo ""
echo ""

# Generate aggregate report
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
REPORT_FILE="$OUTPUT_DIR/batch-report-$TIMESTAMP.txt"

{
    echo "=== Batch Highlight Analysis Report ==="
    echo "Generated: $(date)"
    echo "Input Directory: $INPUT_DIR"
    echo "Files Analyzed: $FILE_COUNT"
    echo ""
    echo "=== Aggregate Capture Statistics ==="

    # Run analysis on all files combined
    COMBINED_OUTPUT=""
    for file in $AL_FILES; do
        python "$ANALYZER" "$file" --json --summary 2>/dev/null
    done | python3 -c "
import json
import sys
from collections import defaultdict

totals = defaultdict(lambda: {'count': 0, 'examples': set()})

for line in sys.stdin:
    try:
        data = json.loads(line)
        for capture, info in data.get('summary', {}).items():
            totals[capture]['count'] += info['count']
            totals[capture]['examples'].update(info.get('examples', [])[:5])
    except:
        continue

print('Capture Type | Count | Example Values')
print('-' * 60)
for capture in sorted(totals.keys()):
    info = totals[capture]
    examples = list(info['examples'])[:5]
    print(f'{capture:25} | {info[\"count\"]:6} | {examples}')
"
} > "$REPORT_FILE"

echo "Report saved to: $REPORT_FILE"
cat "$REPORT_FILE"
