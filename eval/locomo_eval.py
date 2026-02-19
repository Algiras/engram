#!/usr/bin/env python3
"""
LoCoMo Benchmark Evaluation for engram
=======================================
Measures RAG recall accuracy against the LoCoMo long-term conversational memory dataset.

Usage:
    python eval/locomo_eval.py --engram ./target/release/engram --provider ollama
    python eval/locomo_eval.py --engram ./target/release/engram --provider gemini --max-convs 5

Baseline comparison (from Mem0 paper):
    GPT-4 (no memory):  32.1 F1
    Mem0:               67.1 F1  (overall), 51.1 F1 (strict)
    Human ceiling:      87.9 F1
"""

import argparse
import json
import os
import re
import shutil
import string
import subprocess
import sys
import tempfile
import time
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Optional

# ---------------------------------------------------------------------------
# F1 scoring (token-overlap, matching LoCoMo / Mem0 evaluation protocol)
# ---------------------------------------------------------------------------

def normalize(text: str) -> list[str]:
    """Lowercase, strip punctuation, tokenize on whitespace."""
    text = text.lower()
    text = text.translate(str.maketrans("", "", string.punctuation))
    return text.split()


def f1_score(prediction: str, gold: str) -> float:
    pred_tokens = normalize(prediction)
    gold_tokens = normalize(gold)
    if not pred_tokens or not gold_tokens:
        return float(pred_tokens == gold_tokens)
    common = set(pred_tokens) & set(gold_tokens)
    if not common:
        return 0.0
    precision = len(common) / len(pred_tokens)
    recall = len(common) / len(gold_tokens)
    return 2 * precision * recall / (precision + recall)


# ---------------------------------------------------------------------------
# engram subprocess helpers
# ---------------------------------------------------------------------------

class EngramRunner:
    def __init__(self, binary: str, memory_dir: str, provider: str = "ollama"):
        self.binary = binary
        self.memory_dir = memory_dir
        self.provider = provider
        self.env = {**os.environ, "HOME": str(Path.home())}

    def run(self, *args, timeout: int = 60) -> tuple[int, str, str]:
        cmd = [self.binary, *args]
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout, env=self.env
        )
        return result.returncode, result.stdout, result.stderr

    def add_knowledge(self, project: str, category: str, content: str, label: str) -> bool:
        rc, _, err = self.run("add", project, category, content, "--label", label)
        if rc != 0:
            print(f"    [warn] engram add failed: {err.strip()[:80]}", file=sys.stderr)
        return rc == 0

    def embed(self, project: str) -> bool:
        rc, _, err = self.run("embed", project, "--provider", self.provider, timeout=120)
        if rc != 0:
            print(f"    [warn] embed failed: {err.strip()[:80]}", file=sys.stderr)
        return rc == 0

    def ask(self, project: str, question: str, threshold: float = 0.3) -> str:
        rc, stdout, _ = self.run(
            "ask", question, "--project", project,
            "--threshold", str(threshold),
            timeout=30,
        )
        if rc != 0 or "Not found in knowledge base" in stdout:
            return ""
        return stdout.strip()

    def forget(self, project: str):
        """Clean up project knowledge after eval."""
        self.run("forget", project, "--force")


# ---------------------------------------------------------------------------
# LoCoMo data loading
# ---------------------------------------------------------------------------

def load_locomo_dataset(max_convs: Optional[int] = None):
    """Load LoCoMo from HuggingFace datasets."""
    try:
        from datasets import load_dataset
    except ImportError:
        print("ERROR: Install requirements: pip install -r eval/requirements.txt", file=sys.stderr)
        sys.exit(1)

    print("Loading LoCoMo dataset from HuggingFace (snap-research/LoCoMo-dataset)...")
    ds = load_dataset("snap-research/LoCoMo-dataset", split="test")
    if max_convs:
        ds = ds.select(range(min(max_convs, len(ds))))
    print(f"  Loaded {len(ds)} conversations")
    return ds


def ingest_conversation(engram: EngramRunner, conv_id: str, conversation: dict) -> int:
    """
    Convert a LoCoMo conversation into engram knowledge entries.
    Groups turns by session and ingests each session as one 'solutions' entry.
    Returns the number of sessions ingested.
    """
    sessions = conversation.get("sessions", [])
    if not sessions:
        # Fallback: flat list of turns
        turns = conversation.get("turns", conversation.get("messages", []))
        text = " ".join(
            f"{t.get('speaker', t.get('role', 'user'))}: {t.get('text', t.get('content', ''))}"
            for t in turns
            if t.get("text") or t.get("content")
        )
        if text.strip():
            engram.add_knowledge(conv_id, "solutions", text[:3000], f"session-0")
        return 1

    count = 0
    for i, session in enumerate(sessions):
        turns = session if isinstance(session, list) else session.get("turns", [])
        text = " ".join(
            f"{t.get('speaker', t.get('role', 'user'))}: {t.get('text', t.get('content', ''))}"
            for t in turns
            if t.get("text") or t.get("content")
        )
        if text.strip():
            label = f"session-{i}"
            engram.add_knowledge(conv_id, "solutions", text[:3000], label)
            count += 1

    return count


def eval_conversation(
    engram: EngramRunner,
    conv_id: str,
    conversation: dict,
    use_graph: bool = False,
) -> dict[str, list[float]]:
    """
    Run evaluation for one conversation.
    Returns {question_type: [f1_scores]}.
    """
    project = f"locomo-eval-{conv_id}"
    scores: dict[str, list[float]] = defaultdict(list)

    try:
        # 1. Ingest conversation sessions
        n_sessions = ingest_conversation(engram, project, conversation)
        if n_sessions == 0:
            return scores

        # 2. Build embedding index
        engram.embed(project)

        # 3. Build graph if requested
        if use_graph:
            engram.run("graph", "build", project, timeout=120)

        # 4. Answer QA pairs
        qa_pairs = conversation.get("qa_pairs", conversation.get("questions", []))
        for qa in qa_pairs:
            question = qa.get("question", qa.get("q", ""))
            gold = qa.get("answer", qa.get("a", ""))
            qtype = qa.get("question_type", qa.get("type", "unknown"))

            if not question or not gold:
                continue

            prediction = engram.ask(project, question)
            score = f1_score(prediction, gold)
            scores[qtype].append(score)

    finally:
        # Clean up
        engram.forget(project)

    return scores


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------

BASELINES = {
    "GPT-4 (no memory)": {"overall": 32.1},
    "Mem0":              {"overall": 67.1, "strict": 51.1},
    "Human ceiling":     {"overall": 87.9},
}

QUESTION_TYPES = ["single_hop", "multi_hop", "temporal_reasoning", "adversarial"]


def print_report(all_scores: dict[str, list[float]], elapsed: float, n_convs: int):
    overall = [s for scores in all_scores.values() for s in scores]
    overall_f1 = sum(overall) / len(overall) * 100 if overall else 0.0

    print("\n" + "=" * 60)
    print(f"  engram LoCoMo Evaluation Results")
    print(f"  Conversations: {n_convs}  |  Total QA pairs: {len(overall)}")
    print(f"  Time: {elapsed:.1f}s")
    print("=" * 60)
    print(f"\n  Overall F1:  {overall_f1:.1f}")

    print("\n  By question type:")
    for qtype in QUESTION_TYPES:
        scores = all_scores.get(qtype, [])
        if scores:
            avg = sum(scores) / len(scores) * 100
            print(f"    {qtype:<25} {avg:5.1f}  (n={len(scores)})")

    unknown = all_scores.get("unknown", [])
    if unknown:
        avg = sum(unknown) / len(unknown) * 100
        print(f"    {'unknown':<25} {avg:5.1f}  (n={len(unknown)})")

    print("\n  Comparison:")
    print(f"    {'System':<30} {'F1':>6}")
    print(f"    {'-'*36}")
    for name, baseline in BASELINES.items():
        b_f1 = baseline["overall"]
        marker = " ◀ SOTA" if name == "Mem0" else ""
        print(f"    {name:<30} {b_f1:>5.1f}{marker}")
    marker = " ◀ engram" if overall_f1 > 0 else ""
    print(f"    {'engram (this run)':<30} {overall_f1:>5.1f}{marker}")

    if overall_f1 >= 67.1:
        print("\n  🎉 SOTA ACHIEVED: engram >= Mem0 (67.1 F1)")
    elif overall_f1 >= 32.1:
        delta = 67.1 - overall_f1
        print(f"\n  Gap to Mem0 SOTA: {delta:.1f} F1 points")
    print("=" * 60)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="LoCoMo benchmark eval for engram")
    parser.add_argument(
        "--engram",
        default="./target/release/engram",
        help="Path to engram binary (default: ./target/release/engram)",
    )
    parser.add_argument(
        "--provider",
        default="ollama",
        choices=["ollama", "gemini", "openai"],
        help="Embedding provider (default: ollama — free local)",
    )
    parser.add_argument(
        "--max-convs",
        type=int,
        default=None,
        help="Limit number of conversations (default: all ~25)",
    )
    parser.add_argument(
        "--workers",
        type=int,
        default=4,
        help="Parallel workers (default: 4)",
    )
    parser.add_argument(
        "--use-graph",
        action="store_true",
        help="Enable graph-augmented retrieval (Track 2)",
    )
    parser.add_argument(
        "--output",
        default="eval/results.json",
        help="Write raw scores to JSON file",
    )
    args = parser.parse_args()

    # Validate binary
    binary = str(Path(args.engram).resolve())
    if not Path(binary).exists():
        print(f"ERROR: engram binary not found at {binary}", file=sys.stderr)
        print("Build first: cargo build --release", file=sys.stderr)
        sys.exit(1)

    memory_dir = str(Path.home() / "memory")
    engram = EngramRunner(binary, memory_dir, args.provider)

    # Load dataset
    dataset = load_locomo_dataset(args.max_convs)

    # Run eval
    all_scores: dict[str, list[float]] = defaultdict(list)
    start = time.time()

    print(f"\nRunning eval: {len(dataset)} conversations, {args.workers} workers")
    print(f"  Provider: {args.provider}  |  Graph: {args.use_graph}")
    print()

    def process_conv(item):
        conv_id = str(item.get("conversation_id", item.get("id", hash(str(item)))))
        print(f"  [{conv_id[:12]}] ingesting...", flush=True)
        scores = eval_conversation(engram, conv_id, item, use_graph=args.use_graph)
        n_qa = sum(len(v) for v in scores.values())
        overall = sum(s for v in scores.values() for s in v)
        f1 = overall / n_qa * 100 if n_qa else 0.0
        print(f"  [{conv_id[:12]}] done — {n_qa} QA pairs, F1={f1:.1f}", flush=True)
        return scores

    with ThreadPoolExecutor(max_workers=args.workers) as executor:
        futures = {executor.submit(process_conv, item): i for i, item in enumerate(dataset)}
        for future in as_completed(futures):
            try:
                scores = future.result()
                for qtype, vals in scores.items():
                    all_scores[qtype].extend(vals)
            except Exception as e:
                print(f"  [error] conversation failed: {e}", file=sys.stderr)

    elapsed = time.time() - start

    # Save raw scores
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w") as f:
        json.dump({"scores": dict(all_scores), "elapsed": elapsed, "n_convs": len(dataset)}, f, indent=2)
    print(f"\nRaw scores saved to {args.output}")

    # Print report
    print_report(all_scores, elapsed, len(dataset))


if __name__ == "__main__":
    main()
