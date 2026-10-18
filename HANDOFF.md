# RustyClicker AI Handoff

This document gives a future AI assistant the project context needed to make
safe, useful changes without relying on prior chat history. Verify the checkout
before acting: release details and working-tree state can change.

## Current project state

- Repository: `V037/RustyClicker`
- Main development branch: `rust_impl`
- The legacy `opencv_version` branch has been deleted both locally and remotely on GitHub.
- Current release tag: **v1.3.2 (Stable)**
- GitHub release: <https://github.com/V037/RustyClicker/releases/tag/v1.3.2>
- Binary target: Native Windows x64 executable, size ~836 KB (< 1 MB).
- GUI Framework Decision: Retained native Win32 API (`windows-rs`) instead of `egui` to strictly satisfy the < 1 MB size budget and zero GPU gaming overhead.

## What the app does

RustyClicker is a lightweight Windows desktop autoclicker suite written in Rust. It offers:
1. **Classical Autoclicker**: High-frequency cursor clicking at current mouse position with configurable delay (down to 0 ms / uncapped), mouse button selection (Left, Right, Middle), and live real-time CPS / total clicks tracking.
2. **Image Autoclicker**: Screen capture and template matching using grayscale zero-mean normalized cross-correlation (ZNCC), supporting up to 99 image slots, previews, configurable matching threads (1–8), and interval delays.
3. **Behavior Tree Logic Builder**: Visual two-pane Win32 editor (`SysTreeView32`) with dynamic node editing and JSON export/import.
4. **General Features**: Five themes, customizable global hotkeys (default: F1 Start, F2 Stop, F3 Close), stealth mode ("Hide when active"), and persistent per-user preferences.

## Architecture and important implementation details

- `src/main.rs`: Windows GUI subsystem entry point.
- `src/core/`: error types (`CoreError`) and shared definitions.
- `src/matching/`: template preparation, grayscale conversion, and Rayon matching pool.
- `src/capture/`: hybrid capture engine: GPU-accelerated DirectX 11 / DXGI Desktop Duplication (`dxgi.rs`) with sub-millisecond texture staging, automatic multi-monitor prioritization, and transparent fallback to GDI `BitBlt` (`screen.rs`).
- `src/input/`: Windows `SendInput` mouse click injection (`ClickButton` support for Left, Right, Middle).
- `src/worker.rs`: image clicker worker thread loop and interruptible sleep.
- `src/classical_worker.rs`: classical direct cursor clicking worker loop with millisecond precision, atomic CPS and click count tracking.
- `src/gui/`:
  - `mod.rs`: common controls initialization, window creation, and control IDs.
  - `controls.rs`: Win32 control creation (`create_control`) for header, main menu dashboard, classical clicker, image clicker, settings panel, and logic tree panel. Subclasses hotkey edit controls with `hotkey_input_proc` for single-keypress rebinding.
  - `drawing.rs`: custom button owner-drawing (`draw_button`), empty image preview border, and vector 8-tooth mechanical gear rendering (`draw_gear`).
  - `layout.rs`: dynamic DPI-aware control positioning and font sizing for header, main menu cards, classical clicker view, image clicker view, settings view, and logic builder.
  - `preferences.rs`: user preferences serialization with theme, hotkeys (`start_hotkey`, `stop_hotkey`, `close_hotkey`), stealth mode (`hide_when_active`), and virtual key translation (`vk_to_string`).
  - `settings.rs`: settings panel with themes, interactive hotkey fields, always-on-top, and hide-when-active.
  - `logic_gui.rs`: **Behavior Tree Logic Builder panel**:
    - Two-pane Win32 view: Left pane with `SysTreeView32` and action buttons (`+ Click`, `+ Repeat`, `+ If/Else`, `Delete`). Right pane with property inspector edit controls and `Apply Changes`.
    - Nodes: `ClickImage`, `Repeat`, `IfElse`, `Sequence`.
    - Serde integration: Export (`GetSaveFileNameW`) and Import (`GetOpenFileNameW`) of nested logic trees to/from `.json`.
  - `state.rs`: centralized `AppState` struct managing all handles and unified panel navigation via `ActivePanel` (`MainMenu`, `Classical`, `Clicker`, `Logic`, `Settings`), `set_panel`, dynamic status/tip label updating (`update_hotkey_labels`), and window restore logic.
  - `wndproc.rs`: message routing, context-sensitive start hotkey routing, global stop/close execution, window dragging, `WM_CTLCOLORSTATIC` theming, and worker timer updates.
- `src/tests/`:
  - `algorithms_tests.rs`: matching, grayscale conversion, DPI scaling, hotkey formatting, preferences round-tripping, and scan benchmarks.
  - `logic_tests.rs`: behavior tree JSON serialization, hierarchy integrity, and node CRUD operations.
- Total 26 automated tests pass in release mode.
- Matching uses `corrmatch` 0.2.5 with `rayon` and `simd` features (ZNCC coarse-to-fine).
- Matching thread choices are **1, 2, 4, and 8**, with **2** as the default.
- Click interval accepts integer values from **1 to 10,000 ms**, default **100 ms**.
- Classical clicker interval accepts integers from **0 to 10,000 ms** (0 = uncapped), default **50 ms**, button choice Left/Right/Middle.
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
- Push source changes and tags to `rust_impl`: `git push origin rust_impl --tags`.
- GitHub CLI (`gh`) is not installed on this machine. To create a release and attach `RustyClicker.exe`, retrieve the stored GitHub token using `git credential fill` (`protocol=https` and `host=github.com`) and call the GitHub REST API (`POST /repos/V037/RustyClicker/releases` followed by `POST uploads.github.com/.../assets?name=RustyClicker.exe`).
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
- **v1.3.1 (Stable)**: Added ultra-fast Classical Autoclicker with millisecond delay and button choice, interactive single-keypress global hotkey remapping in Settings, stealth mode ("Hide when active"), DPI-aware vector gear icon, grouped navigation buttons, context-sensitive start hotkey guard, dynamic hotkey tips/labels, and classical status label palette theming.
- **v1.3.2 (Stable)**: Adjusted default initial launch window dimensions to 360×330 px with dedicated `DEFAULT_WINDOW_*` and `MIN_WINDOW_*` constants while preserving the compact 320×330 px minimum resize tracking limits.
