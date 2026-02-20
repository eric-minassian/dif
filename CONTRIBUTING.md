# Contributing

## Development standards

### Module boundaries

- Keep modules focused on one responsibility.
- Prefer splitting files once a module grows beyond ~300-400 lines.
- Put shared UI/layout logic in reusable modules instead of duplicating snippets.

### Error handling

- Runtime paths should not use `unwrap`/`expect`.
- Use `anyhow::Context` on IO/process boundaries to keep errors actionable.
- Surface recoverable failures to the status bar instead of panicking.

### Keybinding conventions

- Define key constants in `src/keymap.rs`.
- Derive help/footer text from the same keymap constants.
- Avoid hardcoding key labels in both input handling and UI hints.

### Settings persistence

- Settings changes should be batched/debounced and flushed on close/exit.
- Keep settings normalization in one place (`AppSettings::normalize`).

## Testing expectations

Run before opening a PR:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features
cargo test
```

Add or update tests for:

- Git status parsing behavior (`src/git.rs`)
- Input mapping behavior (`src/input.rs`)
- Terminal key encoding (`src/terminal.rs`)
- Settings normalization/cycling (`src/settings.rs`)
- Diff row parsing and unified rendering snapshots (`src/diff.rs`, `src/ui/diff.rs`)
