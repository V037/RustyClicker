# Changelog

## [1.3.0] - 2026-10-18 (Experimental)

> **Note:** This is an experimental release featuring GPU-accelerated screen capture and the PolyForm Noncommercial License.

### Added

- **GPU-Accelerated Screen Capture**: Integrated DirectX 11 / DXGI Desktop Duplication (`IDXGIOutputDuplication`) delivering sub-millisecond to ~4ms frame capture directly from GPU memory.
- **Smart Display Selection**: Automatic multi-monitor enumeration with primary monitor prioritization and desktop coordinate translation.
- **Static Frame Optimization**: Instantaneous ~0ms reuse when the desktop image has not changed (`DXGI_ERROR_WAIT_TIMEOUT`).
- **Transparent GDI Fallback**: Automatic, resilient fallback to GDI `BitBlt` if DXGI is unavailable or during desktop security state transitions.
- **License**: Adopted the PolyForm Noncommercial License 1.0.0 (`PolyForm-Noncommercial-1.0.0`).

### Fixed

- **Graceful Scanner Loop**: Handled `corrmatch` `NoCandidates` condition as a normal "target not on screen" scan cycle instead of halting with an error dialog.

## [1.2.2] - 2026-10-18 (Beta)

> **Note:** This is a beta preview release featuring the new Behavior Tree Logic Builder and redesigned modular dashboard.

### Added

- **Behavior Tree Logic Builder Panel**: Visual two-pane Win32 editor (`SysTreeView32`) with dynamic property configuration for nodes (`ClickImage`, `Repeat`, `IfElse`, `Sequence`) and JSON tree import/export.
- **Dedicated Clicker Panel**: Relocated clicker controls, target image slot selector, and dynamic preview into a dedicated panel.
- **Compact Window Resizing**: Support for compact minimum window sizes down to 320x330 (DPI scaled) with dynamic preview height adaptation.

### Changed

- **Refactored Main Menu**: Transformed main window into a clean System Overview dashboard displaying live statistics, hotkey cheatsheet, and persistent brand header, removing redundant middle navigation buttons.
- **Non-Overlapping Top Header**: Pinned top-right navigation buttons (`Clicker`, `Logic`, `Settings`, `Back`) with dynamically bounded title width and subtitle row (version badge and credits) that never overlap the buttons at any window width.
- **Clicker Action Layout**: Positioned the "Always on top" checkbox directly above the three action buttons (`F1 - Start`, `F2 - Stop`, `F3 - Close`), resolving narrow window horizontal crowding.

## [1.2.1] - 2026-10-18

### Changed

- Modularize the codebase by splitting monolithic files (`algorithms.rs` and
  `main_gui.rs`) into focused modules (`core`, `matching`, `capture`, `input`,
  `worker`, `gui`, and `tests`) for improved maintainability and reduced AI
  context token consumption.
- Move unit tests into a dedicated `tests` module compiled only during test runs.

## [1.2.0] - 2026-10-18

### Fixed

- Restore the selected image preview when returning from Settings to the main
  view.
- Remove the redundant “Back to images” button; the top-right Back button is
  the single Settings navigation control.
- Fix settings label styling so the click interval label uses normal text size
  and the scanning-thread label follows the active theme accent.

### Changed

- Replace the click-interval dropdown with an integer field accepting 1–10,000
  ms, defaulting to 100 ms.
- Add a persisted matching-thread selector for 1, 2, 4, or 8 Rayon threads,
  defaulting to 2.
- Replace string-based core matching, capture, and click failures with typed
  errors, retaining error categories through the worker and formatting them
  into user-facing messages at the UI boundary.

## [1.1.0] - 2026-10-18

### Added

- Manage GUI brushes, fonts, preview bitmaps, and temporary drawing objects
  with RAII wrappers so handles are released automatically.
- Use the Windows `SendInput` API for mouse clicks and report failed or partial
  input delivery instead of silently assuming a click succeeded.
- Save preferences as `%APPDATA%\RustyClicker\rusty_clicker.json` (falling
  back to `%LOCALAPPDATA%` when needed), rather than beside the executable.
- Migrate existing `image_clicker_preferences.json` settings to the new
  per-user app-data location on first launch.

### Fixed

- Display the saved image preview immediately at startup and repaint it when
  its bitmap changes.
- Repaint the title, version label, and cursor icon when the theme changes,
  using the active theme for the icon background.
- Reduce text redraw artifacts when resizing the window.

### Changed

- Update the embedded rusty-cursor icon asset.
- Keep the release executable independent of write permission to its install
  directory by storing preferences per user.

## [1.0.1] - 2026-10-16

- Fixed saved-slot preview loading at startup.
- Fixed theme repaint artifacts in the title, version label, and cursor icon.
- Reduced text artifacts when resizing the window.
- Refreshed the cursor icon asset.

## [1.0.0]

- Initial versioned release.
