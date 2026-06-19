# Release Process

AgentProv releases are created from version tags named `vMAJOR.MINOR.PATCH`.

## Before tagging

1. Update `CHANGELOG.md`.
2. Run the release validation gates:

   ```bash
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   cargo build --release
   ```

3. Smoke test at least one deterministic example in a temporary directory and
   verify the produced run log.
4. Draft release notes from `.github/release_notes_template.md`.

## Tag release

Create and push a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow builds platform archives and uploads the artifact matching
each matrix target.

## Do not publish

Do not include generated demo output, local key files, temporary run logs, local
SQLite databases, or ad hoc collector dashboard exports in release commits.
