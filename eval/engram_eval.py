#!/usr/bin/env python3
"""
engram Domain Eval — the eval that actually measures engram improvement.

Modes:
  Default (engram RAG):  tests the FULL engram pipeline — retrieval + synthesis
  --full-context:        passes ALL knowledge directly to LLM, bypassing retrieval
                         → upper bound / ceiling score; gap vs engram = retrieval loss

Usage:
    # Run engram RAG eval
    python eval/engram_eval.py --project Personal --dataset eval/qa_dataset.json --use-judge

    # Run full-context ceiling (10M context = perfect retrieval)
    python eval/engram_eval.py --project Personal --dataset eval/qa_dataset.json \
        --full-context --use-judge

    # Compare both (shows retrieval gap)
    python eval/engram_eval.py --project Personal --dataset eval/qa_dataset.json \
        --full-context --use-judge --prev eval/domain_results_v1.json

    # Quick smoke test (10 questions per category)
    python eval/engram_eval.py --project Personal --dataset eval/qa_dataset.json \
        --max-per-cat 10 --full-context --use-judge
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
import unicodedata
import urllib.request
from collections import defaultdict
from pathlib import Path

# ---------------------------------------------------------------------------
# Scoring (same as LoCoMo protocol for comparability)
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
# Gemini helper for judge
# ---------------------------------------------------------------------------

def gemini_call(prompt: str, api_key: str,
                model: str = "gemini-2.5-flash") -> str:
    payload = json.dumps({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {"temperature": 0.0, "maxOutputTokens": 10},
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
        with urllib.request.urlopen(req, timeout=30) as resp:
            result = json.loads(resp.read())
        for part in result["candidates"][0].get("content", {}).get("parts", []):
            if "text" in part:
                return part["text"].strip()
        return ""
    except Exception as e:
        return f"[error: {e}]"


JUDGE_PROMPT = """Is this answer correct or semantically equivalent to the gold answer?

Question: {question}
Gold: {gold}
Answer: {prediction}

Reply YES or NO only."""


def llm_judge(question: str, gold, prediction: str, api_key: str, model: str) -> float:
    if not prediction or "Not found in knowledge base" in prediction:
        return 0.0
    resp = gemini_call(
        JUDGE_PROMPT.format(question=question, gold=str(gold)[:100],
                            prediction=prediction[:200]),
        api_key, model,
    )
    return 1.0 if resp.strip().upper().startswith("YES") else 0.0


# ---------------------------------------------------------------------------
# Full-context (ceiling) answering
# ---------------------------------------------------------------------------

def load_full_knowledge(project: str) -> str:
    """Load all knowledge files for a project into a single context string."""
    from pathlib import Path
    knowledge_dir = Path.home() / "memory" / "knowledge" / project
    categories = ["decisions", "solutions", "patterns", "bugs", "insights",
                  "questions", "procedures"]
    parts = []
    for cat in categories:
        path = knowledge_dir / f"{cat}.md"
        if path.exists():
            content = path.read_text().strip()
            if content:
                parts.append(content)
    return "\n\n---\n\n".join(parts)


FULL_CONTEXT_SYSTEM = (
    "You are a precise technical assistant. Answer the question using ONLY the provided "
    "knowledge base. Give a SHORT answer: 1-15 words. Use exact names, values, and terms "
    "from the knowledge. If the answer is not in the knowledge, say: Not found."
)

FULL_CONTEXT_PROMPT = """{knowledge}

---
QUESTION: {question}

Short answer (1-15 words):"""


def ask_full_context(question: str, full_knowledge: str,
                     api_key: str, model: str = "gemini-2.5-flash") -> str:
    """Answer a question with the full knowledge base in context — no retrieval."""
    prompt = FULL_CONTEXT_PROMPT.format(
        knowledge=full_knowledge[:900_000],  # Stay within context limit
        question=question,
    )
    # Use flash model for speed (28K tokens × 385 questions = manageable)
    effective_max = 4096 if "2.5-pro" in model else 256
    payload = json.dumps({
        "systemInstruction": {"parts": [{"text": FULL_CONTEXT_SYSTEM}]},
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {"temperature": 0.0, "maxOutputTokens": effective_max},
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
        with urllib.request.urlopen(req, timeout=60) as resp:
            result = json.loads(resp.read())
        for part in result["candidates"][0].get("content", {}).get("parts", []):
            if "text" in part and part["text"].strip():
                ans = part["text"].strip()
                if "not found" in ans.lower():
                    return ""
                return ans
        return ""
    except Exception as e:
        return ""


# ---------------------------------------------------------------------------
# engram ask wrapper
# ---------------------------------------------------------------------------

def ask_engram(binary: str, project: str, question: str,
               threshold: float = 0.2, top_k: int = 8,
               concise: bool = True, model: str = "") -> str:
    args = [binary, "ask", question, "--project", project,
            "--threshold", str(threshold), "--top-k", str(top_k)]
    if concise:
        args.append("--concise")

    env = {**os.environ}
    if model:
        env["ENGRAM_LLM_MODEL"] = model

    result = subprocess.run(args, capture_output=True, text=True, timeout=30, env=env)
    if result.returncode != 0 or "Not found in knowledge base" in result.stdout:
        return ""
    lines = [l for l in result.stdout.strip().splitlines()
             if not l.startswith("Sources:") and not l.startswith("Hint:")]
    return "\n".join(lines).strip()


# ---------------------------------------------------------------------------
# Reporting
# ---------------------------------------------------------------------------

CATEGORIES = ["decisions", "solutions", "patterns", "bugs", "insights", "procedures"]

def _avg(vals: list[float]) -> float:
    return sum(vals) / len(vals) * 100 if vals else 0.0


def print_report(f1_by_cat: dict, judge_by_cat: dict, elapsed: float,
                 label: str, prev_results: dict | None = None):
    all_f1 = [s for v in f1_by_cat.values() for s in v]
    all_judge = [s for v in judge_by_cat.values() for s in v]
    overall_f1 = _avg(all_f1)
    overall_judge = _avg(all_judge)

    print()
    print("╔══════════════════════════════════════════════════════╗")
    print(f"  engram Domain Eval")
    print(f"  {label}")
    print(f"  QA pairs: {len(all_f1)}  |  {elapsed:.0f}s")
    print("╠══════════════════════════════════════════════════════╣")

    prev_f1 = prev_results.get("overall_f1", 0) if prev_results else 0
    delta = overall_f1 - prev_f1
    sign = "+" if delta >= 0 else ""
    prev_str = f"  ({sign}{delta:.1f} vs prev)" if prev_results else ""
    print(f"  Token-F1:     {overall_f1:.1f}{prev_str}")
    if all_judge:
        prev_j = prev_results.get("overall_judge", 0) if prev_results else 0
        d_j = overall_judge - prev_j
        s_j = "+" if d_j >= 0 else ""
        pj_str = f"  ({s_j}{d_j:.1f} vs prev)" if prev_results else ""
        print(f"  LLM-judge:    {overall_judge:.1f}{pj_str}")

    print()
    print("  By category:")

    header = f"    {'category':<14} {'F1':>6}  {'n':>4}"
    if all_judge:
        header += f"  {'Judge':>6}"
    if prev_results:
        header += f"  {'Δ F1':>6}"
    print(header)
    print("    " + "─" * 50)

    for cat in CATEGORIES:
        vals = f1_by_cat.get(cat, [])
        if not vals:
            continue
        avg = _avg(vals)
        j_avg = _avg(judge_by_cat.get(cat, []))
        bar = "█" * max(1, int(avg / 5))
        row = f"    {cat:<14} {avg:6.1f}  {len(vals):4}  {bar}"
        if all_judge:
            row += f"  {j_avg:6.1f}"
        if prev_results:
            prev_cat = prev_results.get("by_category", {}).get(cat, {}).get("f1", 0)
            d = avg - prev_cat
            s = "+" if d >= 0 else ""
            row += f"  {s}{d:.1f}"
        print(row)

    not_found = sum(1 for v in f1_by_cat.values() for s in v if s == 0.0)
    print()
    print(f"  Not found / wrong: {not_found}/{len(all_f1)} "
          f"({not_found/len(all_f1)*100:.1f}%)")

    # Show retrieval gap vs ceiling if prev is the full-context result
    if prev_results and prev_results.get("label", "").find("FULL-CONTEXT") != -1:
        ceiling_f1 = prev_results.get("overall_f1", 0)
        ceiling_j = prev_results.get("overall_judge", 0)
        if ceiling_f1 > 0:
            pct_f1 = overall_f1 / ceiling_f1 * 100
            pct_j = overall_judge / ceiling_j * 100 if ceiling_j > 0 else 0
            print()
            print(f"  Retrieval efficiency vs ceiling:")
            print(f"    Token-F1:  {overall_f1:.1f} / {ceiling_f1:.1f} = {pct_f1:.0f}% of ceiling")
            if all_judge:
                print(f"    LLM-judge: {overall_judge:.1f} / {ceiling_j:.1f} = {pct_j:.0f}% of ceiling")
    print("╚══════════════════════════════════════════════════════╝")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser(description="engram domain eval")
    ap.add_argument("--project", default="Personal")
    ap.add_argument("--dataset", default="eval/qa_dataset.json")
    ap.add_argument("--engram", default="./target/release/engram")
    ap.add_argument("--model", default="",
                    help="LLM model override for engram ask (default: use configured)")
    ap.add_argument("--judge-model", default="gemini-2.5-flash")
    ap.add_argument("--threshold", type=float, default=0.2)
    ap.add_argument("--top-k", type=int, default=8)
    ap.add_argument("--use-judge", action="store_true",
                    help="Score with LLM-as-a-Judge (semantic accuracy)")
    ap.add_argument("--full-context", action="store_true",
                    help="Upper bound: answer with ALL knowledge in context (no retrieval)")
    ap.add_argument("--max-per-cat", type=int, default=None,
                    help="Limit questions per category (quick test)")
    ap.add_argument("--categories",
                    default="decisions,solutions,patterns,bugs,insights,procedures")
    ap.add_argument("--output", default="eval/domain_results.json")
    ap.add_argument("--prev", default=None,
                    help="Path to previous results JSON for delta comparison")
    args = ap.parse_args()

    binary = str(Path(args.engram).resolve())
    if not Path(binary).exists():
        print(f"ERROR: {binary} not found. Run: cargo build --release", file=sys.stderr)
        sys.exit(1)

    if not Path(args.dataset).exists():
        print(f"ERROR: {args.dataset} not found.", file=sys.stderr)
        print("Generate first: python eval/gen_qa_dataset.py", file=sys.stderr)
        sys.exit(1)

    api_key = os.environ.get("GEMINI_API_KEY", "")
    if args.use_judge and not api_key:
        print("ERROR: GEMINI_API_KEY required for --use-judge", file=sys.stderr)
        sys.exit(1)

    with open(args.dataset) as f:
        dataset = json.load(f)

    qa_pairs = dataset["qa_pairs"]
    categories = [c.strip() for c in args.categories.split(",")]
    qa_pairs = [q for q in qa_pairs if q["category"] in categories]

    if args.max_per_cat:
        filtered = []
        counts = defaultdict(int)
        for qa in qa_pairs:
            if counts[qa["category"]] < args.max_per_cat:
                filtered.append(qa)
                counts[qa["category"]] += 1
        qa_pairs = filtered

    prev_results = None
    if args.prev and Path(args.prev).exists():
        with open(args.prev) as f:
            prev_results = json.load(f)

    label_parts = [
        f"project={args.project}",
    ]
    if args.full_context:
        label_parts.append("FULL-CONTEXT (ceiling)")
    else:
        label_parts.append(f"t={args.threshold} k={args.top_k}")
        if args.model:
            label_parts.append(f"model={args.model}")
    if args.use_judge:
        label_parts.append("judge")
    label = " | ".join(label_parts)

    # Load full knowledge once if needed
    full_knowledge = None
    fc_model = "gemini-2.5-flash"  # Fast + large context
    if args.full_context:
        if not api_key:
            print("ERROR: GEMINI_API_KEY required for --full-context", file=sys.stderr)
            sys.exit(1)
        full_knowledge = load_full_knowledge(args.project)
        tokens_est = len(full_knowledge) // 4
        print(f"  Full context: {len(full_knowledge):,} chars (~{tokens_est:,} tokens)")

    print(f"engram Domain Eval")
    print(f"  Project:   {args.project}")
    print(f"  Questions: {len(qa_pairs)} across {len(categories)} categories")
    print(f"  Config:    {label}")
    print()

    f1_by_cat: dict[str, list[float]] = defaultdict(list)
    judge_by_cat: dict[str, list[float]] = defaultdict(list)
    start = time.time()

    for i, qa in enumerate(qa_pairs):
        question = qa["question"]
        gold = qa["answer"]
        category = qa["category"]

        if args.full_context:
            prediction = ask_full_context(question, full_knowledge, api_key, fc_model)
        else:
            prediction = ask_engram(
                binary, args.project, question,
                threshold=args.threshold, top_k=args.top_k,
                concise=True, model=args.model,
            )

        f1 = token_f1(prediction, gold)
        f1_by_cat[category].append(f1)

        if args.use_judge and api_key:
            j = llm_judge(question, gold, prediction, api_key, args.judge_model)
            judge_by_cat[category].append(j)

        if (i + 1) % 20 == 0:
            done = i + 1
            all_so_far = [s for v in f1_by_cat.values() for s in v]
            print(f"  [{done}/{len(qa_pairs)}] F1={_avg(all_so_far):.1f}", flush=True)

    elapsed = time.time() - start

    # Build results dict
    all_f1 = [s for v in f1_by_cat.values() for s in v]
    all_judge = [s for v in judge_by_cat.values() for s in v]
    results = {
        "overall_f1": _avg(all_f1),
        "overall_judge": _avg(all_judge),
        "elapsed": elapsed,
        "label": label,
        "by_category": {
            cat: {
                "f1": _avg(f1_by_cat.get(cat, [])),
                "judge": _avg(judge_by_cat.get(cat, [])),
                "n": len(f1_by_cat.get(cat, [])),
            }
            for cat in categories if f1_by_cat.get(cat)
        },
        "args": vars(args),
    }

    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults → {args.output}")

    print_report(f1_by_cat, judge_by_cat, elapsed, label, prev_results)


if __name__ == "__main__":
    main()
