use super::{ConceptCategory, KnowledgeGraph, RelationType};

/// Generate DOT format for graphviz visualization
pub fn to_dot(graph: &KnowledgeGraph) -> String {
    let mut dot = String::new();

    dot.push_str(&format!("digraph \"{}\" {{\n", graph.project));
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  node [shape=box, style=rounded];\n");
    dot.push_str("  \n");

    // Add nodes with colors based on category
    for (id, concept) in &graph.concepts {
        let color = category_color(&concept.category);
        let importance_width = 1.0 + (concept.importance * 3.0);
        let label = if let Some(ref desc) = concept.description {
            format!("{}\\n{}", concept.name, truncate(desc, 40))
        } else {
            concept.name.clone()
        };

        dot.push_str(&format!(
            "  \"{}\" [label=\"{}\", fillcolor=\"{}\", style=\"filled,rounded\", penwidth={}];\n",
            id, label, color, importance_width
        ));
    }

    dot.push_str("  \n");

    // Add edges with labels and colors
    for rel in &graph.relationships {
        let color = relation_color(&rel.rel_type);
        let style = if rel.strength > 0.7 {
            "bold"
        } else if rel.strength > 0.4 {
            "solid"
        } else {
            "dashed"
        };

        dot.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\", color=\"{}\", style=\"{}\"];\n",
            rel.from,
            rel.to,
            rel.rel_type.as_str(),
            color,
            style
        ));
    }

    dot.push_str("  \n");

    // Add legend
    dot.push_str("  subgraph cluster_legend {\n");
    dot.push_str("    label=\"Legend\";\n");
    dot.push_str("    style=dashed;\n");
    dot.push_str("    node [shape=plaintext];\n");
    dot.push_str("    legend [label=<\n");
    dot.push_str("      <table border='0' cellborder='1' cellspacing='0'>\n");
    dot.push_str("        <tr><td colspan='2'><b>Node Categories</b></td></tr>\n");
    dot.push_str("        <tr><td bgcolor='lightblue'>Technology</td></tr>\n");
    dot.push_str("        <tr><td bgcolor='lightgreen'>Pattern</td></tr>\n");
    dot.push_str("        <tr><td bgcolor='lightyellow'>Decision</td></tr>\n");
    dot.push_str("        <tr><td bgcolor='lightcoral'>Problem</td></tr>\n");
    dot.push_str("        <tr><td bgcolor='palegreen'>Solution</td></tr>\n");
    dot.push_str("      </table>\n");
    dot.push_str("    >];\n");
    dot.push_str("  }\n");

    dot.push_str("}\n");

    dot
}

/// Generate ASCII art visualization for terminal
pub fn to_ascii(graph: &KnowledgeGraph, root_concept: Option<&str>) -> String {
    let mut output = String::new();

    if let Some(root) = root_concept {
        // Show tree starting from root
        if let Some(concept) = graph.concepts.get(root) {
            output.push_str(&format!("📊 Knowledge Graph: {}\n\n", concept.name));
            output.push_str(&format!("🔵 {} (importance: {:.1})\n", concept.name, concept.importance));

            // Find relationships
            let outgoing: Vec<_> = graph
                .relationships
                .iter()
                .filter(|r| r.from == root)
                .collect();

            for (i, rel) in outgoing.iter().enumerate() {
                let is_last = i == outgoing.len() - 1;
                let prefix = if is_last { "└──" } else { "├──" };
                let arrow = relation_symbol(&rel.rel_type);

                if let Some(target) = graph.concepts.get(&rel.to) {
                    output.push_str(&format!(
                        "{} {} {} ({})\n",
                        prefix, arrow, target.name, rel.rel_type.as_str()
                    ));
                }
            }
        }
    } else {
        // Show all concepts grouped by category
        output.push_str(&format!("📊 Knowledge Graph: {}\n\n", graph.project));
        output.push_str(&format!("Concepts: {}, Relationships: {}\n\n", graph.concepts.len(), graph.relationships.len()));

        let categories = [
            ConceptCategory::Technology,
            ConceptCategory::Pattern,
            ConceptCategory::Decision,
            ConceptCategory::Solution,
            ConceptCategory::Problem,
        ];

        for category in &categories {
            let concepts: Vec<_> = graph
                .concepts
                .values()
                .filter(|c| &c.category == category)
                .collect();

            if !concepts.is_empty() {
                output.push_str(&format!("{:?}:\n", category));
                for concept in concepts {
                    output.push_str(&format!("  • {} (importance: {:.1})\n", concept.name, concept.importance));
                }
                output.push_str("\n");
            }
        }
    }

    output
}

fn category_color(category: &ConceptCategory) -> &'static str {
    match category {
        ConceptCategory::Technology => "lightblue",
        ConceptCategory::Pattern => "lightgreen",
        ConceptCategory::Decision => "lightyellow",
        ConceptCategory::Problem => "lightcoral",
        ConceptCategory::Solution => "palegreen",
        ConceptCategory::Person => "lavender",
        ConceptCategory::Tool => "lightcyan",
        ConceptCategory::Other => "white",
    }
}

fn relation_color(rel_type: &RelationType) -> &'static str {
    match rel_type {
        RelationType::Implements => "blue",
        RelationType::Uses => "green",
        RelationType::RelatesTo => "gray",
        RelationType::Causes => "red",
        RelationType::PartOf => "purple",
        RelationType::DependsOn => "orange",
        RelationType::Supersedes => "brown",
        RelationType::Contradicts => "darkred",
    }
}

fn relation_symbol(rel_type: &RelationType) -> &'static str {
    match rel_type {
        RelationType::Implements => "▶",
        RelationType::Uses => "→",
        RelationType::RelatesTo => "↔",
        RelationType::Causes => "⇒",
        RelationType::PartOf => "⊂",
        RelationType::DependsOn => "⇐",
        RelationType::Supersedes => "⊗",
        RelationType::Contradicts => "⚠",
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
