# Fifteen Puzzle

A classic 15-puzzle sliding tile game, written in Rust. Core game logic lives in a shared crate,
with two separate frontends built on top of it: a terminal UI and a graphical app that runs on
desktop, iOS, and web.

## Project layout

This is a Cargo workspace. The root package (crate name `fifteen`) *is* the core game logic —
board representation, shuffling, move validation, and win detection. The workspace members are
the frontends:

```
.
├── Cargo.toml        # workspace root + core `fifteen` crate
├── src/               # core game logic
├── cli/               # terminal UI (ratatui + crossterm)
├── gui/               # graphical UI (Bevy) — desktop, iOS, web
├── res/               # shared assets (app icon, etc.)
├── ios/               # Xcode project for the iOS build
└── Makefile.toml      # cargo-make tasks for building every target
```

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [cargo-make](https://github.com/sagiegurari/cargo-make), for the build tasks below:
  ```
  cargo install cargo-make
  ```

Some targets have additional requirements, noted in their sections.

## Controls

Both frontends share the same controls:

- **Arrow keys** — slide a tile into the blank space
- **Click/tap a tile** — slide it (and any tiles between it and the blank) toward the blank
- **N** — start a new game
- **Q** / **Esc** — quit (CLI only; the GUI has its own menu)

## Running the CLI

```
cargo run -p cli
```

(adjust `-p cli` if you've renamed the package in `cli/Cargo.toml`)

### Installing it as a standalone command

```
cargo install --path cli
```

This installs a binary named `fifteen` to your Cargo bin directory (`~/.cargo/bin` by default —
make sure it's on your `PATH`). Afterwards, just run:

```
fifteen
```

## Running the GUI

### Desktop

```
cargo make desktop          # debug build, runs immediately
cargo make desktop-release  # release build
```

### Web (via Trunk)

Requires [Trunk](https://trunkrs.dev/):
```
cargo install trunk
```

```
cargo make web         # release build, output in gui/dist/
cargo make web-serve    # dev server with live rebuild
```

### iOS

Requires Xcode, a configured Apple developer signing team, and the `ios/FifteenPuzzle` Xcode
project set up to link against the Rust static library (see the project's Xcode build settings
for search paths and linked frameworks).

```
cargo make ios                # builds the Rust static lib + copies it into the Xcode project
cargo make ios-run-device     # builds, installs, and launches on a connected device
```

`ios-run-device` needs two environment variables set to identify your device:

```
IOS_DEVICE_UDID=<hardware UDID, from `xcrun xctrace list devices`>
IOS_DEVICE_ID=<devicectl ID, from `xcrun devicectl list devices`>
```

These are two *different* identifiers for the same device — both are required.

### Android

Not yet implemented (placeholder task in `Makefile.toml`).

### macOS app bundle

Requires [cargo-bundle](https://github.com/burtonageo/cargo-bundle) (installed automatically by
the task below if missing):

```
cargo make macos-bundle    # produces target/release/bundle/osx/Fifteen Puzzle.app
cargo make macos-install   # also copies it into /Applications
```

## Development

The core crate (`src/`) has no UI dependencies, so new frontends just depend on it (`fifteen = {
path = ".." }`) and reuse `Board`, `Move`, and the shuffle/move/win-detection logic — no game
rules are duplicated between the CLI and GUI.
