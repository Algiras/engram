#!/usr/bin/env python3
"""
Automated improvement loop for engram retrieval quality.

Runs: measure → analyze failures → apply fix → rebuild → measure → repeat.
Stops when judge score reaches TARGET or no further improvement found.

Usage:
    python eval/improvement_loop.py --project Personal --dataset eval/qa_dataset.json
    python eval/improvement_loop.py --project Personal --max-iterations 5 --target-judge 13.0
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).parent.parent

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def run(cmd: list, timeout: int = 300, env: dict | None = None) -> tuple[int, str, str]:
    result = subprocess.run(
        cmd, capture_output=True, text=True, timeout=timeout,
        env={**os.environ, **(env or {})}
    )
    return result.returncode, result.stdout, result.stderr


def load_results(path: str) -> dict:
    if Path(path).exists():
        with open(path) as f:
            return json.load(f)
    return {}


def overall_judge(results: dict) -> float:
    return results.get("overall_judge", 0.0)


def not_found_rate(results: dict) -> float:
    """Estimate not-found rate from judge=0 proportion (approx)."""
    by_cat = results.get("by_category", {})
    total_n = sum(v.get("n", 0) for v in by_cat.values())
    # Store raw not_found if available
    return results.get("not_found_rate", 0.0)


# ---------------------------------------------------------------------------
# Failure analysis
# ---------------------------------------------------------------------------

def analyze_failures(results_path: str, dataset_path: str,
                     engram_binary: str, project: str) -> dict:
    """
    Run a detailed failure analysis pass:
    - Which categories have most failures?
    - Are failures due to not-found or wrong synthesis?
    - What's the avg embedding similarity for failures vs successes?
    """
    results = load_results(results_path)
    by_cat = results.get("by_category", {})

    analysis = {}
    for cat, stats in by_cat.items():
        f1 = stats.get("f1", 0)
        judge = stats.get("judge", 0)
        ceiling = 67.9  # Known ceiling F1
        gap = ceiling - f1
        analysis[cat] = {
            "f1": f1,
            "judge": judge,
            "gap_to_ceiling": gap,
            "priority": gap,  # Higher gap = higher priority fix
        }

    # Sort by gap
    sorted_cats = sorted(analysis.items(), key=lambda x: -x[1]["priority"])

    print("\n  Failure analysis:")
    print(f"  {'category':<16} {'F1':>6}  {'judge':>6}  {'gap':>6}  priority")
    print("  " + "─" * 55)
    for cat, stats in sorted_cats:
        bar = "█" * max(1, int(stats["gap_to_ceiling"] / 5))
        print(f"  {cat:<16} {stats['f1']:6.1f}  {stats['judge']:6.1f}  "
              f"{stats['gap_to_ceiling']:6.1f}  {bar}")

    return {"by_category": analysis, "sorted_priorities": [c for c, _ in sorted_cats]}


# ---------------------------------------------------------------------------
# Improvement strategies
# ---------------------------------------------------------------------------

def strategy_lower_threshold(current_threshold: float) -> float:
    """Lower threshold by 0.03, minimum 0.05."""
    return max(0.05, current_threshold - 0.03)


def strategy_rebuild_index(project: str, engram: str) -> bool:
    """Rebuild embedding index for project."""
    print(f"  → Rebuilding embedding index for {project}...")
    rc, out, err = run([engram, "embed", project, "--provider", "gemini"], timeout=300)
    if rc == 0:
        # Parse chunk counts from output
        chunks = re.search(r"Total chunks: (\d+)", out)
        print(f"  ✓ Index rebuilt: {chunks.group(1) if chunks else '?'} chunks")
        return True
    print(f"  ✗ Rebuild failed: {err[:80]}")
    return False


def strategy_reduce_chunk_size(project: str) -> bool:
    """
    Reduce chunk size in search.rs from 1000 → 500 chars for more atomic retrieval.
    Rebuilds the Rust binary after the change.
    """
    search_rs = ROOT / "src" / "embeddings" / "search.rs"
    content = search_rs.read_text()

    if "chunk_text(&content, 500)" in content:
        print("  → Chunk size already 500, skipping")
        return False

    if "chunk_text(&content, 1000)" not in content:
        print("  → chunk_text(1000) not found, skipping")
        return False

    new_content = content.replace("chunk_text(&content, 1000)", "chunk_text(&content, 500)")
    search_rs.write_text(new_content)
    print("  → Reduced chunk size 1000→500 chars, rebuilding binary...")

    rc, _, err = run(
        ["cargo", "build", "--release"],
        cwd=str(ROOT), timeout=300
    )
    if rc != 0:
        # Revert
        search_rs.write_text(content)
        print(f"  ✗ Build failed, reverted: {err[:80]}")
        return False

    print("  ✓ Binary rebuilt with smaller chunks")
    return True


def strategy_increase_topk(current_k: int) -> int:
    """Increase top-k by 4, max 24."""
    return min(24, current_k + 4)


def strategy_improve_synthesis(iteration: int) -> bool:
    """
    Improve the SYSTEM_QA_CONCISE prompt based on observed failure patterns.
    Each iteration adds more specific guidance.
    """
    prompts_rs = ROOT / "src" / "llm" / "prompts.rs"
    content = prompts_rs.read_text()

    # Iteration 2: add category-specific guidance
    if iteration == 2 and "For bug entries" not in content:
        new_system = content.replace(
            '     (5) No explanation, no preamble, no \'The answer is\'. Just the answer. \\',
            '     (5) For bug entries: state the fix/root-cause directly. \\\n     (6) For pattern entries: name the pattern/mechanism. \\\n     (7) For procedure entries: list the key steps concisely. \\\n     (8) No explanation, no preamble, no \'The answer is\'. Just the answer. \\'
        )
        if new_system != content:
            prompts_rs.write_text(new_system)
            print("  → Added category-specific synthesis guidance")
            return True

    return False


# ---------------------------------------------------------------------------
# Main loop
# ---------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser(description="Automated engram improvement loop")
    ap.add_argument("--project", default="Personal")
    ap.add_argument("--dataset", default="eval/qa_dataset.json")
    ap.add_argument("--engram", default="./target/release/engram")
    ap.add_argument("--max-iterations", type=int, default=6)
    ap.add_argument("--target-judge", type=float, default=13.5,
                    help="Stop when LLM-judge reaches this score (ceiling=14.0)")
    ap.add_argument("--max-per-cat", type=int, default=None,
                    help="Limit questions per category (faster iterations)")
    ap.add_argument("--use-judge", action="store_true", default=True)
    args = ap.parse_args()

    api_key = os.environ.get("GEMINI_API_KEY", "")
    engram = str(Path(args.engram).resolve())

    print(f"╔══════════════════════════════════════════════════╗")
    print(f"  engram Improvement Loop")
    print(f"  Project: {args.project}  |  Target judge: {args.target_judge}")
    print(f"  Max iterations: {args.max_iterations}")
    print(f"╚══════════════════════════════════════════════════╝")

    # Track state across iterations
    threshold = 0.15
    top_k = 12
    history = []

    # Load last known results as starting point if available
    baseline_path = "eval/domain_results_v3.json"
    if Path(baseline_path).exists():
        baseline = load_results(baseline_path)
        print(f"\nStarting from: F1={baseline.get('overall_f1', 0):.1f}  "
              f"judge={baseline.get('overall_judge', 0):.1f}")
    else:
        baseline = {}

    prev_path = baseline_path if Path(baseline_path).exists() else None

    for iteration in range(1, args.max_iterations + 1):
        print(f"\n{'='*52}")
        print(f"  Iteration {iteration}  (threshold={threshold}  top_k={top_k})")
        print(f"{'='*52}")

        # ── Step 1: Run eval ───────────────────────────────────────
        output_path = f"eval/loop_iter_{iteration}.json"
        eval_cmd = [
            "python3", "eval/engram_eval.py",
            "--project", args.project,
            "--dataset", args.dataset,
            "--engram", engram,
            "--threshold", str(threshold),
            "--top-k", str(top_k),
            "--output", output_path,
        ]
        if args.use_judge:
            eval_cmd.append("--use-judge")
        if args.max_per_cat:
            eval_cmd.extend(["--max-per-cat", str(args.max_per_cat)])
        if prev_path:
            eval_cmd.extend(["--prev", prev_path])

        print(f"\n  Running eval ({args.max_per_cat or 'full'} questions per cat)...")
        start = time.time()
        rc, out, err = run(eval_cmd, timeout=1200)
        elapsed = time.time() - start

        if rc != 0:
            print(f"  ✗ Eval failed: {err[:200]}")
            break

        print(out)

        results = load_results(output_path)
        current_f1 = results.get("overall_f1", 0)
        current_judge = results.get("overall_judge", 0)
        history.append({
            "iteration": iteration,
            "f1": current_f1,
            "judge": current_judge,
            "threshold": threshold,
            "top_k": top_k,
        })
        prev_path = output_path

        # ── Step 2: Check stopping condition ──────────────────────
        if current_judge >= args.target_judge:
            print(f"\n  🎉 TARGET REACHED: judge={current_judge:.1f} >= {args.target_judge}")
            break

        # ── Step 3: Analyze failures ──────────────────────────────
        analysis = analyze_failures(output_path, args.dataset, engram, args.project)
        priorities = analysis["sorted_priorities"]

        # ── Step 4: Apply targeted fixes ──────────────────────────
        print(f"\n  Applying fixes for iteration {iteration + 1}...")

        applied = []

        # Fix 1: Lower threshold if not-found is high
        nf = results.get("not_found_rate", 0)
        by_cat = results.get("by_category", {})
        total_n = sum(v.get("n", 0) for v in by_cat.values())
        # Estimate not-found from zero scores
        # (we don't store per-question scores, but can estimate from judge=0 patterns)
        if threshold > 0.07:
            new_t = strategy_lower_threshold(threshold)
            if new_t != threshold:
                threshold = new_t
                applied.append(f"threshold → {threshold:.3f}")

        # Fix 2: Increase top-k if still missing answers
        if top_k < 20 and current_judge < args.target_judge * 0.9:
            new_k = strategy_increase_topk(top_k)
            if new_k != top_k:
                top_k = new_k
                applied.append(f"top_k → {top_k}")

        # Fix 3: Reduce chunk size on iteration 2 (more atomic retrieval)
        if iteration == 2:
            if strategy_reduce_chunk_size(args.project):
                # Rebuild index after chunk size change
                strategy_rebuild_index(args.project, engram)
                applied.append("chunk_size 1000→500 + reindex")
            else:
                # Just rebuild index to pick up any new knowledge
                strategy_rebuild_index(args.project, engram)
                applied.append("reindex")

        # Fix 4: Rebuild index on every other iteration to pick up new knowledge
        if iteration % 2 == 0 and "reindex" not in str(applied):
            strategy_rebuild_index(args.project, engram)
            applied.append("reindex")

        # Fix 5: Improve synthesis prompt
        if iteration == 2:
            if strategy_improve_synthesis(iteration):
                # Rebuild binary
                print("  → Rebuilding binary with improved prompts...")
                rc2, _, err2 = run(
                    ["cargo", "build", "--release"],
                    cwd=str(ROOT), timeout=300
                )
                if rc2 == 0:
                    print("  ✓ Binary rebuilt")
                    applied.append("synthesis_prompt_v2")
                else:
                    print(f"  ✗ Build failed: {err2[:80]}")

        if applied:
            print(f"  Applied: {', '.join(applied)}")
        else:
            print("  No more fixes to apply")
            break

    # ── Summary ───────────────────────────────────────────────────
    print(f"\n{'='*52}")
    print(f"  Improvement Loop Summary")
    print(f"{'='*52}")
    print(f"  {'iter':>4}  {'F1':>6}  {'judge':>6}  {'threshold':>9}  {'top_k':>5}")
    print(f"  {'─'*40}")
    for h in history:
        print(f"  {h['iteration']:>4}  {h['f1']:6.1f}  {h['judge']:6.1f}  "
              f"{h['threshold']:9.3f}  {h['top_k']:5}")

    if history:
        first = history[0]
        last = history[-1]
        print(f"\n  Total improvement:")
        print(f"    F1:    {first['f1']:.1f} → {last['f1']:.1f}  "
              f"(+{last['f1'] - first['f1']:.1f})")
        print(f"    judge: {first['judge']:.1f} → {last['judge']:.1f}  "
              f"(+{last['judge'] - first['judge']:.1f})")
        ceiling_judge = 14.0
        print(f"    ceiling efficiency: {last['judge'] / ceiling_judge * 100:.0f}%")

    # Save loop results
    with open("eval/loop_results.json", "w") as f:
        json.dump({"history": history, "target": args.target_judge}, f, indent=2)
    print(f"\n  History → eval/loop_results.json")


if __name__ == "__main__":
    main()
