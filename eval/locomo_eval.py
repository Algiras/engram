#!/usr/bin/env python3
"""
LoCoMo Benchmark Evaluation for engram — v3
=============================================
Key improvements vs v2:
  - gemini-2.5-pro for extraction and QA (vs Flash)
  - Atomic facts with entity tagging: [Person: X][Date: Y] fact text
  - LLM-as-a-Judge metric (matches Mem0's headline 67.1 metric)
  - Per-fact engram add (atomic, not grouped) + Update Resolver dedup

Correct metric comparison (from Mem0 paper):
    Token-overlap F1:   Mem0=38.72   GPT-4-no-mem=32.1   Human=87.9
    LLM-as-a-Judge:     Mem0=67.13   GPT-4-no-mem≈18      Human ceiling
    ← We measure BOTH

engram history:
    v1 raw add, verbose:           18.0 F1 token  (2026-02-19)
    v2 fact extract + concise:     ~26  F1 token  (2026-02-20, full run pending)
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
import unicodedata
import uuid
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Optional
import urllib.request

# ---------------------------------------------------------------------------
# Scoring
# ---------------------------------------------------------------------------

def normalize_answer(s) -> str:
    s = str(s).replace(",", "")
    s = unicodedata.normalize("NFD", s)
    s = re.sub(r"\b(a|an|the|and)\b", " ", s.lower())
    s = re.sub(r"[^a-z0-9 ]", " ", s)
    return " ".join(s.split())


def token_f1(prediction: str, gold) -> float:
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


CATEGORY_NAMES = {
    1: "single_hop",
    2: "temporal",
    3: "open_ended",
    4: "multi_hop",
    5: "multi_hop_cs",
}

# ---------------------------------------------------------------------------
# Gemini API helper
# ---------------------------------------------------------------------------

def gemini_call(prompt: str, api_key: str, model: str = "gemini-2.5-pro",
                max_tokens: int = 2048, temperature: float = 0.1) -> str:
    """Direct Gemini API call — bypasses engram for higher-quality extraction & judging.
    Handles both standard and thinking (2.5-pro) response formats.
    """
    # Thinking models need enough tokens for reasoning + output
    effective_max = max(max_tokens, 500) if "2.5-pro" in model else max_tokens

    payload = json.dumps({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {"temperature": temperature, "maxOutputTokens": effective_max},
    }).encode()
    url = (
        f"https://generativelanguage.googleapis.com/v1beta/models/"
        f"{model}:generateContent?key={api_key}"
    )
    req = urllib.request.Request(
        url, data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            result = json.loads(resp.read())
        candidate = result["candidates"][0]
        content = candidate.get("content", {})
        parts = content.get("parts", [])
        # Thinking models may return parts with role but no text if tokens ran out
        for part in parts:
            if "text" in part and part["text"].strip():
                return part["text"].strip()
        return ""
    except Exception as e:
        return f"[error: {e}]"


# ---------------------------------------------------------------------------
# LLM-as-a-Judge
# ---------------------------------------------------------------------------

JUDGE_PROMPT = """You are evaluating a QA system answer.

Question: {question}
Gold answer: {gold}
System answer: {prediction}

Is the system answer correct or semantically equivalent to the gold answer?
Answer with exactly one word: YES or NO"""


def llm_judge(question: str, gold, prediction: str,
              api_key: str, model: str) -> float:
    """Returns 1.0 if judge says YES, 0.0 if NO."""
    if not prediction or "Not found in knowledge base" in prediction:
        return 0.0
    response = gemini_call(
        JUDGE_PROMPT.format(
            question=question,
            gold=str(gold),
            prediction=prediction[:200],
        ),
        api_key=api_key,
        model=model,
        max_tokens=5,
        temperature=0.0,
    )
    return 1.0 if response.strip().upper().startswith("YES") else 0.0


# ---------------------------------------------------------------------------
# Fact extraction (gemini-2.5-pro)
# ---------------------------------------------------------------------------

FACT_PROMPT = """Extract ALL factual statements from this conversation as a numbered list.

Rules:
- Include: names, dates, events, hobbies, emotions, locations, relationships, goals, achievements
- For EACH fact: identify the person it's about and include the exact date if mentioned
- Format: "[Person: NAME][Date: DATE if known] factual statement"
  Example: "[Person: Caroline][Date: 7 May 2023] Caroline went to an LGBTQ support group."
  Example: "[Person: Melanie] Melanie is working on a charity race for cancer awareness."
- Include facts about BOTH speakers. Be specific, not vague.
- Maximum 40 facts. One fact per line.

CONVERSATION:
{text}

Facts:"""

ENTITY_PATTERN = re.compile(r"\[Person:\s*([^\]]+)\]")
DATE_PATTERN = re.compile(r"\[Date:\s*([^\]]+)\]")


def extract_facts(text: str, api_key: str, model: str) -> list[str]:
    """Extract atomic entity-tagged facts from a session."""
    response = gemini_call(FACT_PROMPT.format(text=text[:6000]),
                           api_key=api_key, model=model, max_tokens=2048)
    facts = []
    for line in response.splitlines():
        line = re.sub(r"^\d+\.\s*", "", line).strip()
        if len(line) > 15 and not line.startswith("[error"):
            facts.append(line)
    return facts


# ---------------------------------------------------------------------------
# engram subprocess driver
# ---------------------------------------------------------------------------

class EngramRunner:
    def __init__(self, binary: str, embed_provider: str = "gemini",
                 qa_model: str = "gemini-2.5-pro"):
        self.binary = binary
        self.embed_provider = embed_provider
        self.qa_model = qa_model
        # Set env so engram ask uses the better model
        self.env = {
            **os.environ,
            "ENGRAM_LLM_MODEL": qa_model,
        }

    def _run(self, *args, timeout: int = 60) -> tuple[int, str, str]:
        result = subprocess.run(
            [self.binary, *args],
            capture_output=True, text=True, timeout=timeout,
            env=self.env,
        )
        return result.returncode, result.stdout, result.stderr

    def add(self, project: str, category: str, content: str, label: str) -> bool:
        rc, _, err = self._run("add", project, category, content, "--label", label)
        if rc != 0:
            print(f"    [warn] add: {err.strip()[:80]}", file=sys.stderr)
        return rc == 0

    def embed(self, project: str) -> bool:
        rc, _, err = self._run(
            "embed", project, "--provider", self.embed_provider, timeout=180
        )
        if rc != 0:
            print(f"    [warn] embed: {err.strip()[:80]}", file=sys.stderr)
        return rc == 0

    def graph_build(self, project: str) -> bool:
        rc, _, _ = self._run("graph", "build", project, timeout=120)
        return rc == 0

    def ask(self, project: str, question: str,
            threshold: float = 0.1, top_k: int = 10,
            use_graph: bool = False) -> str:
        args = [
            "ask", question,
            "--project", project,
            "--threshold", str(threshold),
            "--top-k", str(top_k),
            "--concise",
        ]
        if use_graph:
            args.append("--use-graph")
        rc, stdout, _ = self._run(*args, timeout=120)
        if rc != 0 or "Not found in knowledge base" in stdout:
            return ""
        lines = [l for l in stdout.strip().splitlines()
                 if not l.startswith("Sources:") and not l.startswith("Hint:")]
        return "\n".join(lines).strip()

    def forget(self, project: str):
        self._run("forget", project, "--force", timeout=10)


# ---------------------------------------------------------------------------
# Ingestion strategies
# ---------------------------------------------------------------------------

def ingest_atomic_facts(engram: EngramRunner, project: str, conv: dict,
                        api_key: str, model: str) -> int:
    """
    v3 strategy: atomic facts with entity+date tags, stored individually.
    Entity tags ([Person: X][Date: Y]) anchor embeddings for precise retrieval.
    The Update Resolver in engram deduplicates cross-session redundancy.
    """
    total = 0
    for i in range(1, 50):
        key = f"session_{i}"
        if key not in conv["conversation"] or not conv["conversation"][key]:
            break
        turns = conv["conversation"][key]
        date_key = f"session_{i}_date_time"
        date_str = conv["conversation"].get(date_key, "")

        date_prefix = f"[Session date: {date_str}]\n" if date_str else ""
        raw_text = date_prefix + "\n".join(
            f"{t['speaker']}: {t['text']}" for t in turns if t.get("text")
        )

        facts = extract_facts(raw_text, api_key, model)
        if not facts:
            # Fallback: store raw session with date
            engram.add(project, "solutions", raw_text[:4000], f"session-{i}-raw")
            total += 1
            continue

        # Store each fact atomically — entity tags improve embedding precision
        for j, fact in enumerate(facts):
            label = f"s{i}f{j}"
            # Route by content: temporal facts → decisions, rest → solutions
            has_date = bool(DATE_PATTERN.search(fact)) or any(
                kw in fact.lower() for kw in [" on ", " in 20", " in 19", "date:"]
            )
            category = "decisions" if has_date else "solutions"
            engram.add(project, category, fact, label)
            total += 1

    return total


def ingest_raw_with_dates(engram: EngramRunner, project: str, conv: dict) -> int:
    """Baseline: raw session text with session dates prepended."""
    count = 0
    for i in range(1, 50):
        key = f"session_{i}"
        if key not in conv["conversation"] or not conv["conversation"][key]:
            break
        turns = conv["conversation"][key]
        date_str = conv["conversation"].get(f"session_{i}_date_time", "")
        date_prefix = f"[Session date: {date_str}]\n" if date_str else ""
        text = date_prefix + "\n".join(
            f"{t['speaker']}: {t['text']}" for t in turns if t.get("text")
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
    strategy: str = "atomic",   # "atomic" | "raw"
    use_graph: bool = False,
    threshold: float = 0.1,
    top_k: int = 10,
    api_key: str = "",
    extract_model: str = "gemini-2.5-pro",
    use_judge: bool = False,
    judge_model: str = "gemini-2.5-pro",
) -> dict:
    """
    Returns {
        "f1": {category_int: [scores]},
        "judge": {category_int: [scores]},   # only if use_judge=True
    }
    """
    sample_id = conv["sample_id"]
    project = f"locomo-{sample_id}"
    f1_scores: dict[int, list[float]] = defaultdict(list)
    judge_scores: dict[int, list[float]] = defaultdict(list)

    try:
        if strategy == "atomic":
            ingest_atomic_facts(engram, project, conv, api_key, extract_model)
        else:
            ingest_raw_with_dates(engram, project, conv)

        engram.embed(project)

        if use_graph:
            engram.graph_build(project)

        for qa in conv["qa"]:
            question = qa.get("question", "")
            gold = qa.get("answer", "")
            category = qa.get("category", 0)
            if not question or not gold:
                continue

            prediction = engram.ask(
                project, question,
                threshold=threshold, top_k=top_k, use_graph=use_graph,
            )

            f1_scores[category].append(token_f1(prediction, gold))

            if use_judge and api_key:
                judge_scores[category].append(
                    llm_judge(question, gold, prediction, api_key, judge_model)
                )

    finally:
        engram.forget(project)

    return {"f1": f1_scores, "judge": judge_scores}


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------

V1_BASELINE = 18.0
BASELINES_F1 = {
    "GPT-4 (no memory)": 32.1,
    "Mem0 (token F1)":   38.72,
}
BASELINES_JUDGE = {
    "Mem0 (LLM-judge)":  67.13,
    "Human ceiling":     87.9,
}


def _avg(scores: list[float]) -> float:
    return sum(scores) / len(scores) * 100 if scores else 0.0


def print_report(f1_all: dict, judge_all: dict, elapsed: float,
                 n_convs: int, label: str):
    flat_f1 = [s for v in f1_all.values() for s in v]
    flat_judge = [s for v in judge_all.values() for s in v]
    overall_f1 = _avg(flat_f1)
    overall_judge = _avg(flat_judge)
    delta = overall_f1 - V1_BASELINE

    print()
    print("╔══════════════════════════════════════════════════════╗")
    print(f"  engram LoCoMo-10 v3  |  {label}")
    print(f"  {n_convs} convs  |  {len(flat_f1)} QA pairs  |  {elapsed:.0f}s")
    print("╠══════════════════════════════════════════════════════╣")
    sign = "+" if delta >= 0 else ""
    print(f"  Token-F1:       {overall_f1:.1f}  ({sign}{delta:.1f} vs v1)")
    if flat_judge:
        print(f"  LLM-as-Judge:   {overall_judge:.1f}")
    print()
    print("  By category (Token-F1):")
    for cat_id in sorted(CATEGORY_NAMES):
        vals = f1_all.get(cat_id, [])
        if vals:
            avg = _avg(vals)
            j_avg = _avg(judge_all.get(cat_id, []))
            j_str = f"  judge={j_avg:.1f}" if flat_judge else ""
            bar = "█" * max(1, int(avg / 4))
            print(f"    {CATEGORY_NAMES[cat_id]:<16} {avg:5.1f}  {bar}{j_str}")

    print()
    print("  Token-F1 comparison:")
    print(f"    {'engram v1 (raw add)':<28} {V1_BASELINE:5.1f}")
    for name, b in BASELINES_F1.items():
        marker = " ◀ Mem0 F1" if "Mem0" in name else ""
        print(f"    {name:<28} {b:5.1f}{marker}")
    marker = " ✓ beats GPT-4!" if overall_f1 >= 32.1 else f" (gap to GPT-4: {32.1 - overall_f1:.1f})"
    print(f"    {'engram (this run)':<28} {overall_f1:5.1f}{marker}")

    if flat_judge:
        print()
        print("  LLM-judge comparison:")
        for name, b in BASELINES_JUDGE.items():
            marker = " ◀ SOTA" if "Mem0" in name else ""
            print(f"    {name:<28} {b:5.1f}{marker}")
        marker = " ✓ SOTA!" if overall_judge >= 67.13 else f" (gap: {67.13 - overall_judge:.1f})"
        print(f"    {'engram (this run)':<28} {overall_judge:5.1f}{marker}")

    print("╚══════════════════════════════════════════════════════╝")
    if overall_f1 >= 38.72:
        print("\n  🎉 Beats Mem0 token-F1 (38.72)!")
    elif overall_f1 >= 32.1:
        print(f"\n  ✓  Beats GPT-4 no-memory baseline!")
    if flat_judge and overall_judge >= 67.13:
        print("  🎉 Beats Mem0 LLM-judge (67.13)!")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser(description="LoCoMo-10 v3 benchmark for engram")
    ap.add_argument("--engram", default="./target/release/engram")
    ap.add_argument("--data", default="eval/locomo10.json")
    ap.add_argument("--embed-provider", default="gemini",
                    choices=["gemini", "openai", "ollama"])
    ap.add_argument("--extract-model", default="gemini-2.5-pro",
                    help="Gemini model for fact extraction (default: gemini-2.5-pro)")
    ap.add_argument("--qa-model", default="gemini-2.5-flash",
                    help="Model for engram ask synthesis (default: gemini-2.5-flash)")
    ap.add_argument("--judge-model", default="gemini-2.5-flash",
                    help="Model for LLM-as-a-Judge (default: gemini-2.5-flash — fast)")
    ap.add_argument("--strategy", default="atomic",
                    choices=["atomic", "raw"],
                    help="atomic=entity-tagged facts (v3), raw=session text (v1)")
    ap.add_argument("--use-graph", action="store_true")
    ap.add_argument("--use-judge", action="store_true",
                    help="Add LLM-as-a-Judge metric (matches Mem0's 67.1 headline)")
    ap.add_argument("--threshold", type=float, default=0.1)
    ap.add_argument("--top-k", type=int, default=10)
    ap.add_argument("--max-convs", type=int, default=None)
    ap.add_argument("--workers", type=int, default=3)
    ap.add_argument("--output", default="eval/results_v3.json")
    args = ap.parse_args()

    binary = str(Path(args.engram).resolve())
    if not Path(binary).exists():
        print(f"ERROR: {binary} not found. Run: cargo build --release", file=sys.stderr)
        sys.exit(1)

    api_key = os.environ.get("GEMINI_API_KEY", "")
    if not api_key:
        print("ERROR: GEMINI_API_KEY not set", file=sys.stderr)
        sys.exit(1)

    with open(args.data) as f:
        dataset = json.load(f)
    if args.max_convs:
        dataset = dataset[:args.max_convs]

    engram = EngramRunner(binary, args.embed_provider, args.qa_model)

    label_parts = [
        f"strategy={args.strategy}",
        f"extract={args.extract_model.replace('gemini-', '')}",
        f"qa={args.qa_model.replace('gemini-', '')}",
        f"t={args.threshold} k={args.top_k}",
    ]
    if args.use_graph:
        label_parts.append("graph")
    if args.use_judge:
        label_parts.append("judge")
    label = " | ".join(label_parts)

    print(f"engram LoCoMo v3")
    print(f"  Binary:   {Path(binary).name}")
    print(f"  Strategy: {label}")
    print(f"  Dataset:  {len(dataset)} convs, "
          f"{sum(len(d['qa']) for d in dataset)} QA pairs")
    print(f"  Workers:  {args.workers}")
    print()

    f1_all: dict[int, list[float]] = defaultdict(list)
    judge_all: dict[int, list[float]] = defaultdict(list)
    start = time.time()

    def process(conv):
        sid = conv["sample_id"]
        n = len(conv["qa"])
        print(f"  [{sid}] starting ({n} QA)...", flush=True)
        result = eval_conversation(
            engram, conv,
            strategy=args.strategy,
            use_graph=args.use_graph,
            threshold=args.threshold,
            top_k=args.top_k,
            api_key=api_key,
            extract_model=args.extract_model,
            use_judge=args.use_judge,
            judge_model=args.judge_model,
        )
        flat = [s for v in result["f1"].values() for s in v]
        f1 = _avg(flat)
        j_flat = [s for v in result["judge"].values() for s in v]
        j_str = f"  judge={_avg(j_flat):.1f}" if j_flat else ""
        print(f"  [{sid}] done — F1={f1:.1f} ({len(flat)} pairs){j_str}", flush=True)
        return result

    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        for result in pool.map(process, dataset):
            for cat, vals in result["f1"].items():
                f1_all[cat].extend(vals)
            for cat, vals in result["judge"].items():
                judge_all[cat].extend(vals)

    elapsed = time.time() - start

    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w") as f:
        json.dump({
            "f1_scores": {str(k): v for k, v in f1_all.items()},
            "judge_scores": {str(k): v for k, v in judge_all.items()},
            "elapsed": elapsed,
            "n_convs": len(dataset),
            "label": label,
            "args": vars(args),
        }, f, indent=2)
    print(f"\nScores → {args.output}")

    print_report(f1_all, judge_all, elapsed, len(dataset), label)


if __name__ == "__main__":
    main()
