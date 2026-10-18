# RustyClicker AI Handoff

This document gives a future AI assistant the project context needed to make
safe, useful changes without relying on prior chat history. Verify the checkout
before acting: release details and working-tree state can change.

## Current project state

- Repository: `V037/RustyClicker`
- Main development branch: `rust_impl`
- Last verified release at handoff creation: **v1.2.0**
- Release tag: `v1.2.0`
- Release commit: `1eae53d`
- GitHub release: <https://github.com/V037/RustyClicker/releases/tag/v1.2.0>
- The workspace was clean at handoff creation.

## What the app does

RustyClicker is a Windows desktop image-target autoclicker written in Rust. It
captures the virtual desktop, searches for selected image targets, moves the
cursor to a match, and clicks. It has up to 99 image slots, previews, five
themes, global hotkeys, an optional always-on-top setting, and scan/click
statistics.

## Architecture and important implementation details

- `src/main.rs`: Windows GUI subsystem entry point.
- `src/main_gui.rs`: Win32 window, controls, layout, image previews, saved
  preferences, and worker lifecycle.
- `src/settings_gui.rs`: settings interactions, theme definitions, and the
  serialized `Preferences` model.
- `src/algorithms.rs`: template preparation, grayscale conversion, screen
  capture, matching, worker loop, click injection, and core error types.
- Matching uses `corrmatch` 0.2.5 with `rayon` and `simd` features. It searches
  grayscale images with ZNCC/coarse-to-fine matching.
- Matching thread choices are **1, 2, 4, and 8**, with **2** as the default.
  Each choice has a lazily initialized persistent Rayon pool.
- Click interval accepts integer values from **1 to 10,000 ms**, default
  **100 ms**. This is separate from the internal idle scan interval.
- Preferences are saved as `%APPDATA%\RustyClicker\rusty_clicker.json`, falling
  back to `%LOCALAPPDATA%`. Existing `image_clicker_preferences.json` files
  beside the executable are migrated on first launch.
- GDI resources in the GUI use RAII ownership. Screen capture resources use a
  `ScreenCapture` drop guard. Mouse clicks use `SendInput`.
- Core algorithm failures are represented by `CoreError` and formatted for
  display at the UI boundary.
- The Settings header's top-right **Back** button returns to the main view;
  there is no separate “Back to images” button.
- Release builds are optimized, use LTO, one codegen unit, and strip symbols.

## Build and validation

Requirements: Windows, Rust 1.91+, MSVC Rust toolchain, and Visual Studio C++
build tools.

Run the smallest relevant checks, then escalate when needed:

```powershell
cargo fmt --check
cargo check
cargo test --release
cargo build --release
```

The usual executable output is `target\release\RustyClicker.exe`. If Windows
reports that it is locked by a running app, build to a separate target
directory, for example:

```powershell
cargo build --release --target-dir target\handoff-build
```

## Release and documentation notes

- Keep `Cargo.toml`'s package version, `Cargo.lock`, `README.md`'s version
  badge/text, and `CHANGELOG.md` in sync for a release.
- Update the changelog with user-visible fixes and feature changes.
- Push source changes to `rust_impl`, create a version tag such as `v1.2.0`,
  then publish the optimized EXE as a GitHub release asset if requested.
- README performance figures are synthetic matcher-only measurements, not a
  guarantee for a user's hardware or full screen-capture/click cycle. Preserve
  that qualification when editing performance claims.
- The Rust executable name is `RustyClicker.exe`; the Windows app icon is
  embedded from the project assets/resource build setup.

## Working safely

1. Check `git status`, current branch, recent commits, and tags before edits.
   Do not overwrite or discard pre-existing changes.
2. Read the current implementation before acting; this note may be stale.
3. Keep changes focused, follow existing Win32/Rust patterns, and update
   directly related docs.
4. Add or adjust tests for measurable behavior, saved-setting compatibility,
   and validation boundaries.
5. Never assume a successful build means the GUI was visually exercised.
   Report GUI behavior that could not be tested interactively.
6. Do not publish, tag, or commit unless the user asks for it.

## Useful project history

- OpenCV was replaced with the Rust-native `corrmatch` matcher.
- The matcher uses a persistent configurable Rayon pool and SIMD support; the
  README documents its limitations and non-guaranteed match ordering.
- v1.1 added GUI GDI RAII, `SendInput`, per-user preferences, and migration
  from the old preference-file location.
- v1.2 added typed core errors, configurable thread count and click interval,
  settings-preview restoration, label styling fixes, and removal of the
  redundant Settings navigation button.
