# Architecture

`dif` has three primary layers:

1. State and domain logic
2. Rendering
3. Input/event routing

## State and domain

- `src/app.rs` is the app state coordinator and transition engine.
- `src/git.rs` is the only module that shells out to `git`.
- `src/settings.rs` owns serialization/deserialization and normalization.
- `src/terminal.rs` manages the PTY session and terminal output model.

## Rendering

- `src/ui/mod.rs` is the render entrypoint.
- `src/ui/sidebar.rs` draws staged/unstaged panes.
- `src/ui/diff.rs` draws split/unified diffs.
- `src/ui/modal.rs` draws settings and terminal overlays.
- `src/ui/palette.rs` owns color palettes and style helpers.
- `src/layout.rs` provides shared geometry/layout helpers for app + ui.

Rendering is intentionally pure (`&App`) and should not mutate state.

## Input routing

- `src/input.rs` routes crossterm events to app actions.
- Keybinding constants and user-facing hints are centralized in `src/keymap.rs`.

## Update loop

`src/main.rs` runs the event loop with:

- a terminal lifecycle guard (raw mode + alternate screen cleanup in `Drop`)
- dirty redraw behavior (draw only after input/tick changes)
- pre-render layout computation (`App::update_layout`)
