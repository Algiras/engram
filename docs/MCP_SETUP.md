# MCP Server Setup for Claude Desktop

`engram` now includes MCP (Model Context Protocol) server support, allowing Claude Desktop to directly access your conversation memory without shell hooks.

## What is MCP?

MCP allows Claude Desktop to call external tools and read external resources during conversations. With the `engram` MCP server, Claude can:

- **Recall** project context and knowledge
- **Search** across all your memory
- **Lookup** specific topics in projects
- **List** all projects with activity
- **Read** project contexts as resources

## Installation

### 1. Install engram

```bash
cargo install --path .
# or use the install script
curl -fsSL https://raw.githubusercontent.com/Algiras/engram/master/install.sh | sh
```

### 2. Configure Claude Desktop

Add the MCP server to your Claude Desktop configuration file:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
**Linux**: `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "engram": {
      "command": "engram",
      "args": ["mcp"]
    }
  }
}
```

If you want to use a specific LLM provider for knowledge extraction, add the provider flag:

```json
{
  "mcpServers": {
    "engram": {
      "command": "engram",
      "args": ["mcp", "--provider", "anthropic"],
      "env": {
        "ANTHROPIC_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

### 3. Restart Claude Desktop

After updating the configuration, restart Claude Desktop for the changes to take effect.

## Available Tools

Once configured, Claude Desktop will have access to these tools:

### `recall`
Retrieve project context and knowledge summary.

**Parameters:**
- `project` (string, required): Project name

**Example usage:**
> Claude, recall the engram project

### `search`
Search across all memory using regex patterns.

**Parameters:**
- `query` (string, required): Search query (regex supported)
- `project` (string, optional): Limit search to specific project
- `knowledge_only` (boolean, optional): Search only knowledge files

**Example usage:**
> Claude, search for "authentication" in my memory

### `lookup`
Look up knowledge by topic for a specific project.

**Parameters:**
- `project` (string, required): Project name
- `query` (string, required): Topic to search for

**Example usage:**
> Claude, lookup "rate limiting" in the api-server project

### `projects`
List all discovered projects with activity.

**Example usage:**
> Claude, show me all my projects

## Available Resources

Claude Desktop can also read project contexts as resources:

- `memory://<project>/context` - Project context markdown file

These resources can be attached to conversations, allowing Claude to reference your project knowledge automatically.

## Advantages over Shell Hooks

- **Direct integration**: No need for shell scripts
- **Real-time access**: Claude can query memory during conversations
- **Better error handling**: Structured errors instead of silent failures
- **Cross-platform**: Works on Windows, macOS, and Linux
- **Resource access**: Claude can read project contexts as resources

## Troubleshooting

### Server not starting

Check the Claude Desktop logs:
- **macOS**: `~/Library/Logs/Claude/mcp*.log`
- **Windows**: `%APPDATA%\Claude\logs\mcp*.log`
- **Linux**: `~/.config/Claude/logs/mcp*.log`

### Command not found

Ensure `engram` is in your PATH:

```bash
which engram
# Should output: /Users/<username>/.cargo/bin/engram (or similar)
```

If not, add `~/.cargo/bin` to your PATH in your shell profile.

### No projects found

Run an initial ingestion to populate the memory:

```bash
engram ingest
```

### LLM provider authentication

If knowledge extraction is not working, configure authentication:

```bash
engram auth login --provider anthropic
engram auth status
```

## Testing the MCP Server

You can test the MCP server manually using stdio:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | engram mcp
```

This should return a JSON response with server capabilities.

To test the tools:

```bash
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | engram mcp
```

## Next Steps

Once configured:

1. Start a conversation in Claude Desktop
2. Ask Claude to recall project context or search your memory
3. Claude will automatically use the MCP tools to access your knowledge
4. Your conversation history will continue to be ingested via the hooks

The MCP server and shell hooks work together - hooks populate memory in the background, while MCP provides real-time access during conversations.
