#!/usr/bin/env python3
"""
AL Syntax Highlighting Analyzer

Analyzes tree-sitter syntax highlighting captures for AL files.
Outputs detailed capture information for comparison and refinement.

Usage:
    python highlight-analyzer.py <al_file> [--json] [--summary] [--compare <vscode_captures.json>]

Examples:
    python highlight-analyzer.py test.al                    # Full capture output
    python highlight-analyzer.py test.al --json             # JSON output for processing
    python highlight-analyzer.py test.al --summary          # Summary by capture type
    python highlight-analyzer.py test.al --json > out.json  # Save for later analysis
"""

import subprocess
import json
import sys
import os
import re
from pathlib import Path
from collections import defaultdict
from typing import Dict, List, Tuple, Optional

# Find the tree-sitter grammar root
SCRIPT_DIR = Path(__file__).parent
GRAMMAR_ROOT = SCRIPT_DIR.parent
QUERIES_DIR = GRAMMAR_ROOT / "queries"
HIGHLIGHTS_SCM = QUERIES_DIR / "highlights.scm"


def run_tree_sitter_parse(file_path: str) -> str:
    """Run tree-sitter parse and return the S-expression tree."""
    result = subprocess.run(
        ["tree-sitter", "parse", file_path],
        capture_output=True,
        text=True,
        cwd=GRAMMAR_ROOT
    )
    return result.stdout


def run_tree_sitter_query(file_path: str, query_file: str) -> str:
    """Run tree-sitter query and return captures."""
    result = subprocess.run(
        ["tree-sitter", "query", query_file, file_path, "--captures"],
        capture_output=True,
        text=True,
        cwd=GRAMMAR_ROOT
    )
    if result.returncode != 0:
        print(f"Query error: {result.stderr}", file=sys.stderr)
    return result.stdout


def parse_captures(query_output: str) -> List[Dict]:
    """Parse tree-sitter query capture output into structured data."""
    captures = []

    # Pattern matches tree-sitter query output format:
    # pattern: 136, capture: 5 - keyword, start: (0, 0), end: (0, 14), text: `tableextension`
    pattern = r'pattern:\s*\d+,\s*capture:\s*\d+\s*-\s*(\w+(?:\.\w+)*),\s*start:\s*\((\d+),\s*(\d+)\),\s*end:\s*\((\d+),\s*(\d+)\),\s*text:\s*`([^`]*)`'

    for match in re.finditer(pattern, query_output):
        capture_name = match.group(1)
        start_row = int(match.group(2))
        start_col = int(match.group(3))
        end_row = int(match.group(4))
        end_col = int(match.group(5))
        text = match.group(6)

        captures.append({
            "capture": f"@{capture_name}",
            "node_type": "unknown",  # Not in this output format
            "start": {"row": start_row, "col": start_col},
            "end": {"row": end_row, "col": end_col},
            "text": text,
        })

    return captures


def add_text_to_captures(captures: List[Dict], file_path: str) -> List[Dict]:
    """Add the actual source text to each capture (if not already present)."""
    # If text is already present from query output, skip
    if captures and "text" in captures[0] and captures[0]["text"]:
        return captures

    with open(file_path, 'r') as f:
        lines = f.readlines()

    for cap in captures:
        if "text" in cap and cap["text"]:
            continue

        start = cap["start"]
        end = cap["end"]

        if start["row"] == end["row"]:
            # Single line capture
            line = lines[start["row"]] if start["row"] < len(lines) else ""
            cap["text"] = line[start["col"]:end["col"]]
        else:
            # Multi-line capture
            text_parts = []
            for row in range(start["row"], end["row"] + 1):
                if row >= len(lines):
                    break
                line = lines[row]
                if row == start["row"]:
                    text_parts.append(line[start["col"]:])
                elif row == end["row"]:
                    text_parts.append(line[:end["col"]])
                else:
                    text_parts.append(line)
            cap["text"] = "".join(text_parts).strip()

    return captures


def generate_summary(captures: List[Dict]) -> Dict:
    """Generate a summary of captures by type."""
    by_capture = defaultdict(list)
    for cap in captures:
        by_capture[cap["capture"]].append(cap["text"])

    summary = {}
    for capture_name, texts in sorted(by_capture.items()):
        unique_texts = sorted(set(texts))
        summary[capture_name] = {
            "count": len(texts),
            "unique_count": len(unique_texts),
            "examples": unique_texts[:10],  # First 10 unique examples
        }

    return summary


def generate_line_report(captures: List[Dict], file_path: str) -> List[Dict]:
    """Generate a line-by-line report showing what's highlighted."""
    with open(file_path, 'r') as f:
        lines = f.readlines()

    # Group captures by line
    by_line = defaultdict(list)
    for cap in captures:
        by_line[cap["start"]["row"]].append(cap)

    report = []
    for line_num, line in enumerate(lines):
        line_caps = by_line.get(line_num, [])
        # Sort by column
        line_caps.sort(key=lambda c: c["start"]["col"])

        report.append({
            "line": line_num + 1,  # 1-indexed for display
            "text": line.rstrip(),
            "captures": [
                {
                    "col": c["start"]["col"],
                    "end_col": c["end"]["col"],
                    "capture": c["capture"],
                    "node": c["node_type"],
                    "text": c["text"]
                }
                for c in line_caps
            ]
        })

    return report


def format_text_output(captures: List[Dict], file_path: str) -> str:
    """Format captures as readable text output."""
    lines = []
    lines.append(f"=== Syntax Highlighting Analysis: {file_path} ===\n")

    # Summary first
    summary = generate_summary(captures)
    lines.append("--- Capture Summary ---")
    for capture_name, data in summary.items():
        lines.append(f"{capture_name}: {data['count']} occurrences ({data['unique_count']} unique)")
        for ex in data['examples'][:5]:
            lines.append(f"    {repr(ex)}")
    lines.append("")

    # Line-by-line detail
    lines.append("--- Line-by-Line Detail ---")
    report = generate_line_report(captures, file_path)

    for entry in report:
        if entry["captures"]:
            lines.append(f"\nLine {entry['line']}: {entry['text']}")
            for cap in entry["captures"]:
                lines.append(f"  [{cap['col']}-{cap['end_col']}] {cap['capture']} ({cap['node']}): {repr(cap['text'])}")

    return "\n".join(lines)


def analyze_missing_highlights(captures: List[Dict], file_path: str) -> Dict:
    """Analyze what tokens are NOT highlighted (might need captures)."""
    with open(file_path, 'r') as f:
        content = f.read()
        lines = content.split('\n')

    # Build a coverage map
    covered = set()
    for cap in captures:
        for row in range(cap["start"]["row"], cap["end"]["row"] + 1):
            start_col = cap["start"]["col"] if row == cap["start"]["row"] else 0
            end_col = cap["end"]["col"] if row == cap["end"]["row"] else len(lines[row]) if row < len(lines) else 0
            for col in range(start_col, end_col):
                covered.add((row, col))

    # Find uncovered identifiers (potential missing highlights)
    uncovered_tokens = []
    identifier_pattern = r'\b[a-zA-Z_][a-zA-Z0-9_]*\b'

    for row, line in enumerate(lines):
        for match in re.finditer(identifier_pattern, line):
            start_col = match.start()
            end_col = match.end()

            # Check if any part is uncovered
            is_covered = all((row, col) in covered for col in range(start_col, end_col))

            if not is_covered:
                token = match.group()
                # Skip common false positives
                if token.lower() not in {'the', 'a', 'an', 'is', 'are', 'was', 'were'}:
                    uncovered_tokens.append({
                        "text": token,
                        "line": row + 1,
                        "col": start_col
                    })

    # Group by token text
    by_token = defaultdict(list)
    for tok in uncovered_tokens:
        by_token[tok["text"]].append(f"L{tok['line']}:{tok['col']}")

    return {
        "total_uncovered": len(uncovered_tokens),
        "unique_uncovered": len(by_token),
        "tokens": {k: {"count": len(v), "locations": v[:5]} for k, v in sorted(by_token.items())}
    }


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Analyze AL syntax highlighting captures")
    parser.add_argument("file", help="AL file to analyze")
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    parser.add_argument("--summary", action="store_true", help="Only show summary")
    parser.add_argument("--uncovered", action="store_true", help="Show uncovered tokens (potential missing highlights)")
    parser.add_argument("--query", default=str(HIGHLIGHTS_SCM), help="Path to highlights.scm query file")
    parser.add_argument("--output", "-o", help="Output file (default: stdout)")

    args = parser.parse_args()

    if not os.path.exists(args.file):
        print(f"Error: File not found: {args.file}", file=sys.stderr)
        sys.exit(1)

    if not os.path.exists(args.query):
        print(f"Error: Query file not found: {args.query}", file=sys.stderr)
        sys.exit(1)

    # Run query
    query_output = run_tree_sitter_query(args.file, args.query)

    # Parse captures
    captures = parse_captures(query_output)
    captures = add_text_to_captures(captures, args.file)

    # Generate output
    if args.json:
        output_data = {
            "file": args.file,
            "query": args.query,
            "total_captures": len(captures),
            "summary": generate_summary(captures),
            "captures": captures if not args.summary else None,
        }

        if args.uncovered:
            output_data["uncovered"] = analyze_missing_highlights(captures, args.file)

        output = json.dumps(output_data, indent=2)
    else:
        if args.summary:
            summary = generate_summary(captures)
            lines = [f"=== Capture Summary: {args.file} ===\n"]
            for capture_name, data in summary.items():
                lines.append(f"{capture_name}: {data['count']} ({data['unique_count']} unique)")
            output = "\n".join(lines)
        else:
            output = format_text_output(captures, args.file)

        if args.uncovered:
            uncovered = analyze_missing_highlights(captures, args.file)
            output += f"\n\n=== Uncovered Tokens ({uncovered['total_uncovered']} total, {uncovered['unique_uncovered']} unique) ===\n"
            for token, data in list(uncovered['tokens'].items())[:20]:
                output += f"  {token}: {data['count']}x at {', '.join(data['locations'])}\n"

    # Output
    if args.output:
        with open(args.output, 'w') as f:
            f.write(output)
        print(f"Output written to: {args.output}")
    else:
        print(output)


if __name__ == "__main__":
    main()
