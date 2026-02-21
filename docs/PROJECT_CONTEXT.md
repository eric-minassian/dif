# Project Context

This file captures practical usage/context that is intentionally kept outside `README.md`.

## What `dif` is for

`dif` is a TUI for fast staging workflows:

- inspect changed files in a single tree view with staged/unstaged markers
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

- Focus toggles between sidebar tree and diff (`Tab`, reverse with `Shift+Tab`)
- Pane focus alternates between sidebar and diff (`Left`/`Right` or `h`/`l`)
- Selection movement uses arrows or vim keys (`j`/`k`)
- Stage toggle is contextual on current list (`Enter` or `Space`)
- Sidebar always uses a single tree list with staged/unstaged markers
- Quick help overlay is available from most non-text-input contexts (`?` or `F1`)
- Terminal modal opens with `:` or `!`
- Settings modal opens with `o`

## Internal architecture references

- Layout primitives: `src/layout.rs`
- App state and transitions: `src/app.rs`
- Event routing and key mapping: `src/input.rs`, `src/keymap.rs`
- Rendering modules: `src/ui/`
- PTY integration: `src/terminal.rs`
