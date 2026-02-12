use super::{Concept, ConceptCategory, KnowledgeGraph, Relationship, RelationType};
use crate::config::Config;
use crate::error::Result;
use crate::llm::client::LlmClient;

const GRAPH_EXTRACTION_PROMPT: &str = r#"You are a knowledge graph extractor. Analyze the provided knowledge and extract:

1. **Concepts**: Key ideas, technologies, patterns, decisions
2. **Relationships**: How concepts connect to each other

Output ONLY valid JSON in this exact format:
{
  "concepts": [
    {
      "id": "oauth",
      "name": "OAuth 2.0",
      "category": "technology",
      "description": "Authentication protocol",
      "importance": 0.9
    }
  ],
  "relationships": [
    {
      "from": "authentication",
      "to": "oauth",
      "type": "implements",
      "strength": 0.8
    }
  ]
}

Categories: technology, pattern, decision, problem, solution, person, tool, other
Relationship types: implements, uses, relates-to, causes, part-of, depends-on, supersedes, contradicts
Strength: 0.0 (weak) to 1.0 (strong)
Importance: 0.0 (trivial) to 1.0 (critical)

Extract 10-30 most important concepts and their relationships."#;

pub async fn build_graph_from_knowledge(
    config: &Config,
    project: &str,
    knowledge_content: &str,
) -> Result<KnowledgeGraph> {
    let client = LlmClient::new(&config.llm);

    let prompt = format!(
        "Project: {}\n\nKnowledge to analyze:\n\n{}",
        project, knowledge_content
    );

    let response = client.chat(GRAPH_EXTRACTION_PROMPT, &prompt).await?;

    // Try to extract JSON from response (LLM might add markdown formatting)
    let json_str = if response.contains("```json") {
        // Extract JSON from markdown code block
        response
            .split("```json")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap_or(&response)
            .trim()
    } else if response.contains("```") {
        // Extract from generic code block
        response
            .split("```")
            .nth(1)
            .unwrap_or(&response)
            .trim()
    } else {
        response.trim()
    };

    // Parse LLM response as JSON
    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| {
            eprintln!("LLM response:\n{}", &response[..response.len().min(500)]);
            crate::error::MemoryError::Config(format!("Failed to parse graph JSON: {}. Try a better model (--provider anthropic)", e))
        })?;

    let mut graph = KnowledgeGraph::new(project.to_string());

    // Extract concepts
    if let Some(concepts_array) = parsed.get("concepts").and_then(|c| c.as_array()) {
        for concept_json in concepts_array {
            if let (Some(id), Some(name)) = (
                concept_json.get("id").and_then(|i| i.as_str()),
                concept_json.get("name").and_then(|n| n.as_str()),
            ) {
                let category = concept_json
                    .get("category")
                    .and_then(|c| c.as_str())
                    .and_then(parse_category)
                    .unwrap_or(ConceptCategory::Other);

                let description = concept_json
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string());

                let importance = concept_json
                    .get("importance")
                    .and_then(|i| i.as_f64())
                    .unwrap_or(0.5) as f32;

                graph.add_concept(Concept {
                    id: id.to_string(),
                    name: name.to_string(),
                    category,
                    description,
                    source_sessions: Vec::new(),
                    importance,
                });
            }
        }
    }

    // Extract relationships
    if let Some(rels_array) = parsed.get("relationships").and_then(|r| r.as_array()) {
        for rel_json in rels_array {
            if let (Some(from), Some(to), Some(rel_type_str)) = (
                rel_json.get("from").and_then(|f| f.as_str()),
                rel_json.get("to").and_then(|t| t.as_str()),
                rel_json.get("type").and_then(|t| t.as_str()),
            ) {
                if let Some(rel_type) = RelationType::from_str(rel_type_str) {
                    let strength = rel_json
                        .get("strength")
                        .and_then(|s| s.as_f64())
                        .unwrap_or(0.5) as f32;

                    graph.add_relationship(Relationship {
                        from: from.to_string(),
                        to: to.to_string(),
                        rel_type,
                        strength,
                        source_sessions: Vec::new(),
                    });
                }
            }
        }
    }

    Ok(graph)
}

fn parse_category(s: &str) -> Option<ConceptCategory> {
    match s.to_lowercase().as_str() {
        "technology" | "tech" => Some(ConceptCategory::Technology),
        "pattern" => Some(ConceptCategory::Pattern),
        "decision" => Some(ConceptCategory::Decision),
        "problem" => Some(ConceptCategory::Problem),
        "solution" => Some(ConceptCategory::Solution),
        "person" | "people" => Some(ConceptCategory::Person),
        "tool" => Some(ConceptCategory::Tool),
        _ => Some(ConceptCategory::Other),
    }
}
