# AgentProv <version>

## Summary

-

## Highlights

-

## Compatibility Notes

- CLI, schema, and record formats are still MVP surfaces and may change before a
  stable release.

## Artifacts

The release workflow publishes platform archives for:

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

## Validation

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] `cargo build --release`
- [ ] Example run logs created in a temporary directory and verified
- [ ] `CHANGELOG.md` updated
