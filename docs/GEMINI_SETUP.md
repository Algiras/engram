# Gemini API Support

`engram` now supports Google's Gemini API for knowledge extraction!

## Setup

### 1. Get Gemini API Key

Get your API key from [Google AI Studio](https://makersuite.google.com/app/apikey)

### 2. Set Environment Variable

```bash
export GEMINI_API_KEY='your-api-key-here'
```

Add this to your `~/.bashrc` or `~/.zshrc` to make it permanent.

### 3. Verify Setup

```bash
engram auth list
```

You should see:
```
Google Gemini    env var
```

## Usage

### Use Gemini for Knowledge Extraction

```bash
# Use Gemini for a specific project
engram ingest --project my-project --provider gemini

# Use Gemini for all projects
engram ingest --provider gemini
```

### Custom Model

The default model is `gemini-pro`. To use a different model:

```bash
export ENGRAM_LLM_MODEL='gemini-1.5-pro-latest'
```

Common Gemini models:
- `gemini-pro` (default)
- `gemini-1.5-pro-latest`
- `gemini-1.5-flash-latest`
- `gemini-exp-1206` (experimental)

## Authentication with Internal Services

If you have access to internal authentication services (like ANTIGRAVITY), you can integrate them by:

1. **Environment Variable Approach** (easiest):
   ```bash
   # Get token from your auth service
   export GEMINI_API_KEY=$(your-auth-service get-token gemini)
   ```

2. **Wrapper Script** (for automatic token refresh):
   ```bash
   #!/bin/bash
   # ~/.local/bin/engram-with-auth
   export GEMINI_API_KEY=$(your-auth-service get-token gemini)
   exec engram "$@"
   ```

   Then use:
   ```bash
   engram-with-auth ingest --provider gemini
   ```

3. **Custom Provider** (advanced):
   You can add a custom provider by modifying `src/auth/providers.rs`:

   ```rust
   pub enum Provider {
       Anthropic,
       OpenAI,
       Ollama,
       Gemini,
       YourCustomProvider,  // Add your provider
   }
   ```

## Comparison with Other Providers

| Provider | Speed | Quality | Cost | Local |
|----------|-------|---------|------|-------|
| **Gemini** | ⚡⚡⚡ Fast | ⭐⭐⭐⭐ Very Good | 💰 Free tier generous | ❌ No |
| **Anthropic (Claude)** | ⚡⚡ Medium | ⭐⭐⭐⭐⭐ Excellent | 💰💰 Paid | ❌ No |
| **OpenAI (GPT-4)** | ⚡⚡ Medium | ⭐⭐⭐⭐ Excellent | 💰💰 Paid | ❌ No |
| **Ollama** | ⚡ Slow (depends on hardware) | ⭐⭐⭐ Good | 🆓 Free | ✅ Yes |

## Tips

- **First run**: Use `--skip-knowledge` for fast initial archival, then run with Gemini later
- **Large projects**: Gemini's generous free tier makes it great for processing many sessions
- **Best quality**: Use Claude (Anthropic) for highest quality knowledge extraction
- **Privacy**: Use Ollama (local) if you need to keep conversations offline
- **Speed**: Gemini is one of the fastest options

## Troubleshooting

### "Gemini API returned 404 Not Found"

The model name might not be available. Try:
```bash
export ENGRAM_LLM_MODEL='gemini-1.5-pro-latest'
```

### "API key required"

Make sure `GEMINI_API_KEY` is set:
```bash
echo $GEMINI_API_KEY  # Should print your key
```

### Rate Limits

Gemini's free tier has generous limits, but if you hit them:
- Add delays between ingestions
- Use `--since 1d` to process only recent sessions
- Consider upgrading to paid tier

## MCP Server with Gemini

When using the MCP server with Claude Desktop, you can specify Gemini:

```json
{
  "mcpServers": {
    "engram": {
      "command": "engram",
      "args": ["mcp", "--provider", "gemini"],
      "env": {
        "GEMINI_API_KEY": "your-key-here"
      }
    }
  }
}
```

This way, Claude Desktop can query your memory, and knowledge extraction uses Gemini!
