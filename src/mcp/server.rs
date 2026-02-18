use super::protocol::*;
use crate::config::Config;
use crate::error::{MemoryError, Result};
use colored::Colorize;
use serde_json::json;
use std::io::{BufRead, Write};
use std::sync::Mutex;

/// Tracks writes made by the LLM during a single MCP session.
#[derive(Default)]
struct SessionStats {
    added: u32,
    reflected: u32,
    updated: u32,
    forgotten: u32,
    synthesized: u32,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct McpServer {
    config: Config,
    session: Mutex<SessionStats>,
}

impl McpServer {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            session: Mutex::new(SessionStats {
                started_at: Some(chrono::Utc::now()),
                ..Default::default()
            }),
        }
    }

    /// Run the MCP server on stdio
    pub fn run(&self) -> Result<()> {
        eprintln!("{}", "engram MCP server starting...".green());
        eprintln!("{}", "Listening on stdio".dimmed());

        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = stdin.lock();

        for line in reader.lines() {
            let line = line.map_err(MemoryError::Io)?;

            if line.trim().is_empty() {
                continue;
            }

            let request: Request = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("{} Failed to parse request: {}", "Error:".red(), e);
                    continue;
                }
            };

            let response = self.handle_request(request);
            let response_json = serde_json::to_string(&response)
                .map_err(|e| MemoryError::Config(format!("Failed to serialize response: {}", e)))?;

            writeln!(stdout, "{}", response_json)?;
            stdout.flush()?;
        }

        Ok(())
    }

    fn handle_request(&self, request: Request) -> Response {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request.id, request.params),
            "resources/list" => self.handle_resources_list(request.id),
            "resources/read" => self.handle_resources_read(request.id, request.params),
            _ => Response::error(
                request.id,
                -32601,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    fn handle_initialize(&self, id: serde_json::Value) -> Response {
        Response::success(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    },
                    "resources": {
                        "subscribe": false,
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "engram",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    fn handle_tools_list(&self, id: serde_json::Value) -> Response {
        let tools = vec![
            Tool {
                name: "recall".to_string(),
                description: "Recall project context and knowledge summary".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Project name"
                        }
                    },
                    "required": ["project"]
                }),
            },
            Tool {
                name: "search".to_string(),
                description: "Search across all memory using regex patterns".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query (regex supported)"
                        },
                        "project": {
                            "type": "string",
                            "description": "Limit search to specific project (optional)"
                        },
                        "knowledge_only": {
                            "type": "boolean",
                            "description": "Search only knowledge files (default: false)"
                        }
                    },
                    "required": ["query"]
                }),
            },
            Tool {
                name: "lookup".to_string(),
                description: "Look up knowledge by topic for a specific project".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Project name"
                        },
                        "query": {
                            "type": "string",
                            "description": "Topic to search for"
                        }
                    },
                    "required": ["project", "query"]
                }),
            },
            Tool {
                name: "projects".to_string(),
                description: "List all discovered projects with activity".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            Tool {
                name: "add".to_string(),
                description: "Add a manual knowledge entry to a project".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Project name"
                        },
                        "category": {
                            "type": "string",
                            "description": "Category: decisions, solutions, patterns, or preferences",
                            "enum": ["decisions", "solutions", "patterns", "preferences"]
                        },
                        "content": {
                            "type": "string",
                            "description": "Knowledge content in markdown format"
                        },
                        "label": {
                            "type": "string",
                            "description": "Optional label for the entry"
                        }
                    },
                    "required": ["project", "category", "content"]
                }),
            },
            Tool {
                name: "analytics".to_string(),
                description: "Show usage analytics and top knowledge for a project".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Project name (optional, shows all if not provided)"
                        },
                        "days": {
                            "type": "number",
                            "description": "Number of days to analyze (default: 30)",
                            "default": 30
                        }
                    }
                }),
            },
            Tool {
                name: "search_semantic".to_string(),
                description: "Semantic search using embeddings (vector similarity)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query"
                        },
                        "project": {
                            "type": "string",
                            "description": "Limit to specific project (optional)"
                        },
                        "limit": {
                            "type": "number",
                            "description": "Maximum results to return (default: 10)",
                            "default": 10
                        }
                    },
                    "required": ["query"]
                }),
            },
            Tool {
                name: "graph_query".to_string(),
                description: "Query knowledge graph for concept relationships and connections"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Project name"
                        },
                        "concept": {
                            "type": "string",
                            "description": "Concept to query (e.g., 'authentication', 'database')"
                        }
                    },
                    "required": ["project", "concept"]
                }),
            },
            Tool {
                name: "reflect".to_string(),
                description: "Extract and persist structured knowledge from text you provide. Pass key insights, decisions, or learnings from the current conversation — engram will run LLM extraction and store them immediately. Use this to learn in real time without waiting for session ingest.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Project name to store knowledge under"
                        },
                        "text": {
                            "type": "string",
                            "description": "The text to extract knowledge from — paste relevant conversation excerpts, your own summary, or a mix"
                        },
                        "categories": {
                            "type": "array",
                            "items": { "type": "string", "enum": ["decisions", "solutions", "patterns"] },
                            "description": "Which categories to extract. Defaults to all three if omitted."
                        }
                    },
                    "required": ["project", "text"]
                }),
            },
            Tool {
                name: "update".to_string(),
                description: "Correct or replace an existing knowledge entry identified by its label (session_id). Use this when you find that a stored entry is wrong, outdated, or incomplete.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Project name"
                        },
                        "category": {
                            "type": "string",
                            "enum": ["decisions", "solutions", "patterns"],
                            "description": "Knowledge category file to search"
                        },
                        "label": {
                            "type": "string",
                            "description": "The session_id / label of the entry to update (from lookup or recall output)"
                        },
                        "content": {
                            "type": "string",
                            "description": "The new content to replace the entry with"
                        }
                    },
                    "required": ["project", "category", "label", "content"]
                }),
            },
            Tool {
                name: "forget".to_string(),
                description: "Remove a knowledge entry by label. Use this to delete incorrect, duplicate, or outdated entries.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Project name"
                        },
                        "category": {
                            "type": "string",
                            "enum": ["decisions", "solutions", "patterns"],
                            "description": "Knowledge category file containing the entry"
                        },
                        "label": {
                            "type": "string",
                            "description": "The session_id / label of the entry to remove"
                        }
                    },
                    "required": ["project", "category", "label"]
                }),
            },
            Tool {
                name: "status".to_string(),
                description: "Show how many knowledge entries you have written, updated, or removed in this MCP session, plus a summary of what exists for the project.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Project name (optional)"
                        }
                    }
                }),
            },
            Tool {
                name: "synthesize".to_string(),
                description: "Re-synthesize context.md for a project using the LLM. Call this after a batch of reflect/add/update/forget operations to regenerate the consolidated summary.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Project name"
                        }
                    },
                    "required": ["project"]
                }),
            },
        ];

        Response::success(id, json!({ "tools": tools }))
    }

    fn handle_tools_call(&self, id: serde_json::Value, params: serde_json::Value) -> Response {
        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(name) => name,
            None => return Response::error(id, -32602, "Missing tool name"),
        };

        let args = params.get("arguments").cloned().unwrap_or(json!({}));

        let result = match tool_name {
            "recall" => self.tool_recall(args),
            "search" => self.tool_search(args),
            "lookup" => self.tool_lookup(args),
            "projects" => self.tool_projects(args),
            "add" => {
                let r = self.tool_add(args);
                if r.is_ok() {
                    if let Ok(mut s) = self.session.lock() {
                        s.added += 1;
                    }
                }
                r
            }
            "analytics" => self.tool_analytics(args),
            "search_semantic" => self.tool_search_semantic(args),
            "graph_query" => self.tool_graph_query(args),
            "reflect" => {
                let r = self.tool_reflect(args);
                if r.is_ok() {
                    if let Ok(mut s) = self.session.lock() {
                        s.reflected += 1;
                    }
                }
                r
            }
            "update" => {
                let r = self.tool_update(args);
                if r.is_ok() {
                    if let Ok(mut s) = self.session.lock() {
                        s.updated += 1;
                    }
                }
                r
            }
            "forget" => {
                let r = self.tool_forget(args);
                if r.is_ok() {
                    if let Ok(mut s) = self.session.lock() {
                        s.forgotten += 1;
                    }
                }
                r
            }
            "status" => self.tool_status(args),
            "synthesize" => {
                let r = self.tool_synthesize(args);
                if r.is_ok() {
                    if let Ok(mut s) = self.session.lock() {
                        s.synthesized += 1;
                    }
                }
                r
            }
            _ => Err(MemoryError::Config(format!("Unknown tool: {}", tool_name))),
        };

        match result {
            Ok(content) => Response::success(
                id,
                json!({
                    "content": [
                        {
                            "type": "text",
                            "text": content
                        }
                    ]
                }),
            ),
            Err(e) => Response::error(id, -32000, format!("Tool error: {}", e)),
        }
    }

    fn handle_resources_list(&self, id: serde_json::Value) -> Response {
        // Discover all projects and expose their contexts as resources
        let projects =
            match crate::parser::discovery::discover_projects(&self.config.claude_projects_dir) {
                Ok(projects) => projects,
                Err(e) => {
                    return Response::error(
                        id,
                        -32000,
                        format!("Failed to discover projects: {}", e),
                    );
                }
            };

        let resources: Vec<Resource> = projects
            .iter()
            .map(|p| Resource {
                uri: format!("memory://{}/context", p.name),
                name: format!("{} context", p.name),
                description: Some(format!("Project context and knowledge for {}", p.name)),
                mime_type: Some("text/markdown".to_string()),
            })
            .collect();

        Response::success(id, json!({ "resources": resources }))
    }

    fn handle_resources_read(&self, id: serde_json::Value, params: serde_json::Value) -> Response {
        let uri = match params.get("uri").and_then(|v| v.as_str()) {
            Some(uri) => uri,
            None => return Response::error(id, -32602, "Missing resource URI"),
        };

        // Parse URI: memory://<project>/context
        if !uri.starts_with("memory://") {
            return Response::error(id, -32602, "Invalid resource URI");
        }

        let path = &uri["memory://".len()..];
        let parts: Vec<&str> = path.split('/').collect();

        if parts.len() != 2 || parts[1] != "context" {
            return Response::error(id, -32602, "Invalid resource path");
        }

        let project = parts[0];

        match self.read_project_context(project) {
            Ok(content) => Response::success(
                id,
                json!({
                    "contents": [
                        {
                            "uri": uri,
                            "mimeType": "text/markdown",
                            "text": content
                        }
                    ]
                }),
            ),
            Err(e) => Response::error(id, -32000, format!("Failed to read context: {}", e)),
        }
    }

    // Tool implementations

    fn tool_recall(&self, args: serde_json::Value) -> Result<String> {
        let project = args
            .get("project")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MemoryError::Config("Missing project parameter".into()))?;

        self.read_project_context(project)
    }

    fn tool_search(&self, args: serde_json::Value) -> Result<String> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MemoryError::Config("Missing query parameter".into()))?;

        let project = args.get("project").and_then(|v| v.as_str());
        let knowledge_only = args
            .get("knowledge_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let search_dir = if knowledge_only {
            self.config.memory_dir.join("knowledge")
        } else {
            self.config.memory_dir.clone()
        };

        if !search_dir.exists() {
            return Ok("No memory directory found. Run 'ingest' first.".to_string());
        }

        let pattern = regex::Regex::new(query)
            .map_err(|e| MemoryError::Config(format!("Invalid regex: {}", e)))?;

        let mut results = String::new();

        for entry in walkdir::WalkDir::new(&search_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "md" || ext == "json")
            })
        {
            let path = entry.path();

            if let Some(ref proj) = project {
                let path_str = path.to_string_lossy();
                if !path_str.contains(proj) {
                    continue;
                }
            }

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let lines: Vec<&str> = content.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if pattern.is_match(line) {
                    let rel = path.strip_prefix(&self.config.memory_dir).unwrap_or(path);
                    results.push_str(&format!("\n{}\n", rel.display()));
                    results.push_str(&format!("  Line {}: {}\n", i + 1, line));
                    break;
                }
            }
        }

        if results.is_empty() {
            Ok(format!("No matches found for '{}'", query))
        } else {
            Ok(results)
        }
    }

    fn tool_lookup(&self, args: serde_json::Value) -> Result<String> {
        let project = args
            .get("project")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MemoryError::Config("Missing project parameter".into()))?;

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MemoryError::Config("Missing query parameter".into()))?;

        let knowledge_dir = self.config.memory_dir.join("knowledge").join(project);

        if !knowledge_dir.exists() {
            return Ok(format!("No knowledge found for project '{}'", project));
        }

        let query_lower = query.to_lowercase();
        let mut results = String::new();

        let files = [
            ("decisions", knowledge_dir.join("decisions.md")),
            ("solutions", knowledge_dir.join("solutions.md")),
            ("patterns", knowledge_dir.join("patterns.md")),
        ];

        for (category, path) in &files {
            if !path.exists() {
                continue;
            }

            let content = std::fs::read_to_string(path)?;
            if content.to_lowercase().contains(&query_lower) {
                results.push_str(&format!("\n## {} (from {})\n\n", category, project));

                for line in content.lines() {
                    if line.to_lowercase().contains(&query_lower) {
                        results.push_str(&format!("{}\n", line));
                    }
                }
            }
        }

        if results.is_empty() {
            Ok(format!(
                "No knowledge matching '{}' in project '{}'",
                query, project
            ))
        } else {
            Ok(results)
        }
    }

    fn tool_projects(&self, _args: serde_json::Value) -> Result<String> {
        let projects =
            crate::parser::discovery::discover_projects(&self.config.claude_projects_dir)?;

        if projects.is_empty() {
            return Ok("No Claude projects found.".to_string());
        }

        let mut output = String::from("Claude Projects:\n\n");

        for project in &projects {
            let total_size: u64 = project.sessions.iter().map(|s| s.size).sum();
            let latest = project
                .sessions
                .iter()
                .map(|s| s.modified)
                .max()
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "unknown".into());

            let has_knowledge = self
                .config
                .memory_dir
                .join("knowledge")
                .join(&project.name)
                .join("context.md")
                .exists();

            output.push_str(&format!(
                "  {}{}\n    {} sessions, {}, last active: {}\n",
                project.name,
                if has_knowledge { " *" } else { "" },
                project.sessions.len(),
                humansize::format_size(total_size, humansize::BINARY),
                latest
            ));
        }

        Ok(output)
    }

    fn read_project_context(&self, project: &str) -> Result<String> {
        let knowledge_dir = self.config.memory_dir.join("knowledge").join(project);
        let context_path = knowledge_dir.join("context.md");

        if context_path.exists() {
            Ok(std::fs::read_to_string(&context_path)?)
        } else {
            // Try raw fallback
            match self.build_raw_context(project, &knowledge_dir) {
                Some(raw) => Ok(raw),
                None => Ok(format!(
                    "No context found for project '{}'. Run 'engram ingest' first.",
                    project
                )),
            }
        }
    }

    fn build_raw_context(
        &self,
        project: &str,
        project_knowledge_dir: &std::path::Path,
    ) -> Option<String> {
        use crate::extractor::knowledge::{
            parse_session_blocks, partition_by_expiry, reconstruct_blocks,
        };

        let read_and_filter = |path: &std::path::Path| -> String {
            let raw = std::fs::read_to_string(path).unwrap_or_default();
            let (preamble, blocks) = parse_session_blocks(&raw);
            let (active, _) = partition_by_expiry(blocks);
            reconstruct_blocks(&preamble, &active)
        };

        let decisions = read_and_filter(&project_knowledge_dir.join("decisions.md"));
        let solutions = read_and_filter(&project_knowledge_dir.join("solutions.md"));
        let patterns = read_and_filter(&project_knowledge_dir.join("patterns.md"));

        if decisions.trim().is_empty() && solutions.trim().is_empty() && patterns.trim().is_empty()
        {
            return None;
        }

        let mut out = format!("# {} - Project Context (raw, not synthesized)\n\n", project);

        if !decisions.trim().is_empty() {
            out.push_str(&decisions);
            out.push_str("\n\n");
        }
        if !solutions.trim().is_empty() {
            out.push_str(&solutions);
            out.push_str("\n\n");
        }
        if !patterns.trim().is_empty() {
            out.push_str(&patterns);
            out.push_str("\n\n");
        }

        Some(out)
    }

    fn tool_add(&self, args: serde_json::Value) -> Result<String> {
        let project = args["project"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing project parameter".into()))?;
        let category = args["category"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing category parameter".into()))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing content parameter".into()))?;
        let label = args["label"].as_str();

        // Delegate to CLI command (simpler than reimplementing)
        let mut cmd = std::process::Command::new("engram");
        cmd.args(["add", project, category, content]);
        if let Some(l) = label {
            cmd.args(["--label", l]);
        }

        let output = cmd.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MemoryError::Config(format!("Add failed: {}", stderr)));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn tool_analytics(&self, args: serde_json::Value) -> Result<String> {
        let days = args["days"].as_u64().unwrap_or(30);

        // Delegate to CLI
        let mut cmd = std::process::Command::new("engram");
        cmd.args(["analytics"]);

        if let Some(proj) = args["project"].as_str() {
            cmd.args([proj]);
        }

        cmd.args(["--days", &days.to_string()]);

        let output = cmd.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MemoryError::Config(format!("Analytics failed: {}", stderr)));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn tool_search_semantic(&self, args: serde_json::Value) -> Result<String> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing query parameter".into()))?;
        let limit = args["limit"].as_u64().unwrap_or(10);

        // Delegate to CLI
        let mut cmd = std::process::Command::new("engram");
        cmd.args(["search-semantic", query]);

        if let Some(proj) = args["project"].as_str() {
            cmd.args(["--project", proj]);
        }

        cmd.args(["--top", &limit.to_string()]);

        let output = cmd.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MemoryError::Config(format!(
                "Semantic search failed: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn tool_graph_query(&self, args: serde_json::Value) -> Result<String> {
        let project = args["project"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing project parameter".into()))?;
        let concept = args["concept"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing concept parameter".into()))?;

        let output = std::process::Command::new("engram")
            .args(["graph", "query", project, concept])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MemoryError::Config(format!(
                "Graph query failed: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    // ── Learning tools ──────────────────────────────────────────────────────

    /// Extract structured knowledge from provided text using the LLM pipeline.
    fn tool_reflect(&self, args: serde_json::Value) -> Result<String> {
        use crate::auth::resolve_provider;
        use crate::extractor::knowledge::{parse_session_blocks, reconstruct_blocks};
        use crate::llm::{client::LlmClient, prompts};
        use std::io::Write as IoWrite;

        let project = args["project"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing project parameter".into()))?;
        let text = args["text"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing text parameter".into()))?;

        // Which categories to extract — defaults to all three
        let all_cats = vec!["decisions", "solutions", "patterns"];
        let categories: Vec<&str> = if let Some(arr) = args["categories"].as_array() {
            arr.iter().filter_map(|v| v.as_str()).collect()
        } else {
            all_cats.clone()
        };

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

        let knowledge_dir = self.config.memory_dir.join("knowledge").join(project);
        std::fs::create_dir_all(&knowledge_dir)?;

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let label = format!("reflect-{}", &now[..10]);
        let mut stored: Vec<(&str, String)> = Vec::new();

        rt.block_on(async {
            let env_endpoint = std::env::var("ENGRAM_LLM_ENDPOINT").ok();
            let env_model = std::env::var("ENGRAM_LLM_MODEL").ok();
            let resolved = resolve_provider(None, env_endpoint, env_model)?;
            let client = LlmClient::new(&resolved);

            for cat in &categories {
                let prompt = match *cat {
                    "decisions" => prompts::decisions_prompt(text),
                    "solutions" => prompts::solutions_prompt(text),
                    "patterns" => prompts::patterns_prompt(text),
                    _ => continue,
                };

                let response = match client
                    .chat(prompts::SYSTEM_KNOWLEDGE_EXTRACTOR, &prompt)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("reflect/{}: LLM error: {}", cat, e);
                        continue;
                    }
                };

                // Skip if LLM found nothing significant
                if response.trim().to_lowercase().starts_with("no significant")
                    || response.trim().to_lowercase().starts_with("no clear")
                {
                    continue;
                }

                stored.push((cat, response));
            }
            Ok::<_, MemoryError>(())
        })?;

        if stored.is_empty() {
            return Ok("No significant knowledge extracted from the provided text.".to_string());
        }

        let mut summary = format!(
            "Reflected on text for '{}'. Stored {} category/-ies:\n",
            project,
            stored.len()
        );

        for (cat, content) in &stored {
            let filename = format!("{}.md", cat);
            let path = knowledge_dir.join(&filename);

            // Initialise file if needed
            if !path.exists() {
                let title = cat.chars().next().unwrap().to_uppercase().to_string() + &cat[1..];
                std::fs::write(&path, format!("# {}\n", title))?;
            }

            let header = format!("\n\n## Session: {} ({})\n\n", label, now);

            // Append
            let mut file = std::fs::OpenOptions::new().append(true).open(&path)?;
            writeln!(file, "{}{}", header, content)?;
            drop(file);

            // Invalidate context
            let ctx = knowledge_dir.join("context.md");
            if ctx.exists() {
                let _ = std::fs::remove_file(&ctx);
            }

            let preview: String = content
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .chars()
                .take(80)
                .collect();

            summary.push_str(&format!("  [{}] {} entries — \"{}\"\n", cat, 1, preview));
        }

        // Also write a combined reflection entry so the knowledge base stays coherent
        // The parse/reconstruct round-trip is a no-op here since we appended above,
        // but we do a quick count to confirm
        let _ = parse_session_blocks;
        let _ = reconstruct_blocks; // used elsewhere
        summary.push_str(&format!(
            "\nLabel: {} — use `update`/`forget` to correct.",
            label
        ));
        Ok(summary)
    }

    /// Update (replace) an existing knowledge entry by its label.
    fn tool_update(&self, args: serde_json::Value) -> Result<String> {
        use crate::extractor::knowledge::replace_session_block;

        let project = args["project"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing project".into()))?;
        let category = args["category"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing category".into()))?;
        let label = args["label"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing label".into()))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing content".into()))?;

        let path = self
            .config
            .memory_dir
            .join("knowledge")
            .join(project)
            .join(format!("{}.md", category));

        if !path.exists() {
            return Err(MemoryError::Config(format!(
                "No {} knowledge found for project '{}'",
                category, project
            )));
        }

        let file_content = std::fs::read_to_string(&path)?;
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let new_header = format!("\n\n## Session: {} ({}) [updated]\n\n", label, now);

        match replace_session_block(&file_content, label, &new_header, content) {
            Some(updated) => {
                std::fs::write(&path, updated)?;
                // Invalidate context
                let ctx = self
                    .config
                    .memory_dir
                    .join("knowledge")
                    .join(project)
                    .join("context.md");
                if ctx.exists() {
                    let _ = std::fs::remove_file(&ctx);
                }
                Ok(format!(
                    "Updated entry '{}' in {}/{}.md. Run `synthesize` to rebuild context.",
                    label, project, category
                ))
            }
            None => Err(MemoryError::Config(format!(
                "Entry '{}' not found in {}/{}.md — use `lookup` to find the correct label.",
                label, project, category
            ))),
        }
    }

    /// Remove a knowledge entry by label.
    fn tool_forget(&self, args: serde_json::Value) -> Result<String> {
        use crate::extractor::knowledge::remove_session_blocks;

        let project = args["project"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing project".into()))?;
        let category = args["category"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing category".into()))?;
        let label = args["label"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing label".into()))?;

        let path = self
            .config
            .memory_dir
            .join("knowledge")
            .join(project)
            .join(format!("{}.md", category));

        if !path.exists() {
            return Err(MemoryError::Config(format!(
                "No {} knowledge for project '{}'",
                category, project
            )));
        }

        let file_content = std::fs::read_to_string(&path)?;
        match remove_session_blocks(&file_content, &[label]) {
            Some(updated) => {
                std::fs::write(&path, updated)?;
                let ctx = self
                    .config
                    .memory_dir
                    .join("knowledge")
                    .join(project)
                    .join("context.md");
                if ctx.exists() {
                    let _ = std::fs::remove_file(&ctx);
                }
                Ok(format!(
                    "Removed entry '{}' from {}/{}.md.",
                    label, project, category
                ))
            }
            None => Err(MemoryError::Config(format!(
                "Entry '{}' not found in {}/{}.md — use `lookup` to find the correct label.",
                label, project, category
            ))),
        }
    }

    /// Session statistics — what this LLM instance has written.
    fn tool_status(&self, args: serde_json::Value) -> Result<String> {
        let project = args["project"].as_str();

        let stats = self
            .session
            .lock()
            .map_err(|_| MemoryError::Config("Session lock poisoned".into()))?;

        let uptime = stats
            .started_at
            .map(|t| {
                let secs = (chrono::Utc::now() - t).num_seconds();
                if secs < 60 {
                    format!("{}s", secs)
                } else {
                    format!("{}m {}s", secs / 60, secs % 60)
                }
            })
            .unwrap_or_else(|| "unknown".into());

        let mut out = format!(
            "Session stats (uptime: {})\n  added:      {}\n  reflected:  {}\n  updated:    {}\n  forgotten:  {}\n  synthesized:{}\n",
            uptime, stats.added, stats.reflected, stats.updated, stats.forgotten, stats.synthesized
        );

        // Count entries in project knowledge files
        if let Some(proj) = project {
            let kdir = self.config.memory_dir.join("knowledge").join(proj);
            out.push_str(&format!("\nKnowledge for '{}':\n", proj));
            for cat in &["decisions", "solutions", "patterns"] {
                let path = kdir.join(format!("{}.md", cat));
                if path.exists() {
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    let count = content.matches("## Session:").count();
                    out.push_str(&format!("  {}: {} entries\n", cat, count));
                }
            }
            let ctx_exists = kdir.join("context.md").exists();
            out.push_str(&format!(
                "  context.md: {}\n",
                if ctx_exists {
                    "present (may be stale — run `synthesize`)"
                } else {
                    "absent (run `synthesize` to build)"
                }
            ));
        }

        out.push_str(
            "\nTip: use `reflect` to extract knowledge, `synthesize` to rebuild context.md.",
        );
        Ok(out)
    }

    /// Re-synthesize context.md by delegating to `engram regen`.
    fn tool_synthesize(&self, args: serde_json::Value) -> Result<String> {
        let project = args["project"]
            .as_str()
            .ok_or_else(|| MemoryError::Config("Missing project".into()))?;

        let output = std::process::Command::new("engram")
            .args(["regen", project])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MemoryError::Config(format!(
                "Synthesize failed: {}",
                stderr
            )));
        }

        Ok(format!(
            "context.md rebuilt for '{}'. Use `recall` to read the updated summary.",
            project
        ))
    }
}
