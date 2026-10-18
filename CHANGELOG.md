# Changelog

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
