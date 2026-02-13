# Publishing Checklist

This checklist keeps releases reproducible and aligned across binaries, docs, and crate metadata.

## 1) Prepare release metadata

1. Update `Cargo.toml` version.
2. Add/update the matching section in `CHANGELOG.md`.
3. Verify install command in `README.md` and command examples are current.

## 2) Validate locally

Run from repository root:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build --release
```

CI also runs installer validation (`Install Validation`) fully in CI:

- version-pinned install path (`VERSION=vtest`)
- latest-tag API lookup path
- checksum tamper negative test (must fail)

CI includes a release guard (`Release Workflow Guard`) that fails if checksum generation/upload is removed from `.github/workflows/release.yml`.

Optional crate package validation:

```bash
cargo publish --dry-run
```

## 3) Publish GitHub release artifacts

Release workflow is triggered by a tag push matching `v*`:

```bash
git tag v0.3.0
git push origin v0.3.0
```

The workflow builds and uploads these assets:

- `claude-memory-x86_64-unknown-linux-gnu.tar.gz`
- `claude-memory-aarch64-apple-darwin.tar.gz`
- `claude-memory-x86_64-pc-windows-msvc.zip`
- `checksums.txt` (SHA-256 checksums for all release archives)

## 4) Verify installer compatibility

`install.sh` auto-detects OS/architecture, resolves both current target-based and legacy asset names, verifies SHA-256 using `checksums.txt`, and supports optional version pinning (`VERSION=vX.Y.Z`).

Quick check after release:

```bash
curl -fsSL https://raw.githubusercontent.com/Algiras/claude-memory/master/install.sh | sh
claude-memory --version
```

## 5) Optional: publish to crates.io

If you want crate distribution in addition to GitHub binaries:

```bash
cargo login
cargo publish
```

## 6) Post-release smoke checks

- Run `claude-memory ingest --skip-knowledge`
- Run `claude-memory projects`
- Run `claude-memory tui`
- If using MCP: `claude-memory mcp`

If any smoke check fails, cut a patch release (`vX.Y.Z+1`) with the fix and changelog entry.
