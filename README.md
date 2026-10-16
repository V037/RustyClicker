# RustyClicker

**Version:** v1.0.0

RustyClicker is a lightweight Windows autoclicker written in Rust. It watches
the screen for selected image targets and clicks the center of a target when it
finds one.

## Features

- Match image targets against the screen using grayscale zero-mean normalized
  cross-correlation (ZNCC).
- Configure between 1 and 99 image slots, and advance to the next configured
  target after a click.
- Choose a click delay from 25 ms to 2 seconds.
- Use a compact, resizable window with five themes, an optional always-on-top
  setting, and image previews.
- View the latest scan duration and total click count in the main window.
- Use a persistent two-thread Rayon pool and SIMD-enabled matching without an
  OpenCV runtime dependency.
- Build a standalone executable with the rusty-cursor icon embedded.

## Build

Rust 1.91 or newer is required. On Windows, install the Rust MSVC toolchain and
its required Visual Studio C++ build tools.

Build the optimized executable with:

```powershell
cargo build --release
```

The output is:

```text
target\release\RustyClicker.exe
```

To run directly from the project:

```powershell
cargo run --release
```

## Use

1. Open **Settings** and choose how many image slots to use.
2. Select an image for each target slot.
3. Set the click delay and any preferred theme or always-on-top option.
4. Return to the main view and press **Start**.

The app scans the virtual desktop, moves the pointer to the center of a detected
target, and clicks. A scan interval of 10 ms is the intended idle interval; the
actual interval depends on capture and matching time and may be longer.

## Matching and performance

The matcher uses grayscale ZNCC with a confidence threshold of `0.90`. Template
data is prepared when an image is loaded. Screen matching uses
[`corrmatch`](https://crates.io/crates/corrmatch) with its SIMD and Rayon
features enabled; Rayon is configured with a persistent pool limited to two
threads. Matching uses a coarse-to-fine image pyramid, so its selected location
is not guaranteed to be the first row-major match when a target appears more
than once.

In a release-mode synthetic benchmark on a 1920×1080 image with a 64×64 target,
the matcher measured:

| Case | Scan time |
| --- | ---: |
| No match | 38.08 ms |
| Exact match | 24.80 ms |

These timings are from one test run, not a hardware-independent performance
guarantee. The test includes RGB-to-grayscale conversion and matching, but not
Windows screen capture, cursor movement, or clicking. In particular, the
10 ms scan interval is a target, not a guarantee that each full-screen scan
finishes in 10 ms.

## Credits

Created by **V037_**.
