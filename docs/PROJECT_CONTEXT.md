# Project Context

This file captures practical usage/context that is intentionally kept outside `README.md`.

## What `dif` is for

`dif` is a TUI for fast staging workflows:

- inspect unstaged and staged files side by side
- inspect split or unified file diffs with syntax highlighting
- stage/unstage/undo selected files quickly
- open an embedded shell in the same repo context without leaving the app

## Configuration path and format

Settings are persisted to:

- `$XDG_CONFIG_HOME/dif/config.toml` when `XDG_CONFIG_HOME` is set
- otherwise `~/.config/dif/config.toml`

Example config:

```toml
diff_view_mode = "auto"
sidebar_position = "left"
sidebar_visible = true
sidebar_width = 34
auto_split_min_width = 140
theme = "ocean"
confirm_undo_to_mainline = true
```

## Core interaction model

- Focus alternates between Unstaged and Staged lists (`Tab`)
- Pane focus alternates between sidebar and diff (`Left`/`Right`)
- Selection movement uses arrows or vim keys (`j`/`k`)
- Terminal modal opens with `:` or `!`
- Settings modal opens with `o`

## Internal architecture references

- Layout primitives: `src/layout.rs`
- App state and transitions: `src/app.rs`
- Event routing and key mapping: `src/input.rs`, `src/keymap.rs`
- Rendering modules: `src/ui/`
- PTY integration: `src/terminal.rs`
