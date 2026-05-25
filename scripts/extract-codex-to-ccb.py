#!/usr/bin/env python3
"""
Extract CodeX-2M-Thinking dataset into CCB expert YAML format.

Samples rows by domain/language, extracts thinking patterns from <think> blocks,
and writes one YAML file per CCB expert persona (coder, architect, debugger).

Usage:
    python3 extract-codex-to-ccb.py <dataset_dir> <output_dir> [--sample N]
"""

import argparse
import os
import re
import sys
from pathlib import Path
from collections import defaultdict

try:
    import datasets as hf_datasets
    import yaml
except ImportError:
    print("Missing deps: pip install datasets pyyaml")
    sys.exit(1)

SAMPLES_PER_DOMAIN = 500  # per language/domain bucket

# Map dataset content signals to CCB expert personas
PERSONA_ROUTING = {
    "coder": {
        "keywords": ["implement", "write a function", "algorithm", "leetcode",
                     "data structure", "sort", "search", "dynamic programming",
                     "recursion", "graph", "tree", "array", "string manipulation"],
        "languages": ["python", "javascript", "java", "c++"],
    },
    "architect": {
        "keywords": ["design", "system design", "architecture", "api", "database",
                     "microservice", "scalab", "distributed", "schema", "model",
                     "pattern", "abstraction", "interface", "class hierarchy"],
        "languages": ["python", "java", "typescript"],
    },
    "debugger": {
        "keywords": ["bug", "fix", "error", "exception", "debug", "trace",
                     "stack overflow", "null", "undefined", "segfault", "memory",
                     "race condition", "deadlock", "timeout", "regression"],
        "languages": ["python", "c++", "java", "javascript"],
    },
}

def extract_think_block(output: str) -> str:
    """Extract content inside <think>...</think> tags."""
    m = re.search(r"<think>(.*?)</think>", output, re.DOTALL)
    return m.group(1).strip() if m else ""

def extract_code_solution(output: str) -> str:
    """Extract code after </think> block."""
    parts = re.split(r"</think>", output, maxsplit=1)
    return parts[1].strip() if len(parts) > 1 else output.strip()

def classify_row(input_text: str, persona_routing: dict) -> str | None:
    """Return persona name for a row, or None if no match."""
    text = input_text.lower()
    scores = defaultdict(int)
    for persona, cfg in persona_routing.items():
        for kw in cfg["keywords"]:
            if kw in text:
                scores[persona] += 1
    if not scores:
        return None
    return max(scores, key=scores.__getitem__)

def think_to_pattern(idx: int, input_text: str, think: str) -> dict:
    """Convert a thinking trace to a CCB pattern entry."""
    # Use first line of problem as pattern name (trimmed)
    first_line = input_text.split("\n")[0].strip()[:80]
    name = re.sub(r"[^a-zA-Z0-9 \-_]", "", first_line).strip()
    if not name:
        name = f"pattern-{idx}"

    # Extract key reasoning steps as mitigations (sentences starting with action verbs)
    sentences = re.split(r"(?<=[.!?])\s+", think)
    mitigations = []
    action_starters = ("first", "then", "next", "so ", "we ", "to ", "the key",
                       "note", "check", "use ", "apply", "consider", "start",
                       "since", "because", "this means")
    for s in sentences:
        s = s.strip()
        if len(s) > 20 and s.lower().startswith(action_starters):
            mitigations.append(s[:200])
        if len(mitigations) >= 4:
            break

    if not mitigations:
        # Fallback: first 3 non-empty sentences
        mitigations = [s.strip()[:200] for s in sentences[:3] if len(s.strip()) > 20]

    return {
        "id": f"CX-{idx:06d}",
        "name": name,
        "mitigations": mitigations or ["See thinking trace for solution approach."],
    }

def build_yaml_for_persona(persona: str, patterns_by_domain: dict) -> dict:
    domains = []
    for domain_name, patterns in patterns_by_domain.items():
        domains.append({
            "name": domain_name,
            "category": "codex-2m",
            "patterns": patterns,
        })
    return {
        "personas": [{
            "name": persona,
            "description": f"Code reasoning expert — {persona} persona, distilled from CodeX-2M-Thinking dataset (Apache 2.0).",
            "domains": domains,
        }]
    }

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("dataset_dir", help="Path to downloaded CodeX-2M-Thinking parquet files")
    parser.add_argument("output_dir", help="Directory to write CCB YAML files")
    parser.add_argument("--sample", type=int, default=SAMPLES_PER_DOMAIN,
                        help=f"Rows per domain bucket (default {SAMPLES_PER_DOMAIN})")
    args = parser.parse_args()

    out_dir = Path(args.output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    print(f"Loading dataset from {args.dataset_dir}...")
    ds = hf_datasets.load_dataset(
        "parquet",
        data_dir=args.dataset_dir,
        split="train",
        streaming=True,
    )

    persona_buckets: dict[str, dict[str, list]] = {p: defaultdict(list) for p in PERSONA_ROUTING}
    persona_counts = defaultdict(int)
    target = args.sample * len(PERSONA_ROUTING)
    processed = 0

    print(f"Sampling up to {args.sample} rows per persona...")
    for row in ds:
        if all(persona_counts[p] >= args.sample for p in PERSONA_ROUTING):
            break

        persona = classify_row(row["input"], PERSONA_ROUTING)
        if not persona:
            continue
        if persona_counts[persona] >= args.sample:
            continue

        think = extract_think_block(row["output"])
        if len(think) < 100:
            continue

        # Use first keyword match as domain label
        domain = "general"
        text = row["input"].lower()
        for kw in PERSONA_ROUTING[persona]["keywords"]:
            if kw in text:
                domain = kw.replace(" ", "-")[:30]
                break

        pattern = think_to_pattern(persona_counts[persona], row["input"], think)
        persona_buckets[persona][domain].append(pattern)
        persona_counts[persona] += 1
        processed += 1

        if processed % 500 == 0:
            print(f"  {processed} rows sampled: " +
                  ", ".join(f"{p}={persona_counts[p]}" for p in PERSONA_ROUTING))

    print(f"\nExtraction complete: {processed} rows → {sum(persona_counts.values())} patterns")
    for persona, buckets in persona_buckets.items():
        if not any(buckets.values()):
            continue
        out_path = out_dir / f"{persona}.yaml"
        data = build_yaml_for_persona(persona, dict(buckets))
        with open(out_path, "w") as f:
            yaml.dump(data, f, allow_unicode=True, sort_keys=False)
        total = sum(len(p) for p in buckets.values())
        print(f"  {out_path.name}: {total} patterns across {len(buckets)} domains")

    print(f"\nYAML files written to {out_dir}")

if __name__ == "__main__":
    main()
