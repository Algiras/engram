pub const SYSTEM_KNOWLEDGE_EXTRACTOR: &str = r#"You are a knowledge extraction assistant. You analyze software development conversations and extract structured knowledge. Be concise and factual. Only extract what is clearly stated or demonstrated in the conversation."#;

pub fn decisions_prompt(conversation_text: &str) -> String {
    format!(
        r#"Analyze this Claude Code conversation and extract key technical decisions that were made.

For each decision, write:
- **Decision**: What was decided
- **Context**: Why it was decided (if clear)
- **Alternatives**: What was considered (if mentioned)

Only include clear, actionable decisions. Skip trivial choices.
If no significant decisions were made, respond with "No significant decisions."

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
        r#"Analyze this Claude Code conversation and extract problems that were solved.

For each problem-solution pair, write:
- **Problem**: What issue was encountered
- **Solution**: How it was resolved
- **Key insight**: The crucial realization (if any)

Focus on problems that might recur. Skip trivial fixes.
If no significant problems were solved, respond with "No significant problems solved."

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
- **Pattern**: Name/description of the pattern
- **Details**: How it works
- **Files**: Key files involved (if mentioned)

Focus on patterns someone would need to know when working on this codebase.
If no significant patterns were discovered, respond with "No significant patterns."

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
        r#"Analyze this Claude Code conversation and extract user preferences and workflow habits.

Look for:
- Preferred tools, languages, or frameworks
- Coding style preferences
- Workflow preferences (how they like to work)
- Communication preferences (how they interact with Claude)

For each preference, write a concise bullet point.
If no clear preferences are evident, respond with "No clear preferences."

---
CONVERSATION:
{}
---

Extract preferences:"#,
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

pub fn context_prompt(
    project_name: &str,
    decisions: &str,
    solutions: &str,
    patterns: &str,
    summaries: &str,
) -> String {
    format!(
        r#"Generate a project context summary for "{project_name}" based on the extracted knowledge below. This summary will be used to give Claude context in future sessions.

Format it as markdown with these sections:
## What This Project Is
(1-2 sentences about the project)

## Key Decisions
(Bullet points of important decisions)

## Current State
(What's been done, what's working)

## Patterns
(Codebase patterns and conventions to know)

## Common Issues
(Problems that came up and their solutions)

Be concise and practical. Only include information that would be useful for future development sessions.

---
DECISIONS FROM SESSIONS:
{decisions}

SOLUTIONS FROM SESSIONS:
{solutions}

PATTERNS FROM SESSIONS:
{patterns}

SESSION SUMMARIES:
{summaries}
---

Generate context for {project_name}:"#
    )
}

/// Truncate conversation text to fit within LLM context limits
fn truncate_for_llm(text: &str) -> &str {
    // Keep roughly 12k chars to leave room for prompt + response in small models
    const MAX_CHARS: usize = 12_000;
    if text.len() <= MAX_CHARS {
        text
    } else {
        &text[..MAX_CHARS]
    }
}
