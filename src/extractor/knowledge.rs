use crate::config::Config;
use crate::error::Result;
use crate::llm::client::LlmClient;
use crate::llm::prompts;
use crate::parser::conversation::Conversation;

/// Extract knowledge from a conversation and merge into project knowledge files
pub async fn extract_and_merge_knowledge(
    config: &Config,
    project_name: &str,
    conversation: &Conversation,
) -> Result<()> {
    let client = LlmClient::new(&config.llm);

    // Build a text representation of the conversation for LLM input
    let conv_text = conversation_to_text(conversation);

    if conv_text.trim().is_empty() {
        return Ok(());
    }

    // Extract different knowledge types in sequence (be gentle on local models)
    let decisions = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::decisions_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let solutions = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::solutions_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let patterns = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::patterns_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let preferences = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::preferences_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let summary = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::summary_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    // Write to knowledge directory
    let knowledge_dir = config.memory_dir.join("knowledge").join(project_name);
    std::fs::create_dir_all(&knowledge_dir)?;

    // Append to per-project knowledge files
    let session_header = format!(
        "\n\n## Session: {} ({})\n\n",
        conversation.session_id,
        conversation.start_time.as_deref().unwrap_or("unknown date")
    );

    append_knowledge(&knowledge_dir.join("decisions.md"), &session_header, &decisions)?;
    append_knowledge(&knowledge_dir.join("solutions.md"), &session_header, &solutions)?;
    append_knowledge(&knowledge_dir.join("patterns.md"), &session_header, &patterns)?;

    // Global preferences
    let global_dir = config.memory_dir.join("knowledge").join("_global");
    std::fs::create_dir_all(&global_dir)?;
    append_knowledge(&global_dir.join("preferences.md"), &session_header, &preferences)?;

    // Write summary
    let summary_dir = config.memory_dir.join("summaries").join(project_name);
    std::fs::create_dir_all(&summary_dir)?;
    let summary_with_meta = format!(
        "# {} - {}\n\n**Date:** {}\n\n{}\n",
        project_name,
        conversation.session_id,
        conversation.start_time.as_deref().unwrap_or("unknown"),
        summary
    );
    std::fs::write(
        summary_dir.join(format!("{}.md", conversation.session_id)),
        &summary_with_meta,
    )?;

    // Generate context.md — the key output
    // Read all existing knowledge to synthesize
    let all_decisions = read_or_default(&knowledge_dir.join("decisions.md"));
    let all_solutions = read_or_default(&knowledge_dir.join("solutions.md"));
    let all_patterns = read_or_default(&knowledge_dir.join("patterns.md"));
    let all_summaries = collect_summaries(&summary_dir)?;

    let context = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::context_prompt(
                project_name,
                &all_decisions,
                &all_solutions,
                &all_patterns,
                &all_summaries,
            ),
        )
        .await
        .unwrap_or_else(|_| {
            // Fallback: simple concatenation
            format!(
                "# {} - Project Context\n\n## Summary\n{}\n\n## Key Decisions\n{}\n\n## Patterns\n{}\n",
                project_name, summary, decisions, patterns
            )
        });

    let context_with_header = format!("# {} - Project Context\n\n{}\n", project_name, context);
    std::fs::write(knowledge_dir.join("context.md"), &context_with_header)?;

    Ok(())
}

fn conversation_to_text(conv: &Conversation) -> String {
    let mut text = String::with_capacity(4096);

    for turn in &conv.turns {
        text.push_str("USER: ");
        // Limit user message to avoid flooding
        let user = if turn.user_text.len() > 1000 {
            &turn.user_text[..1000]
        } else {
            &turn.user_text
        };
        text.push_str(user);
        text.push('\n');

        // Include tool names but not full output
        for tool in &turn.tool_interactions {
            text.push_str(&format!("[Tool: {} -> {}]\n", tool.tool_name, tool.input_summary));
        }

        if !turn.assistant_text.is_empty() {
            text.push_str("ASSISTANT: ");
            let assistant = if turn.assistant_text.len() > 1500 {
                &turn.assistant_text[..1500]
            } else {
                &turn.assistant_text
            };
            text.push_str(assistant);
            text.push('\n');
        }

        text.push('\n');
    }

    text
}

fn append_knowledge(path: &std::path::Path, header: &str, content: &str) -> Result<()> {
    use std::io::Write;

    // Initialize file with title if it doesn't exist
    if !path.exists() {
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Knowledge");
        std::fs::write(path, format!("# {}\n", capitalize(title)))?;
    }

    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    write!(file, "{}{}\n", header, content)?;

    Ok(())
}

fn read_or_default(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn collect_summaries(dir: &std::path::Path) -> Result<String> {
    let mut summaries = String::new();

    if !dir.exists() {
        return Ok(summaries);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().extension().map_or(false, |e| e == "md") {
            let content = std::fs::read_to_string(entry.path())?;
            summaries.push_str(&content);
            summaries.push('\n');
        }
    }

    Ok(summaries)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
