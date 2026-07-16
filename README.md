# Fifteen Puzzle

A classic 15-puzzle sliding tile game, written in Rust. Core game logic lives in a shared crate,
with two separate frontends built on top of it: a terminal UI and a graphical app that runs on
desktop, iOS, Android, and web.

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

Built via [`cargo-apk`](https://github.com/rust-mobile/cargo-apk), using Android's `NativeActivity`
— a single-command build, no separate Gradle/Android Studio project needed for the app itself
(you still need the SDK/NDK, installed via Android Studio).

Requires:
- Android SDK Platform (API 33) and NDK, installed via Android Studio's SDK Manager (SDK Tools
  tab: check "NDK (Side by side)"; SDK Platforms tab: check the matching API level)
- `ANDROID_HOME` and `ANDROID_NDK_HOME` environment variables set
- `JAVA_HOME` set (Android Studio bundles its own JDK, e.g.
  `/Applications/Android Studio.app/Contents/jbr/Contents/Home` on macOS)

```
cargo make android       # builds a debug APK
cargo make android-run   # builds, installs, and launches on a connected device/emulator
```

`android-run` targets whatever `adb devices` sees, so either a physical device with USB debugging
enabled or a running emulator (AVD) works.

**16KB page size:** `gui/build.rs` already passes the linker flags (`-Wl,-z,max-page-size=16384`)
needed for compatibility with newer 16KB-page-size Android devices, gated to only apply when
building for `target_os = "android"`. That said, the "16k" preview emulator images in Android
Studio's Device Manager (e.g. `sdk_gphone16k_arm64`) have been unreliable for testing graphics-
heavy apps — in our testing, every graphics backend (hardware, software, explicit swiftshader,
including after a full cold boot to rule out stale snapshots) hit an identical
`Unable to find a GPU!` panic from wgpu, pointing to a limitation in the preview image's GPU
virtualization rather than anything in this project. Stick to a standard (non-16k) system image
for day-to-day testing; validating actual 16KB-page compatibility needs either a fixed/future
emulator image or a real 16KB-page-size device.

Release builds need a real signing keystore configured under
`[package.metadata.android.signing.release]` in `gui/Cargo.toml` (see `cargo-apk`'s README);
without it, `cargo apk build --release` will refuse to sign the APK.

**Sharing a build with someone:** the debug build (`cargo make android`) already produces a fully
installable, signed APK — signed with an auto-generated debug keystore rather than a real release
identity, which is fine for sending to a friend. Find it with:
```
find target -name "*.apk"
```
Use the aligned one (e.g. `target/debug/apk/fifteen_gui.apk`), not the `-unaligned.apk` sibling —
that's just an intermediate build artifact. Send the `.apk` file directly; the recipient will need
to enable "Install unknown apps" for whatever app they open it with, then tap it to install.

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
