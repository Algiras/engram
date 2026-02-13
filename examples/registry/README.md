# Example Registry

This is a working example of an Engram knowledge pack registry. You can use it directly to try out the registry system, or copy it as a template for your own.

## Try It Out

```bash
# From the engram repo root, add this example as a local registry
engram hive registry add file://$(pwd)/examples/registry

# Browse available packs
engram hive browse

# Install the core pack
engram hive install engram-core

# Verify it's installed
engram hive list
```

## Structure

```
examples/registry/
├── README.md
├── registry.json              # Index of all packs in this registry
└── packs/
    └── engram-core/
        ├── .pack/
        │   └── manifest.json  # Pack metadata (required)
        └── knowledge/
            ├── patterns.md
            ├── solutions.md
            └── workflows.md
```

## Creating Your Own Registry

1. Create a new GitHub repository
2. Copy this structure
3. Add `.pack/manifest.json` with full pack metadata under `packs/{name}/`
4. Add knowledge files in `packs/{name}/knowledge/`
5. Update `registry.json` with your pack entries

### Pack Manifest Example

```json
{
  "name": "rust-patterns",
  "version": "1.0.0",
  "description": "Rust idioms and best practices",
  "author": {
    "name": "youruser",
    "email": null
  },
  "license": "MIT",
  "keywords": ["rust", "patterns", "best-practices"],
  "categories": ["Patterns", "Solutions"],
  "homepage": "https://github.com/youruser/rust-patterns",
  "repository": "https://github.com/youruser/rust-patterns",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z",
  "privacy": {
    "share_patterns": true,
    "share_solutions": true,
    "share_decisions": false,
    "share_preferences": false,
    "redact_secrets": true,
    "require_review": true
  },
  "min_engram_version": "0.1.0"
}
```

### Requirements

- Valid `.pack/manifest.json` following the schema above
- Knowledge files in `knowledge/` directory (at least one .md file)
- No secrets or proprietary information
- MIT or compatible open-source license

## Using a Custom Registry

```bash
# From GitHub (shorthand)
engram hive registry add owner/repo

# From GitHub (full URL)
engram hive registry add https://github.com/owner/repo.git

# From a local path
engram hive registry add file:///absolute/path/to/registry

# List all registries
engram hive registry list

# Browse packs from all registries
engram hive browse

# Remove a registry
engram hive registry remove your-registry
```

## Use Cases

- **Enterprise:** Internal knowledge packs for your company
- **Teams:** Shared best practices across projects
- **Communities:** Language/framework-specific registries
