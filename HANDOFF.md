# RustyClicker AI Handoff

This document gives a future AI assistant the project context needed to make
safe, useful changes without relying on prior chat history. Verify the checkout
before acting: release details and working-tree state can change.

## Current project state

- Repository: `V037/RustyClicker`
- Main development branch: `rust_impl`
- The legacy `opencv_version` branch has been deleted both locally and remotely on GitHub.
- Current release tag: **v1.3.0 (Experimental)**
- GitHub release: <https://github.com/V037/RustyClicker/releases/tag/v1.3.0>
- Binary target: Native Windows x64 executable, size ~796 KB (< 1 MB).
- GUI Framework Decision: Retained native Win32 API (`windows-rs`) instead of `egui` to strictly satisfy the < 1 MB size budget and zero GPU gaming overhead.

## What the app does

RustyClicker is a lightweight Windows desktop image-target autoclicker written in Rust. It
captures the virtual desktop, searches for selected image targets, moves the
cursor to a match, and clicks. It has up to 99 image slots, previews, five
themes, global hotkeys (F1 Start, F2 Stop, F3 Close), an optional always-on-top setting, scan/click
statistics, and an isolated Behavior Tree Logic Builder panel.

## Architecture and important implementation details

- `src/main.rs`: Windows GUI subsystem entry point.
- `src/core/`: error types (`CoreError`) and shared definitions.
- `src/matching/`: template preparation, grayscale conversion, and Rayon matching pool.
- `src/capture/`: hybrid capture engine: GPU-accelerated DirectX 11 / DXGI Desktop Duplication (`dxgi.rs`) with sub-millisecond texture staging, automatic multi-monitor prioritization, and transparent fallback to GDI `BitBlt` (`screen.rs`).
- `src/input/`: Windows `SendInput` mouse click injection.
- `src/worker.rs`: worker thread loop and interruptible sleep.
- `src/gui/`:
  - `mod.rs`: common controls initialization (`ICC_TREEVIEW_CLASSES`) and window creation.
  - `controls.rs`: Win32 control creation (`create_control`) for header, main menu dashboard, clicker panel, settings panel, and logic tree panel.
  - `layout.rs`: dynamic DPI-aware control positioning and font sizing for header, main menu cards, clicker view, settings view, and logic builder.
  - `settings.rs`: settings panel, themes, slot count, thread count, click interval.
  - `logic_gui.rs`: **Behavior Tree Logic Builder panel**:
    - Two-pane Win32 view: Left pane with `SysTreeView32` and action buttons (`+ Click`, `+ Repeat`, `+ If/Else`, `Delete`). Right pane with property inspector edit controls and `Apply Changes`.
    - Nodes: `ClickImage`, `Repeat`, `IfElse`, `Sequence`.
    - Serde integration: Export (`GetSaveFileNameW`) and Import (`GetOpenFileNameW`) of nested logic trees to/from `.json`.
  - `state.rs`: centralized `AppState` struct managing all handles and unified panel navigation via `ActivePanel` (`MainMenu`, `Clicker`, `Logic`, `Settings`), `set_panel`, and live menu status summary updates.
  - `wndproc.rs`: message routing, hotkeys, dragging, custom button owner-drawing, `WM_NOTIFY` (`TVN_SELCHANGEDW`) handling, and panel switching commands.
- `src/tests/`:
  - `algorithms_tests.rs`: matching, grayscale conversion, DPI scaling, and release scan benchmarks.
  - `logic_tests.rs`: behavior tree JSON serialization, hierarchy integrity, and node CRUD operations.
- Total 23 automated tests pass in release mode.
- Matching uses `corrmatch` 0.2.5 with `rayon` and `simd` features (ZNCC coarse-to-fine).
- Matching thread choices are **1, 2, 4, and 8**, with **2** as the default.
- Click interval accepts integer values from **1 to 10,000 ms**, default **100 ms**.
- Preferences are saved as `%APPDATA%\RustyClicker\rusty_clicker.json`, falling back to `%LOCALAPPDATA%`.
- Release builds are optimized, use LTO, one codegen unit, and strip symbols.
- License: PolyForm Noncommercial License 1.0.0 (`PolyForm-Noncommercial-1.0.0`), permitting personal/hobbyist/educational use while strictly forbidding commercial sale and unauthorized redistribution.

## Build and validation

Requirements: Windows, Rust 1.91+, MSVC Rust toolchain, and Visual Studio C++
build tools.

Standard verification commands:

```powershell
cargo fmt --check
cargo check
cargo test --release
cargo build --release
```

The compiled release executable is located at `target\release\RustyClicker.exe`.

## Release and documentation notes

- Keep `Cargo.toml`'s package version, `Cargo.lock`, `README.md`'s version
  badge/text, and `CHANGELOG.md` in sync for a release.
- Update the changelog with user-visible fixes and feature changes.
- Push source changes to `rust_impl`, create a version tag such as `v1.3.0`,
  then publish the release and upload the compiled EXE as an asset.
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

- **v1.0**: Initial Rust implementation replacing legacy OpenCV with the native `corrmatch` ZNCC matcher.
- **v1.1**: Added GDI RAII resource management, Windows `SendInput`, per-user preferences, and auto-migration from older preference paths.
- **v1.2**: Added typed core errors, configurable Rayon matching thread count (1–8) and click delay (1–10,000 ms), settings-preview restoration, and label styling fixes.
- **v1.2.1**: Modularized monolithic GUI into dedicated submodules (`controls.rs`, `layout.rs`, `settings.rs`, `state.rs`, `wndproc.rs`, `logic_gui.rs`) and added the Behavior Tree Logic Builder panel (`SysTreeView32`, dynamic property editor, JSON export/import).
- **v1.2.2 (Beta)**: Relocated clicker controls into a dedicated panel, transformed main menu into a System Overview dashboard, non-overlapping header layout, compact window resizing (320×330), always-on-top button relocation, and adoption of PolyForm Noncommercial License 1.0.0.
- **v1.3.0 (Experimental)**: Added GPU-accelerated DirectX 11 / DXGI Desktop Duplication capture engine (`dxgi.rs`) with sub-millisecond texture staging, automatic multi-monitor display prioritization, seamless GDI `BitBlt` fallback, and graceful `NoCandidates` scanner loop handling.
