pub const SYSTEM_KNOWLEDGE_EXTRACTOR: &str = r#"You are a knowledge extraction assistant. You analyze software development conversations and extract structured knowledge. Be concise and factual. Only extract what is clearly stated or demonstrated in the conversation."#;

pub fn decisions_prompt(conversation_text: &str) -> String {
    format!(
        r#"Analyze this Claude Code conversation and extract key technical decisions that were made.

For each decision, write:
- **Decision**: What was decided
- **Context**: Why it was decided (if clear)
- **Alternatives**: What was considered (if mentioned)

Rules:
- Only include clear, actionable decisions with lasting impact. Skip trivial or obvious choices.
- Maximum 5 decisions. If fewer are significant, extract fewer.
- Each decision: 1-3 lines maximum.
- If no significant decisions were made, respond with exactly: "No significant decisions."

---
CONVERSATION:
{}
---

Extract decisions:"#,
        truncate_for_llm(conversation_text)
    )
}

pub fn solutions_prompt(conversation_text: &str) -> String {
    format!(
        r#"Analyze this Claude Code conversation and extract non-trivial problems that were solved.

For each problem-solution pair, write:
- **Problem**: What issue was encountered
- **Solution**: How it was resolved
- **Key insight**: The crucial realization (if any)

Rules:
- Focus on problems likely to recur. Skip trivial fixes or one-liners.
- Maximum 5 solutions. If fewer are significant, extract fewer.
- Each entry: 2-4 lines maximum.
- If no significant problems were solved, respond with exactly: "No significant problems solved."

---
CONVERSATION:
{}
---

Extract solutions:"#,
        truncate_for_llm(conversation_text)
    )
}

pub fn patterns_prompt(conversation_text: &str) -> String {
    format!(
        r#"Analyze this Claude Code conversation and extract codebase patterns and conventions that were discovered or used.

For each pattern, write:
- **Pattern**: Name/description
- **Details**: How it works (1-2 sentences)
- **Files**: Key files involved (if mentioned)

Rules:
- Only extract patterns that are non-obvious and specific to this codebase. Skip generic best practices.
- Maximum 4 patterns. If fewer are significant, extract fewer.
- If no significant patterns were discovered, respond with exactly: "No significant patterns."

---
CONVERSATION:
{}
---

Extract patterns:"#,
        truncate_for_llm(conversation_text)
    )
}

pub fn preferences_prompt(conversation_text: &str) -> String {
    format!(
        r#"Analyze this Claude Code conversation and extract clear user preferences and workflow habits.

Look for:
- Preferred tools, languages, or frameworks
- Coding style preferences
- Workflow preferences (how they like to work)
- Communication preferences

Rules:
- Only include preferences that are explicitly stated or strongly implied. Skip guesses.
- Maximum 5 preferences, one bullet point each (under 15 words).
- If no clear preferences are evident, respond with exactly: "No clear preferences."

---
CONVERSATION:
{}
---

Extract preferences:"#,
        truncate_for_llm(conversation_text)
    )
}

pub fn bugs_prompt(conversation_text: &str) -> String {
    format!(
        r#"Analyze this Claude Code conversation and extract real bugs or defects that were encountered.

For each bug, write:
- **Bug**: What went wrong (1 sentence)
- **Root cause**: Why it happened (if clear, 1 sentence)
- **Fix**: How it was resolved (if resolved, 1 sentence)

Rules:
- Only include real bugs with concrete details. Skip expected behavior, vague complaints, or config issues.
- Maximum 5 bugs.
- If no real bugs were encountered, respond with exactly: "No bugs encountered."

---
CONVERSATION:
{}
---

Extract bugs:"#,
        truncate_for_llm(conversation_text)
    )
}

pub fn insights_prompt(conversation_text: &str) -> String {
    format!(
        r#"Analyze this Claude Code conversation and extract non-obvious insights or key realizations.

For each insight, write:
- **Insight**: The non-obvious realization (1 sentence)
- **Context**: When/why this matters (1 sentence)

Rules:
- Only include things genuinely surprising or counterintuitive — not standard practices.
- Maximum 3 insights. High bar: if nothing is truly non-obvious, extract nothing.
- If no significant insights were found, respond with exactly: "No significant insights."

---
CONVERSATION:
{}
---

Extract insights:"#,
        truncate_for_llm(conversation_text)
    )
}

pub fn questions_prompt(conversation_text: &str) -> String {
    format!(
        r#"Analyze this Claude Code conversation and extract genuinely unresolved open questions.

For each question, write:
- **Open question**: What is still unclear (1 sentence)
- **Context**: Why this question matters (1 sentence)

Rules:
- Only include questions that were explicitly left open. Skip rhetorical questions and answered questions.
- Maximum 3 questions.
- If no open questions remain, respond with exactly: "No open questions."

---
CONVERSATION:
{}
---

Extract questions:"#,
        truncate_for_llm(conversation_text)
    )
}

pub fn summary_prompt(conversation_text: &str) -> String {
    format!(
        r#"Summarize this Claude Code conversation in 2-3 sentences. Focus on:
- What was the main goal/task
- What was accomplished
- Any notable outcomes

Be specific and concise.

---
CONVERSATION:
{}
---

Summary:"#,
        truncate_for_llm(conversation_text)
    )
}

#[allow(clippy::too_many_arguments)]
pub fn context_prompt(
    project_name: &str,
    decisions: &str,
    solutions: &str,
    patterns: &str,
    bugs: &str,
    insights: &str,
    questions: &str,
    summaries: &str,
) -> String {
    // Build optional sections only when there's non-trivial content
    let bugs_section = if bugs.trim().is_empty() || bugs.trim() == "No bugs encountered." {
        String::new()
    } else {
        format!("\nKNOWN BUGS FROM SESSIONS:\n{bugs}\n")
    };
    let insights_section =
        if insights.trim().is_empty() || insights.trim() == "No significant insights." {
            String::new()
        } else {
            format!("\nINSIGHTS FROM SESSIONS:\n{insights}\n")
        };
    let questions_section =
        if questions.trim().is_empty() || questions.trim() == "No open questions." {
            String::new()
        } else {
            format!("\nOPEN QUESTIONS FROM SESSIONS:\n{questions}\n")
        };

    format!(
        r#"Generate a concise project context summary for "{project_name}" to give Claude context in future sessions.

Format as markdown with these sections (omit any section that has no content):
## What This Project Is
(1-2 sentences)

## Key Decisions
(3-5 bullet points, most impactful only)

## Current State
(What's working, what's in progress — 3-5 bullets)

## Patterns & Conventions
(Non-obvious codebase patterns to know — 3-5 bullets)

## Known Issues & Solutions
(Recurring problems and fixes — only if substantive)

## Open Questions
(Unresolved questions — only if any exist)

Rules: be terse. Each bullet max 20 words. Skip generic advice. Only include what would change how a developer approaches this project.

---
DECISIONS:
{decisions}

SOLUTIONS:
{solutions}

PATTERNS:
{patterns}
{bugs_section}{insights_section}{questions_section}
SESSION SUMMARIES:
{summaries}
---

Generate context for {project_name}:"#
    )
}

pub const SYSTEM_QA_ASSISTANT: &str = "You are a precise Q&A assistant over a developer's \
    project knowledge base. Answer concisely from the provided knowledge only. \
    Cite 1-3 session IDs used. \
    If the answer is not found in the knowledge, say exactly: 'Not found in knowledge base.'";

pub fn ask_prompt(question: &str, context: &str) -> String {
    format!(
        "QUESTION: {question}\n\nKNOWLEDGE:\n{context}\n\n\
         Answer the question based only on the knowledge above. \
         Start your answer directly, then end with:\n\
         Sources: <comma-separated session IDs used>"
    )
}

pub const SYSTEM_CONTRADICTION_CHECKER: &str =
    "You are a knowledge consistency checker. Compare new knowledge entries against \
     existing ones and identify direct contradictions — cases where the new entry \
     states the opposite of an existing entry. Be concise. Only flag real contradictions, \
     not refinements or additions.";

pub fn contradiction_check_prompt(new_entries: &str, existing_entries: &str) -> String {
    format!(
        "NEW ENTRIES:\n{new_entries}\n\nEXISTING KNOWLEDGE:\n{existing_entries}\n\n\
         List any direct contradictions as:\n\
         - [category:session_id] CONTRADICTS [category:session_id]: <brief description>\n\
         If no contradictions found, respond exactly: 'No contradictions detected.'"
    )
}

pub fn summarize_stale_prompt(category: &str, entries_text: &str) -> String {
    format!(
        "These are old {category} knowledge entries being retired. \
         Condense them into a single concise summary, preserving any insight \
         that might still be relevant. Aim for 3-5 bullet points.\n\n\
         ENTRIES:\n{entries_text}\n\nSummary:"
    )
}

/// Truncate conversation text to fit within LLM context limits
fn truncate_for_llm(text: &str) -> &str {
    // Keep roughly 12k chars to leave room for prompt + response in small models
    const MAX_CHARS: usize = 12_000;
    if text.len() <= MAX_CHARS {
        text
    } else {
        let mut idx = MAX_CHARS;
        while idx > 0 && !text.is_char_boundary(idx) {
            idx -= 1;
        }
        &text[..idx]
    }
}
