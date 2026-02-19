#!/usr/bin/env python3
"""
Generate a QA eval dataset from engram's own knowledge files.

For each session block in a project's knowledge files, uses an LLM to generate
3-5 targeted questions whose answers are directly contained in that block.
This creates a ground-truth eval dataset aligned with engram's actual domain
(Claude Code sessions, technical knowledge).

Usage:
    python eval/gen_qa_dataset.py --project Personal --output eval/qa_dataset.json
    python eval/gen_qa_dataset.py --project Personal --categories decisions,solutions,bugs
"""

import argparse
import json
import os
import re
import sys
import time
import urllib.request
from pathlib import Path

# ---------------------------------------------------------------------------
# Gemini helper
# ---------------------------------------------------------------------------

def gemini_call(prompt: str, api_key: str, model: str = "gemini-2.5-pro") -> str:
    payload = json.dumps({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {"temperature": 0.2, "maxOutputTokens": 1024},
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
        candidate = result["candidates"][0]
        for part in candidate.get("content", {}).get("parts", []):
            if "text" in part and part["text"].strip():
                return part["text"].strip()
        return ""
    except Exception as e:
        return f"[error: {e}]"


# ---------------------------------------------------------------------------
# QA generation per category
# ---------------------------------------------------------------------------

CATEGORY_PROMPTS = {
    "decisions": """Given this technical decision record from a software project, generate {n} specific questions that can be answered from the text.

Focus on: what was decided, why it was decided, what alternatives were considered.
Questions should require reading the content — not answerable from the question alone.

Format: one question per line, no numbering.

CONTENT:
{content}

Questions:""",

    "solutions": """Given this problem-solution record from a software project, generate {n} specific questions.

Focus on: what the problem was, how it was solved, the key insight.
Questions should be specific enough that the answer is clearly in the content.

Format: one question per line, no numbering.

CONTENT:
{content}

Questions:""",

    "patterns": """Given this codebase pattern/convention record, generate {n} specific questions.

Focus on: what the pattern is, where it's used, how it works.
Questions should target non-obvious details a developer would need to know.

Format: one question per line, no numbering.

CONTENT:
{content}

Questions:""",

    "bugs": """Given this bug record from a software project, generate {n} specific questions.

Focus on: what the bug was, its root cause, how it was fixed.
Questions should be specific and answerable from the content.

Format: one question per line, no numbering.

CONTENT:
{content}

Questions:""",

    "insights": """Given this technical insight record, generate {n} specific questions.

Focus on: what the insight is, why it matters, when it applies.
Questions should probe understanding of the non-obvious realization.

Format: one question per line, no numbering.

CONTENT:
{content}

Questions:""",

    "procedures": """Given this workflow/procedure record, generate {n} specific questions.

Focus on: what the steps are, when to use this procedure, what it accomplishes.
Questions should be practical and answerable from the content.

Format: one question per line, no numbering.

CONTENT:
{content}

Questions:""",
}

ANSWER_PROMPT = """Given this content, answer the following question in 1-15 words.
Be specific and use exact terms from the content. No explanation.

CONTENT:
{content}

QUESTION: {question}

Short answer:"""


def generate_qa_pairs(content: str, category: str, session_id: str,
                      api_key: str, model: str, n_questions: int = 3) -> list[dict]:
    """Generate QA pairs for one session block."""
    if content.strip().startswith("<!-- superseded"):
        return []

    # Truncate very long blocks
    content_truncated = content.strip()[:2000]

    prompt = CATEGORY_PROMPTS.get(category, CATEGORY_PROMPTS["solutions"]).format(
        content=content_truncated,
        n=n_questions,
    )

    raw = gemini_call(prompt, api_key, model)
    if not raw or raw.startswith("[error"):
        return []

    questions = [
        line.strip().lstrip("•-*").strip()
        for line in raw.splitlines()
        if line.strip() and len(line.strip()) > 10 and "?" in line
    ]

    qa_pairs = []
    for question in questions[:n_questions]:
        answer_raw = gemini_call(
            ANSWER_PROMPT.format(content=content_truncated, question=question),
            api_key, model,
        )
        if not answer_raw or answer_raw.startswith("[error") or len(answer_raw) < 2:
            continue
        # Clean the answer
        answer = answer_raw.strip().strip('"').strip("'")
        if len(answer) > 100:  # Too long = LLM went off-script
            continue
        qa_pairs.append({
            "question": question,
            "answer": answer,
            "category": category,
            "session_id": session_id,
            "source_content": content_truncated[:300],
        })
        time.sleep(0.2)  # Rate limiting

    return qa_pairs


# ---------------------------------------------------------------------------
# Block parser (minimal, no Rust dependency)
# ---------------------------------------------------------------------------

def parse_blocks(content: str) -> list[dict]:
    """Parse session blocks from a knowledge file."""
    header_re = re.compile(
        r"(?m)^## Session: (\S+) \(([^)]+)\)((?:\s*\[[^\]]+\])*)"
    )
    positions = list(header_re.finditer(content))
    blocks = []
    for i, m in enumerate(positions):
        session_id = m.group(1)
        end = positions[i + 1].start() if i + 1 < len(positions) else len(content)
        block_content = content[m.end():end].strip()
        if block_content:
            blocks.append({"session_id": session_id, "content": block_content})
    return blocks


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser(description="Generate QA eval dataset from engram knowledge")
    ap.add_argument("--project", default="Personal")
    ap.add_argument("--categories", default="decisions,solutions,patterns,bugs,insights,procedures")
    ap.add_argument("--questions-per-block", type=int, default=3)
    ap.add_argument("--max-blocks", type=int, default=None,
                    help="Limit blocks per category (for quick test runs)")
    ap.add_argument("--model", default="gemini-2.5-pro")
    ap.add_argument("--output", default="eval/qa_dataset.json")
    args = ap.parse_args()

    api_key = os.environ.get("GEMINI_API_KEY", "")
    if not api_key:
        print("ERROR: GEMINI_API_KEY not set", file=sys.stderr)
        sys.exit(1)

    memory_dir = Path.home() / "memory" / "knowledge" / args.project
    if not memory_dir.exists():
        print(f"ERROR: No knowledge found for project '{args.project}'", file=sys.stderr)
        sys.exit(1)

    categories = [c.strip() for c in args.categories.split(",")]
    all_qa: list[dict] = []

    for cat in categories:
        path = memory_dir / f"{cat}.md"
        if not path.exists():
            continue

        blocks = parse_blocks(path.read_text())
        if args.max_blocks:
            blocks = blocks[:args.max_blocks]

        print(f"  {cat}: {len(blocks)} blocks → generating QA pairs...")
        for block in blocks:
            pairs = generate_qa_pairs(
                block["content"], cat, block["session_id"],
                api_key, args.model, args.questions_per_block,
            )
            all_qa.extend(pairs)
            if pairs:
                print(f"    [{block['session_id'][:20]}] {len(pairs)} pairs")

    print(f"\nTotal QA pairs generated: {len(all_qa)}")

    by_cat = {}
    for qa in all_qa:
        by_cat.setdefault(qa["category"], []).append(qa)
    for cat, pairs in by_cat.items():
        print(f"  {cat}: {len(pairs)}")

    Path(args.output).parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w") as f:
        json.dump({
            "project": args.project,
            "model": args.model,
            "qa_pairs": all_qa,
            "total": len(all_qa),
            "by_category": {k: len(v) for k, v in by_cat.items()},
        }, f, indent=2)
    print(f"\nDataset saved → {args.output}")


if __name__ == "__main__":
    main()
