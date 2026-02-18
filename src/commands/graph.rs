use std::path::Path;

use colored::Colorize;

use crate::config::Config;
use crate::error::{MemoryError, Result};
use crate::graph;

pub fn cmd_graph_build(config: &Config, project: &str) -> Result<()> {
    use crate::extractor::knowledge::{
        parse_session_blocks, partition_by_expiry, reconstruct_blocks,
    };

    let knowledge_dir = config.memory_dir.join("knowledge").join(project);

    if !knowledge_dir.exists() {
        eprintln!(
            "{} No knowledge found for '{}'. Run 'ingest' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    println!(
        "{} Building knowledge graph for '{}'...",
        "Analyzing".green().bold(),
        project
    );

    // Read knowledge files
    let read_and_filter = |path: &Path| -> String {
        if !path.exists() {
            return String::new();
        }
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, _) = partition_by_expiry(blocks);
        reconstruct_blocks(&preamble, &active)
    };

    let mut knowledge_content = String::new();
    knowledge_content.push_str(&read_and_filter(&knowledge_dir.join("context.md")));
    knowledge_content.push_str("\n\n");
    knowledge_content.push_str(&read_and_filter(&knowledge_dir.join("decisions.md")));
    knowledge_content.push_str("\n\n");
    knowledge_content.push_str(&read_and_filter(&knowledge_dir.join("solutions.md")));
    knowledge_content.push_str("\n\n");
    knowledge_content.push_str(&read_and_filter(&knowledge_dir.join("patterns.md")));

    if knowledge_content.trim().is_empty() {
        eprintln!("{} No knowledge content to analyze", "Error:".red());
        return Ok(());
    }

    // Build graph using LLM
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    let graph = rt.block_on(async {
        graph::builder::build_graph_from_knowledge(config, project, &knowledge_content).await
    })?;

    // Save graph
    let graph_path = knowledge_dir.join("graph.json");
    graph
        .save(&graph_path)
        .map_err(|e| MemoryError::Config(format!("Failed to save graph: {}", e)))?;

    println!("{} Knowledge graph created:", "Done!".green().bold());
    println!("  Concepts: {}", graph.concepts.len());
    println!("  Relationships: {}", graph.relationships.len());
    println!("  Saved to: {}", graph_path.display().to_string().cyan());
    println!("\nExplore with:");
    println!(
        "  {}",
        format!("engram graph query {} <concept>", project).cyan()
    );
    println!("  {}", format!("engram graph viz {} ascii", project).cyan());

    Ok(())
}

pub fn cmd_graph_query(config: &Config, project: &str, concept: &str, depth: usize) -> Result<()> {
    let graph_path = config
        .memory_dir
        .join("knowledge")
        .join(project)
        .join("graph.json");

    if !graph_path.exists() {
        eprintln!(
            "{} No graph found for '{}'. Run 'graph build' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let graph = graph::KnowledgeGraph::load(&graph_path)
        .map_err(|e| MemoryError::Config(format!("Failed to load graph: {}", e)))?;

    let related = graph::query::find_related(&graph, concept, depth);

    if related.is_empty() {
        println!(
            "{} No concepts found related to '{}'",
            "Not found:".yellow(),
            concept
        );
        return Ok(());
    }

    println!(
        "{} Concepts related to '{}' (depth {}):\n",
        "Graph Query".green().bold(),
        concept,
        depth
    );

    for (concept_id, dist) in related {
        if let Some(c) = graph.concepts.get(&concept_id) {
            let indent = "  ".repeat(dist);
            println!(
                "{}[{}] {} (importance: {:.1})",
                indent,
                dist,
                c.name.cyan(),
                c.importance
            );
        }
    }

    Ok(())
}

pub fn cmd_graph_viz(
    config: &Config,
    project: &str,
    format: &str,
    output: Option<&str>,
    root: Option<&str>,
) -> Result<()> {
    let graph_path = config
        .memory_dir
        .join("knowledge")
        .join(project)
        .join("graph.json");

    if !graph_path.exists() {
        eprintln!(
            "{} No graph found for '{}'. Run 'graph build' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let graph = graph::KnowledgeGraph::load(&graph_path)
        .map_err(|e| MemoryError::Config(format!("Failed to load graph: {}", e)))?;

    let viz_content = match format {
        "dot" => graph::viz::to_dot(&graph),
        "ascii" => graph::viz::to_ascii(&graph, root),
        "svg" => {
            // Generate DOT and convert to SVG using graphviz
            let dot = graph::viz::to_dot(&graph);
            if let Some(out_path) = output {
                // Write DOT to temp file
                let temp_dot = "/tmp/graph.dot";
                std::fs::write(temp_dot, &dot)?;

                // Convert to SVG using dot command
                let status = std::process::Command::new("dot")
                    .args(["-Tsvg", temp_dot, "-o", out_path])
                    .status();

                match status {
                    Ok(s) if s.success() => {
                        println!("{} SVG created: {}", "Done!".green().bold(), out_path);
                        return Ok(());
                    }
                    _ => {
                        eprintln!(
                            "{} graphviz not installed. Install with: brew install graphviz",
                            "Error:".red()
                        );
                        eprintln!("Outputting DOT format instead...");
                        dot
                    }
                }
            } else {
                eprintln!(
                    "{} SVG requires --output. Showing DOT instead.",
                    "Note:".yellow()
                );
                dot
            }
        }
        _ => return Err(MemoryError::Config(format!("Unknown format: {}", format))),
    };

    if let Some(out_path) = output {
        std::fs::write(out_path, &viz_content)?;
        println!(
            "{} Visualization saved to {}",
            "Done!".green().bold(),
            out_path
        );
    } else {
        print!("{}", viz_content);
    }

    Ok(())
}

pub fn cmd_graph_path(config: &Config, project: &str, from: &str, to: &str) -> Result<()> {
    let graph_path = config
        .memory_dir
        .join("knowledge")
        .join(project)
        .join("graph.json");

    if !graph_path.exists() {
        eprintln!(
            "{} No graph found for '{}'. Run 'graph build' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let graph = graph::KnowledgeGraph::load(&graph_path)
        .map_err(|e| MemoryError::Config(format!("Failed to load graph: {}", e)))?;

    match graph::query::shortest_path(&graph, from, to) {
        Some(path) => {
            println!(
                "{} Path from '{}' to '{}':\n",
                "Found".green().bold(),
                from,
                to
            );
            for (i, concept) in path.iter().enumerate() {
                if i > 0 {
                    println!("   ↓");
                }
                println!("  [{}] {}", i, concept.cyan());
            }
        }
        None => {
            println!(
                "{} No path found from '{}' to '{}'",
                "Not found:".yellow(),
                from,
                to
            );
        }
    }

    Ok(())
}

pub fn cmd_graph_hubs(config: &Config, project: &str, top_n: usize) -> Result<()> {
    let graph_path = config
        .memory_dir
        .join("knowledge")
        .join(project)
        .join("graph.json");

    if !graph_path.exists() {
        eprintln!(
            "{} No graph found for '{}'. Run 'graph build' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let graph = graph::KnowledgeGraph::load(&graph_path)
        .map_err(|e| MemoryError::Config(format!("Failed to load graph: {}", e)))?;

    let hubs = graph::query::find_hubs(&graph, top_n);

    println!(
        "{} Top {} most connected concepts:\n",
        "Hubs".green().bold(),
        top_n
    );

    for (i, (concept_id, in_degree, out_degree)) in hubs.iter().enumerate() {
        if let Some(concept) = graph.concepts.get(concept_id) {
            println!(
                "  {}. {} ({} incoming, {} outgoing, importance: {:.1})",
                i + 1,
                concept.name.cyan(),
                in_degree,
                out_degree,
                concept.importance
            );
        }
    }

    Ok(())
}
