pub mod builder;
pub mod query;
pub mod viz;

use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of relationships between concepts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationType {
    /// A implements B
    Implements,
    /// A uses B
    Uses,
    /// A relates to B
    RelatesTo,
    /// A causes B
    Causes,
    /// A is part of B
    PartOf,
    /// A depends on B
    DependsOn,
    /// A supersedes/replaces B
    Supersedes,
    /// A contradicts B
    Contradicts,
}

impl RelationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RelationType::Implements => "implements",
            RelationType::Uses => "uses",
            RelationType::RelatesTo => "relates-to",
            RelationType::Causes => "causes",
            RelationType::PartOf => "part-of",
            RelationType::DependsOn => "depends-on",
            RelationType::Supersedes => "supersedes",
            RelationType::Contradicts => "contradicts",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "implements" => Some(RelationType::Implements),
            "uses" => Some(RelationType::Uses),
            "relates-to" | "relates_to" | "related" => Some(RelationType::RelatesTo),
            "causes" => Some(RelationType::Causes),
            "part-of" | "part_of" => Some(RelationType::PartOf),
            "depends-on" | "depends_on" | "requires" => Some(RelationType::DependsOn),
            "supersedes" | "replaces" => Some(RelationType::Supersedes),
            "contradicts" | "conflicts" => Some(RelationType::Contradicts),
            _ => None,
        }
    }
}

/// A concept node in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: String,
    pub name: String,
    pub category: ConceptCategory,
    pub description: Option<String>,
    pub source_sessions: Vec<String>,
    pub importance: f32,  // 0.0 - 1.0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConceptCategory {
    Technology,
    Pattern,
    Decision,
    Problem,
    Solution,
    Person,
    Tool,
    Other,
}

/// An edge in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub rel_type: RelationType,
    pub strength: f32,  // 0.0 - 1.0 (like synaptic weight!)
    pub source_sessions: Vec<String>,
}

/// Knowledge graph for a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    pub project: String,
    pub concepts: HashMap<String, Concept>,
    pub relationships: Vec<Relationship>,
    pub created_at: String,
    pub updated_at: String,
}

impl KnowledgeGraph {
    pub fn new(project: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            project,
            concepts: HashMap::new(),
            relationships: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn add_concept(&mut self, concept: Concept) {
        self.concepts.insert(concept.id.clone(), concept);
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn add_relationship(&mut self, relationship: Relationship) {
        self.relationships.push(relationship);
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Build a petgraph DiGraph for querying
    pub fn to_petgraph(&self) -> (DiGraph<&Concept, RelationType>, HashMap<String, NodeIndex>) {
        let mut graph = DiGraph::new();
        let mut node_map = HashMap::new();

        // Add nodes
        for (id, concept) in &self.concepts {
            let idx = graph.add_node(concept);
            node_map.insert(id.clone(), idx);
        }

        // Add edges
        for rel in &self.relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (node_map.get(&rel.from), node_map.get(&rel.to)) {
                graph.add_edge(from_idx, to_idx, rel.rel_type);
            }
        }

        (graph, node_map)
    }

    /// Save graph to JSON file
    pub fn save(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load graph from JSON file
    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&json)?)
    }
}
