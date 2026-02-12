use super::{KnowledgeGraph, RelationType, Relationship};
use petgraph::Direction;

/// Find concepts related to a given concept within N hops
pub fn find_related(
    graph: &KnowledgeGraph,
    concept_id: &str,
    max_depth: usize,
) -> Vec<(String, usize)> {
    let (pg, node_map) = graph.to_petgraph();

    let start_idx = match node_map.get(concept_id) {
        Some(&idx) => idx,
        None => return Vec::new(),
    };

    let mut results = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((start_idx, 0));

    while let Some((idx, depth)) = queue.pop_front() {
        if depth > max_depth || visited.contains(&idx) {
            continue;
        }
        visited.insert(idx);

        if depth > 0 {
            // Add this node to results
            if let Some(concept) = pg.node_weight(idx) {
                results.push((concept.id.clone(), depth));
            }
        }

        // Explore neighbors
        if depth < max_depth {
            // Outgoing edges
            for neighbor in pg.neighbors_directed(idx, Direction::Outgoing) {
                queue.push_back((neighbor, depth + 1));
            }
            // Incoming edges (bidirectional traversal)
            for neighbor in pg.neighbors_directed(idx, Direction::Incoming) {
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    results
}

/// Find shortest path between two concepts
pub fn shortest_path(graph: &KnowledgeGraph, from: &str, to: &str) -> Option<Vec<String>> {
    let (pg, node_map) = graph.to_petgraph();

    let start_idx = node_map.get(from)?;
    let end_idx = node_map.get(to)?;

    petgraph::algo::astar(&pg, *start_idx, |n| n == *end_idx, |_| 1, |_| 0).map(|(_, path)| {
        path.iter()
            .filter_map(|&idx| pg.node_weight(idx))
            .map(|c| c.name.clone())
            .collect()
    })
}

/// Find most connected concepts (hubs)
pub fn find_hubs(graph: &KnowledgeGraph, top_n: usize) -> Vec<(String, usize, usize)> {
    let (pg, _) = graph.to_petgraph();

    let mut degree_counts: Vec<_> = pg
        .node_indices()
        .filter_map(|idx| {
            pg.node_weight(idx).map(|concept| {
                let in_degree = pg.edges_directed(idx, Direction::Incoming).count();
                let out_degree = pg.edges_directed(idx, Direction::Outgoing).count();
                (concept.id.clone(), in_degree, out_degree)
            })
        })
        .collect();

    degree_counts.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)));
    degree_counts.truncate(top_n);

    degree_counts
}

/// Find concepts by category
pub fn concepts_by_category(
    graph: &KnowledgeGraph,
    category: &super::ConceptCategory,
) -> Vec<String> {
    graph
        .concepts
        .values()
        .filter(|c| &c.category == category)
        .map(|c| c.id.clone())
        .collect()
}

/// Find all relationships of a specific type
pub fn relationships_by_type(graph: &KnowledgeGraph, rel_type: RelationType) -> Vec<Relationship> {
    graph
        .relationships
        .iter()
        .filter(|r| r.rel_type == rel_type)
        .cloned()
        .collect()
}

/// Get strongly connected components (concept clusters)
pub fn find_clusters(graph: &KnowledgeGraph) -> Vec<Vec<String>> {
    let (pg, _) = graph.to_petgraph();

    let sccs = petgraph::algo::kosaraju_scc(&pg);

    sccs.into_iter()
        .map(|component| {
            component
                .into_iter()
                .filter_map(|idx| pg.node_weight(idx))
                .map(|c| c.name.clone())
                .collect()
        })
        .filter(|cluster: &Vec<String>| cluster.len() > 1)
        .collect()
}
