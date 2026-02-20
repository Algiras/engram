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

def gemini_call(prompt: str, api_key: str, model: str = "gemini-2.5-pro",
                max_tokens: int = 2048) -> str:
    # Thinking models (2.5-pro) need extra tokens for internal reasoning
    effective_max = max(max_tokens, 4096) if "2.5-pro" in model else max_tokens
    payload = json.dumps({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {"temperature": 0.2, "maxOutputTokens": effective_max},
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
    "decisions": """Given this technical decision record, generate {n} questions that are DIRECTLY and COMPLETELY answerable from the text below.

Rules:
- Every question must have a clear, specific answer in the content
- Do NOT ask about things the content doesn't mention
- Focus on: what was decided, why, what the alternatives were, what the context was
- Questions should be natural ("What was decided about X?" not "Please describe...")

Format: one question per line, no numbering, end each with ?

CONTENT:
{content}

Questions:""",

    "solutions": """Given this problem-solution record, generate {n} questions DIRECTLY answerable from the text.

Rules:
- Every question must have a specific answer in the content
- Focus on: what the problem was, how it was solved, the key insight, the fix
- Questions like "How was X fixed?", "What caused X?", "What was the solution to X?"

Format: one question per line, end each with ?

CONTENT:
{content}

Questions:""",

    "patterns": """Given this codebase pattern record, generate {n} questions DIRECTLY answerable from the text.

Rules:
- Every question must have a specific answer in the content
- Focus on: the pattern name, how it works, which files use it, why it exists
- Questions like "What pattern is used for X?", "How does X work?", "Which files use X?"

Format: one question per line, end each with ?

CONTENT:
{content}

Questions:""",

    "bugs": """Given this bug record, generate {n} questions DIRECTLY answerable from the text.

Rules:
- Every question must have a specific answer in the content
- Focus on: what went wrong, the root cause, the fix applied
- Questions like "What was the bug in X?", "What caused X?", "How was X fixed?"

Format: one question per line, end each with ?

CONTENT:
{content}

Questions:""",

    "insights": """Given this technical insight record, generate {n} questions DIRECTLY answerable from the text.

Rules:
- Every question must have a specific answer in the content
- Focus on: the insight itself, why it's non-obvious, when it applies
- Questions like "What is the key insight about X?", "Why does X behave this way?"

Format: one question per line, end each with ?

CONTENT:
{content}

Questions:""",

    "procedures": """Given this procedure/workflow record, generate {n} questions DIRECTLY answerable from the text.

Rules:
- Every question must have a specific answer in the content
- ALWAYS include the procedure name or a key distinguishing phrase in the question so it is unambiguous
- NEVER ask "when should this procedure be used?" without naming the procedure
- Focus on: the steps, what the procedure accomplishes, when to use it, which commands/files are involved
- Good examples: "What is the first step in the 'Add Hooks Health Check' procedure?",
  "When should the 'engram ingest in the background' procedure be used?",
  "Which file is modified in step 2 of the provider config procedure?"
- Bad examples: "What is the first step?", "When should this procedure be used?"

Format: one question per line, end each with ?

CONTENT:
{content}

Questions:""",
}

ANSWER_PROMPT = """Given this content, answer the following question in 1-15 words.
Use exact terms, names, and values from the content. No explanation.
If the content does not contain enough information to answer, say: SKIP

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

    questions = []
    for line in raw.splitlines():
        line = re.sub(r"^\d+[.)]\s*", "", line.strip()).lstrip("•-*").strip()
        if len(line) > 10:
            # Accept lines that look like questions (end with ? or contain question words)
            if "?" in line or any(
                line.lower().startswith(w)
                for w in ["what", "why", "how", "when", "where", "which", "who"]
            ):
                questions.append(line if line.endswith("?") else line + "?")

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
        # Skip questions the LLM itself couldn't answer from the content
        skip_phrases = ["skip", "content does not", "not specified", "not mentioned",
                        "not provided", "no information", "cannot answer", "not stated"]
        if any(p in answer.lower() for p in skip_phrases):
            continue
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
    ap.add_argument("--model", default="gemini-2.5-flash",
                    help="Model for QA generation (default: gemini-2.5-flash)")
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
