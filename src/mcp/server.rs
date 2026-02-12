use super::protocol::*;
use crate::config::Config;
use crate::error::{MemoryError, Result};
use colored::Colorize;
use serde_json::json;
use std::io::{BufRead, Write};

pub struct McpServer {
    config: Config,
}

impl McpServer {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Run the MCP server on stdio
    pub fn run(&self) -> Result<()> {
        eprintln!("{}", "claude-memory MCP server starting...".green());
        eprintln!("{}", "Listening on stdio".dimmed());

        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        let reader = stdin.lock();

        for line in reader.lines() {
            let line = line.map_err(|e| MemoryError::Io(e))?;

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
                    "name": "claude-memory",
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
        let projects = match crate::parser::discovery::discover_projects(&self.config.claude_projects_dir) {
            Ok(projects) => projects,
            Err(e) => {
                return Response::error(id, -32000, format!("Failed to discover projects: {}", e));
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
        let knowledge_only = args.get("knowledge_only").and_then(|v| v.as_bool()).unwrap_or(false);

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
            Ok(format!("No knowledge matching '{}' in project '{}'", query, project))
        } else {
            Ok(results)
        }
    }

    fn tool_projects(&self, _args: serde_json::Value) -> Result<String> {
        let projects = crate::parser::discovery::discover_projects(&self.config.claude_projects_dir)?;

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
                None => Ok(format!("No context found for project '{}'. Run 'claude-memory ingest' first.", project)),
            }
        }
    }

    fn build_raw_context(&self, project: &str, project_knowledge_dir: &std::path::Path) -> Option<String> {
        use crate::extractor::knowledge::{parse_session_blocks, partition_by_expiry, reconstruct_blocks};

        let read_and_filter = |path: &std::path::Path| -> String {
            let raw = std::fs::read_to_string(path).unwrap_or_default();
            let (preamble, blocks) = parse_session_blocks(&raw);
            let (active, _) = partition_by_expiry(blocks);
            reconstruct_blocks(&preamble, &active)
        };

        let decisions = read_and_filter(&project_knowledge_dir.join("decisions.md"));
        let solutions = read_and_filter(&project_knowledge_dir.join("solutions.md"));
        let patterns = read_and_filter(&project_knowledge_dir.join("patterns.md"));

        if decisions.trim().is_empty() && solutions.trim().is_empty() && patterns.trim().is_empty() {
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
}
