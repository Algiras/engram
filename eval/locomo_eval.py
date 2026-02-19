#!/usr/bin/env python3
"""
LoCoMo Benchmark Evaluation for engram
=======================================
Runs engram against the official LoCoMo-10 dataset and scores with token-overlap F1,
matching the Mem0 / MemoryOS evaluation protocol.

Usage:
    # Quick smoke test (2 conversations, Gemini)
    python eval/locomo_eval.py --provider gemini --max-convs 2

    # Full benchmark run
    python eval/locomo_eval.py --provider gemini --workers 4

    # With graph-augmented retrieval (Track 2)
    python eval/locomo_eval.py --provider gemini --use-graph

Data source: eval/locomo10.json (official LoCoMo-10 from snap-research/locomo)

Category mapping (from the paper):
    1 = single-hop     (factual recall from one session)
    2 = temporal       (time/date-based)
    3 = open-ended     (adversarial / requires inference)
    4 = multi-hop      (cross-session reasoning, within speaker)
    5 = multi-hop-cs   (cross-session, cross-speaker)

Baselines (token-overlap F1 from Mem0 paper):
    GPT-4 (no memory):  32.1
    Mem0:               67.1  (overall), 26.7/51.1 by speaker split
    Human ceiling:      87.9
"""

import argparse
import json
import os
import string
import subprocess
import sys
import time
import unicodedata
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Optional

# ---------------------------------------------------------------------------
# Official LoCoMo F1 scoring (matches snap-research/locomo evaluation.py)
# ---------------------------------------------------------------------------

def normalize_answer(s) -> str:
    """Lower, remove punctuation, articles, extra whitespace."""
    s = str(s).replace(",", "")
    s = unicodedata.normalize("NFD", s)

    import re
    s = re.sub(r"\b(a|an|the|and)\b", " ", s.lower())
    s = re.sub(r"[^a-z0-9 ]", " ", s)
    return " ".join(s.split())


def token_f1(prediction: str, gold: str) -> float:
    pred_tokens = normalize_answer(prediction).split()
    gold_tokens = normalize_answer(gold).split()
    if not pred_tokens or not gold_tokens:
        return float(pred_tokens == gold_tokens)
    common = set(pred_tokens) & set(gold_tokens)
    if not common:
        return 0.0
    precision = len(common) / len(pred_tokens)
    recall = len(common) / len(gold_tokens)
    return 2 * precision * recall / (precision + recall)


# ---------------------------------------------------------------------------
# Category labels
# ---------------------------------------------------------------------------

CATEGORY_NAMES = {
    1: "single_hop",
    2: "temporal",
    3: "open_ended",
    4: "multi_hop",
    5: "multi_hop_cs",
}

# ---------------------------------------------------------------------------
# engram subprocess driver
# ---------------------------------------------------------------------------

class EngramRunner:
    def __init__(self, binary: str, provider: str = "gemini"):
        self.binary = binary
        self.provider = provider

    def _run(self, *args, timeout: int = 60, input_text: str | None = None) -> tuple[int, str, str]:
        result = subprocess.run(
            [self.binary, *args],
            capture_output=True,
            text=True,
            timeout=timeout,
            input=input_text,
        )
        return result.returncode, result.stdout, result.stderr

    def add(self, project: str, category: str, content: str, label: str) -> bool:
        rc, _, err = self._run("add", project, category, content, "--label", label)
        if rc != 0:
            print(f"    [warn] add failed: {err.strip()[:80]}", file=sys.stderr)
        return rc == 0

    def embed(self, project: str) -> bool:
        rc, _, err = self._run("embed", project, "--provider", self.provider, timeout=180)
        if rc != 0:
            print(f"    [warn] embed failed: {err.strip()[:80]}", file=sys.stderr)
        return rc == 0

    def graph_build(self, project: str) -> bool:
        rc, _, _ = self._run("graph", "build", project, timeout=120)
        return rc == 0

    def ask(self, project: str, question: str, threshold: float = 0.25, use_graph: bool = False) -> str:
        args = ["ask", question, "--project", project, "--threshold", str(threshold)]
        if use_graph:
            args.append("--use-graph")
        rc, stdout, _ = self._run(*args, timeout=30)
        if rc != 0 or "Not found in knowledge base" in stdout:
            return ""
        # Strip "Sources: ..." line
        lines = [l for l in stdout.strip().splitlines() if not l.startswith("Sources:")]
        return "\n".join(lines).strip()

    def forget(self, project: str):
        self._run("forget", project, "--force", timeout=10)


# ---------------------------------------------------------------------------
# Dataset ingestion
# ---------------------------------------------------------------------------

def ingest_conversation(engram: EngramRunner, project: str, conv: dict) -> int:
    """
    Ingest a LoCoMo conversation into an engram project.
    Each session → one 'solutions' entry (all turns concatenated).
    Returns number of sessions ingested.
    """
    count = 0
    for i in range(1, 50):
        key = f"session_{i}"
        if key not in conv["conversation"] or not conv["conversation"][key]:
            break
        turns = conv["conversation"][key]
        text = "\n".join(
            f"{t['speaker']}: {t['text']}"
            for t in turns
            if t.get("text")
        )
        if text.strip():
            engram.add(project, "solutions", text[:4000], f"session-{i}")
            count += 1
    return count


# ---------------------------------------------------------------------------
# Per-conversation evaluation
# ---------------------------------------------------------------------------

def eval_conversation(
    engram: EngramRunner,
    conv: dict,
    use_graph: bool = False,
    verbose: bool = False,
) -> dict[int, list[float]]:
    """Returns {category_int: [f1_scores]}."""
    sample_id = conv["sample_id"]
    project = f"locomo-{sample_id}"
    scores: dict[int, list[float]] = defaultdict(list)

    try:
        # Ingest
        n = ingest_conversation(engram, project, conv)
        if n == 0:
            return scores

        # Build embedding index
        engram.embed(project)

        # Optionally build graph
        if use_graph:
            engram.graph_build(project)

        # Answer QA pairs
        for qa in conv["qa"]:
            question = qa.get("question", "")
            gold = qa.get("answer", "")
            category = qa.get("category", 0)

            if not question or not gold:
                continue

            prediction = engram.ask(project, question, use_graph=use_graph)
            f1 = token_f1(prediction, gold)
            scores[category].append(f1)

            if verbose:
                cat_name = CATEGORY_NAMES.get(category, str(category))
                print(f"    [{cat_name}] Q: {question[:60]}... F1={f1:.2f}")

    finally:
        engram.forget(project)

    return scores


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------

BASELINES = {
    "GPT-4 (no memory)": 32.1,
    "Mem0":               67.1,
    "Human ceiling":      87.9,
}


def print_report(all_scores: dict[int, list[float]], elapsed: float, n_convs: int, use_graph: bool):
    flat = [s for vals in all_scores.values() for s in vals]
    overall = sum(flat) / len(flat) * 100 if flat else 0.0

    print()
    print("╔══════════════════════════════════════════════════════╗")
    print(f"  engram LoCoMo-10 Results  |  {'+ Graph' if use_graph else 'No Graph'}")
    print(f"  Conversations: {n_convs}  |  QA pairs: {len(flat)}  |  {elapsed:.0f}s")
    print("╠══════════════════════════════════════════════════════╣")
    print(f"  Overall F1:  {overall:.1f}")
    print()
    print("  By category:")
    for cat_id in sorted(CATEGORY_NAMES):
        vals = all_scores.get(cat_id, [])
        if vals:
            avg = sum(vals) / len(vals) * 100
            bar = "█" * int(avg / 5)
            print(f"    {CATEGORY_NAMES[cat_id]:<16} {avg:5.1f}  {bar}")
    print()
    print("  Comparison:")
    for name, b in BASELINES.items():
        marker = " ◀ SOTA" if name == "Mem0" else ""
        print(f"    {name:<25} {b:5.1f}{marker}")
    marker = " ◀ engram ✓" if overall >= 67.1 else f" (gap: {67.1 - overall:.1f})"
    print(f"    {'engram (this run)':<25} {overall:5.1f}{marker}")
    print("╚══════════════════════════════════════════════════════╝")

    if overall >= 67.1:
        print("\n  🎉  SOTA ACHIEVED: engram ≥ Mem0!")
    elif overall >= 50.0:
        print(f"\n  Strong result — {67.1 - overall:.1f} F1 points below Mem0")
    else:
        print(f"\n  Gap to Mem0: {67.1 - overall:.1f} F1 points")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser(description="LoCoMo-10 benchmark for engram")
    ap.add_argument("--engram", default="./target/release/engram")
    ap.add_argument("--data", default="eval/locomo10.json")
    ap.add_argument("--provider", default="gemini", choices=["gemini", "openai", "ollama"])
    ap.add_argument("--max-convs", type=int, default=None)
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--use-graph", action="store_true")
    ap.add_argument("--verbose", action="store_true")
    ap.add_argument("--output", default="eval/results.json")
    args = ap.parse_args()

    binary = str(Path(args.engram).resolve())
    if not Path(binary).exists():
        print(f"ERROR: {binary} not found. Run: cargo build --release", file=sys.stderr)
        sys.exit(1)

    data_path = Path(args.data)
    if not data_path.exists():
        print(f"ERROR: {data_path} not found.", file=sys.stderr)
        print("Download: curl -sL https://raw.githubusercontent.com/snap-research/locomo/main/data/locomo10.json -o eval/locomo10.json", file=sys.stderr)
        sys.exit(1)

    with open(data_path) as f:
        dataset = json.load(f)

    if args.max_convs:
        dataset = dataset[: args.max_convs]

    engram = EngramRunner(binary, args.provider)

    print(f"engram LoCoMo eval")
    print(f"  Binary:    {binary}")
    print(f"  Provider:  {args.provider}")
    print(f"  Dataset:   {len(dataset)} conversations, {sum(len(d['qa']) for d in dataset)} QA pairs")
    print(f"  Graph:     {args.use_graph}")
    print(f"  Workers:   {args.workers}")
    print()

    all_scores: dict[int, list[float]] = defaultdict(list)
    start = time.time()

    def process(conv):
        sid = conv["sample_id"]
        print(f"  [{sid}] starting ({len(conv['qa'])} QA pairs)...", flush=True)
        scores = eval_conversation(engram, conv, use_graph=args.use_graph, verbose=args.verbose)
        n = sum(len(v) for v in scores.values())
        f1 = sum(s for v in scores.values() for s in v) / n * 100 if n else 0.0
        print(f"  [{sid}] done — F1={f1:.1f} ({n} pairs)", flush=True)
        return scores

    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        for scores in pool.map(process, dataset):
            for cat, vals in scores.items():
                all_scores[cat].extend(vals)

    elapsed = time.time() - start

    # Save raw
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w") as f:
        json.dump(
            {"scores": {str(k): v for k, v in all_scores.items()},
             "elapsed": elapsed, "n_convs": len(dataset),
             "use_graph": args.use_graph, "provider": args.provider},
            f, indent=2,
        )
    print(f"\nRaw scores → {args.output}")

    print_report(all_scores, elapsed, len(dataset), args.use_graph)


if __name__ == "__main__":
    main()
