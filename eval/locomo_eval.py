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

    # With LLM extraction (recommended — +15-25 F1 expected)
    python eval/locomo_eval.py --provider gemini --use-llm-ingest

    # With graph-augmented retrieval
    python eval/locomo_eval.py --provider gemini --use-llm-ingest --use-graph

Data source: eval/locomo10.json (official LoCoMo-10 from snap-research/locomo)

Category mapping (from the paper):
    1 = single_hop    (factual recall from one session)
    2 = temporal      (time/date-based)
    3 = open_ended    (adversarial / requires inference)
    4 = multi_hop     (cross-session reasoning, within speaker)
    5 = multi_hop_cs  (cross-session, cross-speaker)

Baselines (token-overlap F1 from Mem0 paper):
    GPT-4 (no memory):  32.1
    Mem0:               67.1
    Human ceiling:      87.9

Engram baseline (raw add):  18.0  (v1 run, 2026-02-19)
"""

import argparse
import json
import os
import re
import string
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
# Official LoCoMo F1 scoring
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

    def _run(self, *args, timeout: int = 60) -> tuple[int, str, str]:
        result = subprocess.run(
            [self.binary, *args],
            capture_output=True, text=True, timeout=timeout,
        )
        return result.returncode, result.stdout, result.stderr

    def add(self, project: str, category: str, content: str, label: str) -> bool:
        rc, _, err = self._run("add", project, category, content, "--label", label)
        if rc != 0:
            print(f"    [warn] add failed: {err.strip()[:80]}", file=sys.stderr)
        return rc == 0

    def ingest(self, project: str, force: bool = True) -> bool:
        args = ["ingest", "--project", project, "--provider", self.provider]
        if force:
            args.append("--force")
        rc, out, err = self._run(*args, timeout=600)
        if rc != 0:
            print(f"    [warn] ingest failed: {err.strip()[:100]}", file=sys.stderr)
        return rc == 0

    def embed(self, project: str) -> bool:
        rc, _, err = self._run("embed", project, "--provider", self.provider, timeout=180)
        if rc != 0:
            print(f"    [warn] embed failed: {err.strip()[:80]}", file=sys.stderr)
        return rc == 0

    def graph_build(self, project: str) -> bool:
        rc, _, _ = self._run("graph", "build", project, timeout=120)
        return rc == 0

    def ask(self, project: str, question: str,
            threshold: float = 0.2, top_k: int = 8,
            use_graph: bool = False, concise: bool = True) -> str:
        args = [
            "ask", question,
            "--project", project,
            "--threshold", str(threshold),
            "--top-k", str(top_k),
        ]
        if use_graph:
            args.append("--use-graph")
        if concise:
            args.append("--concise")
        rc, stdout, _ = self._run(*args, timeout=30)
        if rc != 0 or "Not found in knowledge base" in stdout:
            return ""
        lines = [l for l in stdout.strip().splitlines()
                 if not l.startswith("Sources:") and not l.startswith("Hint:")]
        return "\n".join(lines).strip()

    def forget(self, project: str):
        self._run("forget", project, "--force", timeout=10)


# ---------------------------------------------------------------------------
# Improvement A: LLM-extracted ingestion via synthetic Claude JSONL
# ---------------------------------------------------------------------------

def build_synthetic_jsonl(conv: dict) -> list[dict]:
    """
    Convert a LoCoMo conversation into Claude-format JSONL entries.
    Each session becomes one conversation turn-pair (user asks, assistant recaps).
    Improvement B: prepend session date to each turn so LLM captures temporal facts.
    """
    entries = []
    session_num = 1

    while True:
        key = f"session_{session_num}"
        if key not in conv["conversation"] or not conv["conversation"][key]:
            break

        turns = conv["conversation"][key]
        date_key = f"session_{session_num}_date_time"
        date_str = conv["conversation"].get(date_key, "")

        # Build turn text with date context (Improvement B)
        date_prefix = f"[Date: {date_str}]\n" if date_str else ""
        turn_text = date_prefix + "\n".join(
            f"{t['speaker']}: {t['text']}"
            for t in turns if t.get("text")
        )

        if not turn_text.strip():
            session_num += 1
            continue

        session_id = str(uuid.uuid4())
        ts = f"2024-01-{session_num:02d}T10:00:00.000Z"

        # User turn: the raw conversation
        entries.append({
            "type": "user",
            "uuid": str(uuid.uuid4()),
            "parentUuid": None,
            "sessionId": session_id,
            "timestamp": ts,
            "isSidechain": False,
            "cwd": "/tmp",
            "message": {
                "role": "user",
                "content": turn_text[:6000],
            },
        })

        # Assistant turn: brief recap to help LLM extract facts
        # Including both speakers' names helps cross-speaker multi-hop
        speakers = list({t["speaker"] for t in turns if t.get("speaker")})
        speaker_note = f"Participants: {', '.join(speakers)}. " if speakers else ""
        recap = f"{speaker_note}Above is session {session_num} of a long-term conversation."
        if date_str:
            recap += f" This session took place on {date_str}."

        entries.append({
            "type": "assistant",
            "uuid": str(uuid.uuid4()),
            "parentUuid": None,
            "sessionId": session_id,
            "timestamp": ts,
            "isSidechain": False,
            "message": {
                "role": "assistant",
                "content": recap,
                "model": "claude-sonnet-4-6",
                "usage": {"input_tokens": 100, "output_tokens": 20},
            },
        })

        session_num += 1

    return entries


def write_synthetic_project(project: str, entries: list[dict]) -> Path:
    """Write synthetic JSONL to ~/.claude/projects/<project>/."""
    home = Path.home()
    project_dir = home / ".claude" / "projects" / f"-locomo-{project}"
    project_dir.mkdir(parents=True, exist_ok=True)

    jsonl_path = project_dir / "locomo.jsonl"
    with open(jsonl_path, "w") as f:
        for entry in entries:
            f.write(json.dumps(entry) + "\n")

    return project_dir


def cleanup_synthetic_project(project: str):
    """Remove the synthetic JSONL directory."""
    home = Path.home()
    project_dir = home / ".claude" / "projects" / f"-locomo-{project}"
    if project_dir.exists():
        import shutil
        shutil.rmtree(project_dir, ignore_errors=True)


# ---------------------------------------------------------------------------
# Improvement A2: Direct fact extraction via Gemini API
# ---------------------------------------------------------------------------

FACT_EXTRACTION_PROMPT = """Extract ALL factual statements from this conversation as a numbered list.

Rules:
- Include: names, events, dates, preferences, relationships, locations, achievements
- For each fact include the person it's about and the date/time if mentioned
- Be precise: "Caroline went to the LGBTQ support group on 7 May 2023" not "Caroline went somewhere"
- Include facts from BOTH speakers
- Maximum 30 facts. If fewer are present, extract fewer.
- Format each fact as one sentence.

CONVERSATION:
{text}

Facts:"""


def call_gemini_extract(session_text: str, api_key: str) -> list[str]:
    """Call Gemini directly to extract personal facts with dates."""
    prompt = FACT_EXTRACTION_PROMPT.format(text=session_text[:6000])

    payload = json.dumps({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {"temperature": 0.1, "maxOutputTokens": 1024},
    }).encode()

    url = (
        f"https://generativelanguage.googleapis.com/v1beta/models/"
        f"gemini-2.0-flash:generateContent?key={api_key}"
    )
    req = urllib.request.Request(
        url, data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            result = json.loads(resp.read())
        text = result["candidates"][0]["content"]["parts"][0]["text"]
        # Parse numbered/bulleted list
        facts = []
        for line in text.splitlines():
            line = re.sub(r"^[\d\.\-\*\•]+\s*", "", line).strip()
            if len(line) > 10:
                facts.append(line)
        return facts
    except Exception as e:
        print(f"    [warn] gemini fact extract: {e}", file=sys.stderr)
        return []


def ingest_facts(engram: EngramRunner, project: str, conv: dict, api_key: str) -> int:
    """
    Improvement A2: extract personal facts per session via Gemini, store as grouped blocks.
    Groups all facts from one session into one entry — better embedding density than atomic facts,
    while still capturing dates, names, events precisely.
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

        facts = call_gemini_extract(raw_text, api_key)
        if not facts:
            # Fallback: store raw text with date prefix
            engram.add(project, "solutions", raw_text[:4000], f"session-{i}-raw")
            total += 1
            continue

        # Store all facts from this session as ONE grouped entry (better embedding density)
        date_header = f"Session date: {date_str}\n\n" if date_str else ""
        grouped = date_header + "\n".join(f"- {f}" for f in facts)
        engram.add(project, "solutions", grouped[:4000], f"session-{i}-facts")
        total += 1

    return total


# ---------------------------------------------------------------------------
# Raw ingestion (baseline strategy)
# ---------------------------------------------------------------------------

def ingest_raw(engram: EngramRunner, project: str, conv: dict) -> int:
    """Original strategy: dump raw session text via engram add."""
    count = 0
    for i in range(1, 50):
        key = f"session_{i}"
        if key not in conv["conversation"] or not conv["conversation"][key]:
            break
        turns = conv["conversation"][key]
        date_key = f"session_{i}_date_time"
        date_str = conv["conversation"].get(date_key, "")
        date_prefix = f"[Date: {date_str}]\n" if date_str else ""  # Improvement B
        text = date_prefix + "\n".join(
            f"{t['speaker']}: {t['text']}"
            for t in turns if t.get("text")
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
    use_llm_ingest: bool = False,
    use_fact_extract: bool = False,
    use_graph: bool = False,
    threshold: float = 0.2,
    top_k: int = 8,
    gemini_api_key: str = "",
) -> dict[int, list[float]]:
    """Returns {category_int: [f1_scores]}."""
    sample_id = conv["sample_id"]
    project = f"locomo-{sample_id}"
    synth_project_key = sample_id if use_llm_ingest else None

    scores: dict[int, list[float]] = defaultdict(list)

    try:
        if use_fact_extract and gemini_api_key:
            # Improvement A2: direct Gemini fact extraction → atomic facts in engram
            ingest_facts(engram, project, conv, gemini_api_key)
        elif use_llm_ingest:
            # Improvement A: synthetic JSONL → engram ingest → LLM extraction
            entries = build_synthetic_jsonl(conv)
            write_synthetic_project(sample_id, entries)
            success = engram.ingest(project)
            if not success:
                ingest_raw(engram, project, conv)
        else:
            # Baseline: raw session text
            ingest_raw(engram, project, conv)

        # Build embedding index (Improvement C: increased top-k used at query time)
        engram.embed(project)

        if use_graph:
            engram.graph_build(project)

        # Answer QA pairs
        for qa in conv["qa"]:
            question = qa.get("question", "")
            gold = qa.get("answer", "")
            category = qa.get("category", 0)
            if not question or not gold:
                continue

            prediction = engram.ask(
                project, question,
                threshold=threshold, top_k=top_k,
                use_graph=use_graph,
            )
            scores[category].append(token_f1(prediction, gold))

    finally:
        engram.forget(project)
        if use_llm_ingest and synth_project_key:
            cleanup_synthetic_project(synth_project_key)

    return scores


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------

BASELINES = {
    "GPT-4 (no memory)": 32.1,
    "Mem0 (SOTA)":        67.1,
    "Human ceiling":      87.9,
}
ENGRAM_V1 = 18.0  # baseline from raw-add run


def print_report(all_scores: dict[int, list[float]], elapsed: float,
                 n_convs: int, label: str, prev: float = ENGRAM_V1):
    flat = [s for vals in all_scores.values() for s in vals]
    overall = sum(flat) / len(flat) * 100 if flat else 0.0
    delta = overall - prev

    print()
    print("╔══════════════════════════════════════════════════════╗")
    print(f"  engram LoCoMo-10  |  {label}")
    print(f"  Conversations: {n_convs}  |  QA pairs: {len(flat)}  |  {elapsed:.0f}s")
    print("╠══════════════════════════════════════════════════════╣")
    sign = "+" if delta >= 0 else ""
    print(f"  Overall F1:  {overall:.1f}  ({sign}{delta:.1f} vs v1 baseline)")
    print()
    print("  By category:")
    for cat_id in sorted(CATEGORY_NAMES):
        vals = all_scores.get(cat_id, [])
        if vals:
            avg = sum(vals) / len(vals) * 100
            bar = "█" * max(1, int(avg / 4))
            print(f"    {CATEGORY_NAMES[cat_id]:<16} {avg:5.1f}  {bar}")
    print()
    print("  Comparison:")
    print(f"    {'engram v1 (raw add)':<28} {ENGRAM_V1:5.1f}")
    for name, b in BASELINES.items():
        marker = " ◀ SOTA" if "Mem0" in name else ""
        print(f"    {name:<28} {b:5.1f}{marker}")
    marker = " ◀ engram ✓" if overall >= 67.1 else f" (+{overall-ENGRAM_V1:.1f} vs v1)"
    print(f"    {'engram (this run)':<28} {overall:5.1f}{marker}")
    print("╚══════════════════════════════════════════════════════╝")

    if overall >= 67.1:
        print("\n  🎉  SOTA ACHIEVED: engram ≥ Mem0!")
    elif overall >= 32.1:
        print(f"\n  ✓  Beats GPT-4 no-memory baseline!")
    print(f"\n  Gap to Mem0: {67.1 - overall:.1f} F1 points")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser(description="LoCoMo-10 benchmark for engram")
    ap.add_argument("--engram", default="./target/release/engram")
    ap.add_argument("--data", default="eval/locomo10.json")
    ap.add_argument("--provider", default="gemini",
                    choices=["gemini", "openai", "ollama"])
    ap.add_argument("--max-convs", type=int, default=None)
    ap.add_argument("--workers", type=int, default=3)
    ap.add_argument("--use-llm-ingest", action="store_true",
                    help="Improvement A: use engram ingest (LLM extraction) instead of raw add")
    ap.add_argument("--use-fact-extract", action="store_true",
                    help="Improvement A2: use Gemini to extract atomic dated facts (requires GEMINI_API_KEY)")
    ap.add_argument("--use-graph", action="store_true",
                    help="Improvement: graph-augmented retrieval")
    ap.add_argument("--threshold", type=float, default=None,
                    help="Retrieval similarity threshold (default: 0.1 for fact-extract, 0.2 otherwise)")
    ap.add_argument("--top-k", type=int, default=10,
                    help="Retrieval top-k (default: 10)")
    ap.add_argument("--output", default="eval/results_v2.json")
    args = ap.parse_args()

    binary = str(Path(args.engram).resolve())
    if not Path(binary).exists():
        print(f"ERROR: {binary} not found. Run: cargo build --release", file=sys.stderr)
        sys.exit(1)

    with open(args.data) as f:
        dataset = json.load(f)
    if args.max_convs:
        dataset = dataset[:args.max_convs]

    engram = EngramRunner(binary, args.provider)
    gemini_key = os.environ.get("GEMINI_API_KEY", "")

    # Auto-select threshold: fact-extract needs lower threshold (more atomic entries)
    threshold = args.threshold
    if threshold is None:
        threshold = 0.1 if args.use_fact_extract else 0.2

    label_parts = []
    if args.use_fact_extract:
        label_parts.append("Fact extract")
    elif args.use_llm_ingest:
        label_parts.append("LLM ingest")
    if args.use_graph:
        label_parts.append("+ Graph")
    label_parts.append(f"t={threshold} k={args.top_k}")
    label = " | ".join(label_parts) if label_parts else f"raw add | t={threshold} k={args.top_k}"

    print(f"engram LoCoMo eval v2")
    print(f"  Binary:      {Path(binary).name}")
    print(f"  Provider:    {args.provider}")
    print(f"  Dataset:     {len(dataset)} conversations, "
          f"{sum(len(d['qa']) for d in dataset)} QA pairs")
    print(f"  Strategy:    {label}")
    print(f"  Workers:     {args.workers}")
    print()

    def process(conv):
        sid = conv["sample_id"]
        n_qa = len(conv["qa"])
        print(f"  [{sid}] starting ({n_qa} QA pairs)...", flush=True)
        scores = eval_conversation(
            engram, conv,
            use_llm_ingest=args.use_llm_ingest,
            use_fact_extract=args.use_fact_extract,
            use_graph=args.use_graph,
            threshold=threshold,
            top_k=args.top_k,
            gemini_api_key=gemini_key,
        )
        n = sum(len(v) for v in scores.values())
        f1 = sum(s for v in scores.values() for s in v) / n * 100 if n else 0.0
        print(f"  [{sid}] done — F1={f1:.1f} ({n} pairs)", flush=True)
        return scores

    all_scores: dict[int, list[float]] = defaultdict(list)
    start = time.time()

    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        for scores in pool.map(process, dataset):
            for cat, vals in scores.items():
                all_scores[cat].extend(vals)

    elapsed = time.time() - start

    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w") as f:
        json.dump({
            "scores": {str(k): v for k, v in all_scores.items()},
            "elapsed": elapsed,
            "n_convs": len(dataset),
            "label": label,
            "provider": args.provider,
            "use_llm_ingest": args.use_llm_ingest,
            "use_graph": args.use_graph,
            "threshold": args.threshold,
            "top_k": args.top_k,
        }, f, indent=2)
    print(f"\nRaw scores → {args.output}")

    print_report(all_scores, elapsed, len(dataset), label)


if __name__ == "__main__":
    main()
