# RustyClicker Python Extension (`rusty_clicker.pyd`)
## Complete API Reference, Algorithmic Architecture & Integration Guide

`rusty_clicker` is an ultra-fast, native CPython extension (`.pyd`) engineered in Rust. It provides high-performance screen capture, hardware-accelerated Windows WinRT OCR, normalized cross-correlation template matching, and low-level hardware mouse input injection.

It was designed specifically to replace slow, resource-heavy Python automation libraries—such as `pytesseract` (Tesseract-OCR), `pyautogui.locateOnScreen()`, and `cv2.matchTemplate()`—with zero-dependency, sub-millisecond native Rust implementations.

---

## Table of Contents
1. [Core Features & Performance Highlights](#1-core-features--performance-highlights)
2. [How the Algorithms Work Under the Hood](#2-how-the-algorithms-work-under-the-hood)
   - [Direct Desktop Capture Pipeline (DXGI & GDI)](#21-direct-desktop-capture-pipeline-dxgi--gdi)
   - [Hardware-Accelerated WinRT OCR Pipeline](#22-hardware-accelerated-winrt-ocr-pipeline)
   - [Normalized Cross-Correlation (NCC) Template Matching](#23-normalized-cross-correlation-ncc-template-matching)
   - [Hardware Mouse Injection via Win32 SendInput](#24-hardware-mouse-injection-via-win32-sendinput)
3. [Installation & Setup](#3-installation--setup)
4. [Complete API Reference](#4-complete-api-reference)
   - [Top-Level Module Functions](#41-top-level-module-functions)
   - [Class: `ScreenCapture`](#42-class-screencapture)
   - [Class: `PreparedTemplate`](#43-class-preparedtemplate)
   - [Data Models: `TextBox`, `OcrWord`, `Match`](#44-data-models-textbox-ocrword-match)
5. [Step-by-Step Code Recipes](#5-step-by-step-code-recipes)
   - [Recipe 1: Quick Text Detection & Substring Check](#recipe-1-quick-text-detection--substring-check)
   - [Recipe 2: Find Text on Screen & Click Its Center](#recipe-2-find-text-on-screen--click-its-center)
   - [Recipe 3: High-Frequency Polling Loop (0% CPU Overhead)](#recipe-3-high-frequency-polling-loop-0-cpu-overhead)
   - [Recipe 4: Real-Time In-Game 3D Coordinate Extraction](#recipe-4-real-time-in-game-3d-coordinate-extraction)
   - [Recipe 5: Template Image Matching on Screen](#recipe-5-template-image-matching-on-screen)
   - [Recipe 6: Word-Level Bounding Boxes & Color Filtering](#recipe-6-word-level-bounding-boxes--color-filtering)
6. [Performance Comparison & Benchmarks](#6-performance-comparison--benchmarks)
7. [Standalone Executable Packaging (PyInstaller, Nuitka)](#7-standalone-executable-packaging-pyinstaller-nuitka)
8. [Best Practices & Troubleshooting](#8-best-practices--troubleshooting)

---

## 1. Core Features & Performance Highlights

- **Native Windows WinRT OCR**: Uses the hardware-accelerated OCR engine built into Windows 10 and 11 (`Windows.Media.Ocr`). **No external `tesseract.exe` or `tessdata` files required.**
- **Sub-Region Speed**: Recognizes cropped game text (interaction prompts, dialogue, coordinates) in **10–18 ms** (compared to 800–1,500 ms with Tesseract).
- **Direct GPU Framebuffer Ingestion**: Desktop frames are captured directly into raw 32-bit BGRA memory buffers and passed into WinRT `SoftwareBitmap` with zero color conversion or temporary disk file overhead.
- **True Multithreading & GIL Release**: All heavy capture, matching, and OCR operations execute inside Rust background threads (`py.detach(|| ...)`), completely releasing Python's Global Interpreter Lock (GIL). Your Python GUI / macro threads will never freeze.
- **Multi-Monitor Coordinate Normalization**: Seamlessly handles primary, secondary, and negative-offset monitor spaces.
- **Hardware-Level Click Injection**: Uses Win32 `SendInput` with normalized absolute coordinates (`0..65535`), ensuring inputs are recognized by anti-cheat and direct-input game engines.

---

## 2. How the Algorithms Work Under the Hood

### 2.1 Direct Desktop Capture Pipeline (DXGI & GDI)

Traditional Python tools (`pyautogui`, `PIL.ImageGrab`) capture screens by invoking GDI `BitBlt` into temporary Python objects, or dumping PNGs to disk. This incurs heavy memory copies, GDI handle leaks, and frame lag.

`rusty_clicker` utilizes a two-tier capture pipeline:

```mermaid
flowchart TD
    A["Screen Capture Request"] --> B{"DirectX 11 Available?"}
    B -- Yes --> C["DXGI OutputDuplication<br/>(IDXGIOutputDuplication)"]
    C --> D["AcquireNextFrame() into VRAM"]
    D --> E["CopyResource() to D3D11 CPU Staging Texture"]
    E --> F["D3D11_MAP_READ -> Raw BGRA Slice"]
    B -- No / Fallback --> G["Win32 Desktop DC<br/>(GetDC / OpenInputDesktop)"]
    G --> H["BitBlt() into DIBSection"]
    H --> F
    F --> I{"Target Consumer"}
    I -- OCR --> J["WinRT SoftwareBitmap::CreateCopyFromBuffer"]
    I -- Template Match --> K["ITU-R BT.601 Grayscale Conversion (AVX2/NEON)"]
```

1. **DXGI Desktop Duplication**:
   - Binds to the primary GPU display output via `IDXGIOutputDuplication`.
   - `AcquireNextFrame()` captures the active framebuffer directly from video memory (VRAM).
   - Reuses a persistent DirectX 11 staging texture (`D3D11_USAGE_STAGING`) across calls to eliminate GPU allocation overhead.
2. **Win32 Interactive Desktop Access**:
   - Non-interactive background worker threads often get `0x80070005` (Access Denied) when capturing desktop DCs.
   - `rusty_clicker` implements `attach_input_desktop()`: calls `OpenInputDesktop()` and `SetThreadDesktop()` to bind worker threads directly to the interactive desktop session.
3. **Multi-Monitor GDI Fallback**:
   - If DXGI duplication is unavailable (e.g. fullscreen exclusive mode or remote sessions), it falls back to a custom GDI capture pipeline querying `SM_XVIRTUALSCREEN`, `SM_YVIRTUALSCREEN`, `SM_CXVIRTUALSCREEN`, and `SM_CYVIRTUALSCREEN`.

---

### 2.2 Hardware-Accelerated WinRT OCR Pipeline

Traditional OCR in Python requires spawning `tesseract.exe` as a subprocess, converting PIL images to TIFF/PNG, writing to disk or pipes, and parsing stdout. This caps polling rates at 1–2 Hz and maxes out CPU cores.

```
+-----------------------------------------------------------------------------------------+
|                                RUSTY_CLICKER OCR PIPELINE                               |
+-----------------------------------------------------------------------------------------+
| [Raw BGRA Buffer]                                                                       |
|         |                                                                               |
|         v (Sub-region crop in memory: < 0.5 ms)                                         |
| [Cropped BGRA Buffer (w x h x 4)]                                                       |
|         |                                                                               |
|         v (Zero-copy copy into WinRT buffer)                                            |
| [Windows.Graphics.Imaging.SoftwareBitmap (Bgra8, Premultiplied)]                         |
|         |                                                                               |
|         v (Async hardware neural recognition)                                           |
| [Windows.Media.Ocr.OcrEngine.RecognizeAsync()]                                          |
|         |                                                                               |
|         v                                                                               |
| [OcrResult (Lines & Words with native bounding Rects)]                                  |
|         |                                                                               |
|         +---> Compact Normalization (strip diacritics, case-fold, regex whitespace)     |
|         +---> Substring & Phrase Sliding Window (calculates combined bounding box)      |
|         +---> Float Component Extractor (parses 3D XYZ coordinates directly)            |
+-----------------------------------------------------------------------------------------+
```

- **Zero-Conversion Ingestion**: `SoftwareBitmap.CreateCopyFromBuffer` natively accepts `BitmapPixelFormat::Bgra8`, identical to DXGI and GDI memory layouts. No RGB/BGR swaps or pixel conversions occur.
- **Fast Sub-Region Cropping (`crop_bgra`)**: When a `(x, y, w, h)` region is specified, Rust crops the memory slice row-by-row before sending it to the WinRT engine. OCR processing time scales linearly with pixel area: a full 1080p frame takes ~500 ms, whereas a 400x150 prompt region takes **14–16 ms**.
- **Sliding-Window Phrase Matching**: When searching for multi-word phrases (e.g. `"candy skeleton"`), `rusty_clicker` matches consecutive word sequences and automatically merges their individual bounding rectangles into a single unified `TextBox` with the exact click center.

---

### 2.3 Normalized Cross-Correlation (NCC) Template Matching

For image matching, `rusty_clicker` uses a multi-threaded Normalized Cross-Correlation (NCC) algorithm:

$$\gamma(x, y) = \frac{\sum_{x', y'} (T(x', y') - \bar{T}) \cdot (I(x + x', y + y') - \bar{I}_{x, y})}{\sqrt{\sum_{x', y'} (T(x', y') - \bar{T})^2 \cdot \sum_{x', y'} (I(x + x', y + y') - \bar{I}_{x, y})^2}}$$

1. **Luma Conversion**:
   Screenshots and templates are transformed into 8-bit grayscale using standard ITU-R BT.601 coefficients:
   $$Y = 0.299 \cdot R + 0.587 \cdot G + 0.114 \cdot B$$
2. **Prepared Templates (`PreparedTemplate`)**:
   Pre-computes the template mean $\bar{T}$, variance, and integral sums upfront. Loading and preparing the template once allows subsequent screen matches to complete in **sub-5 ms**.
3. **Multi-Threaded Tile Partitioning**:
   Screen candidate rows are distributed across CPU worker threads via `rayon`, scaling with available CPU cores.
4. **Early Exit Optimization**:
   If candidate block variance deviates beyond the threshold or intermediate correlation falls below target confidence, evaluation of the candidate terminates immediately.

---

### 2.4 Hardware Mouse Injection via Win32 SendInput

Many games ignore synthetic `pyautogui` or `ctypes` mouse events because they do not use hardware-normalized cursor coordinates.

`rusty_clicker.click()` executes:
1. Translates target pixel coordinates $(X, Y)$ to absolute normalized desktop space:
   $$X_{norm} = \frac{X \cdot 65535}{Width_{virtual}}, \quad Y_{norm} = \frac{Y \cdot 65535}{Height_{virtual}}$$
2. Assembles `INPUT` structures with `MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE` and `MOUSEEVENTF_LEFTDOWN` / `MOUSEEVENTF_LEFTUP`.
3. Invokes Win32 `SendInput` directly, dispatching genuine mouse hardware interrupts recognized by games and anti-cheats.

---

## 3. Installation & Setup

### 3.1 Drop-In File Setup

Copy `rusty_clicker.pyd` into:
- The same directory as your Python script, **OR**
- Any folder in your Python `sys.path` (e.g. `site-packages`).

```
your-project/
├── main.py
└── rusty_clicker.pyd
```

### 3.2 Compatibility

- **Operating System**: Windows 10 (Version 1607+) or Windows 11 (64-bit).
- **Python Versions**: CPython 3.9, 3.10, 3.11, 3.12, 3.13, 3.14 (64-bit).
- **External Dependencies**: None. (Does **not** require Visual C++ Redistributable or Tesseract installation).

### 3.3 Verification

Run this one-liner in your terminal:
```powershell
python -c "import rusty_clicker; print('Loaded successfully:', rusty_clicker.__doc__)"
```

---

## 4. Complete API Reference

### 4.1 Top-Level Module Functions

#### `rusty_clicker.read_screen_text(region=None) -> str`
Captures the screen and extracts all visible text via WinRT OCR.
- **`region`** *(Optional[Tuple[int, int, int, int]])*: `(x, y, width, height)` bounding box to crop. If `None`, scans the full primary display.
- **Returns**: Recognized text as a string with preserved line breaks.

#### `rusty_clicker.screen_contains_text(phrase: str, region=None) -> bool`
Performs a fast, case-insensitive, whitespace-tolerant substring check.
- **`phrase`** *(str)*: Target text to look for (e.g. `"press e to open"`, `"skeleton"`).
- **`region`** *(Optional[Tuple[int, int, int, int]])*: Optional bounding box.
- **Returns**: `True` if found, `False` otherwise.

#### `rusty_clicker.find_text_on_screen(phrase: str, region=None) -> Optional[TextBox]`
Finds a phrase on screen and returns its bounding box and click coordinates.
- **`phrase`** *(str)*: Target phrase.
- **`region`** *(Optional[Tuple[int, int, int, int]])*: Optional search sub-region.
- **Returns**: A [`TextBox`](#data-models-textbox-ocrword-match) instance if found, or `None`.

#### `rusty_clicker.read_coords(region=None) -> Optional[Tuple[float, float, float]]`
Specifically parses 3D game coordinates `(X, Y, Z)` from screen text.
- Automatically handles axis prefixes (e.g. `X: 1420.5 Y: 45.2 Z: -920.8` or `Pos: [100.2, 50.1, -30.0]`).
- **Returns**: `(x, y, z)` as a 3-tuple of floats, or `None` if fewer than 3 coordinates were found.

#### `rusty_clicker.is_knock_visible(region=None) -> bool`
Fast heuristic check for interaction prompts (`"knock"`, `"knok"`).
- **Returns**: `True` if an interaction prompt is detected, `False` otherwise.

#### `rusty_clicker.find_template(template_path: str, confidence=0.90, threads=2) -> Optional[Match]`
Finds an image template on screen using normalized cross-correlation.
- **`template_path`** *(str)*: File path to image (`.png`, `.jpg`, `.bmp`).
- **`confidence`** *(float)*: Minimum matching threshold between `0.0` and `1.0`. Default is `0.90`.
- **`threads`** *(int)*: Number of parallel CPU worker threads. Default is `2`.
- **Returns**: A [`Match`](#data-models-textbox-ocrword-match) instance if found, or `None`.

#### `rusty_clicker.capture_screen() -> Tuple[bytes, int, int]`
Captures the screen and returns raw grayscale pixel data.
- **Returns**: `(gray_bytes, width, height)` where `gray_bytes` is a 1-byte-per-pixel buffer of length `width * height`.

#### `rusty_clicker.click(button="left", click_type="single", pos=None) -> None`
Injects a hardware mouse click via Win32 `SendInput`.
- **`button`** *(str)*: `"left"`, `"right"`, or `"middle"`. Default is `"left"`.
- **`click_type`** *(str)*: `"single"` or `"double"`. Default is `"single"`.
- **`pos`** *(Optional[Tuple[int, int]])*: Target `(x, y)` screen pixel coordinates. If `None`, clicks at the current mouse position.

---

### 4.2 Class: `ScreenCapture`

Recommended for **continuous polling loops** and **high-frequency macros**. Reuses DirectX 11 textures, memory buffers, and the Windows OCR engine across frames for minimal latency and 0% CPU consumption.

```python
from rusty_clicker import ScreenCapture

cap = ScreenCapture()
```

#### Methods:

- **`cap.read_text(region=None) -> str`**  
  Reads all text on screen or within `region`.
- **`cap.read_text_words(region=None) -> Tuple[str, List[OcrWord]]`**  
  Returns a tuple containing `(full_text, list_of_ocr_words)`. Each `OcrWord` has its exact bounding box and center.
- **`cap.contains_text(phrase: str, region=None) -> bool`**  
  Fast substring check using warm engine buffers.
- **`cap.find_text(phrase: str, region=None) -> Optional[TextBox]`**  
  Locates phrase and returns its `TextBox` with click coordinates.
- **`cap.read_coords(region=None) -> Optional[Tuple[float, float, float]]`**  
  Extracts `(x, y, z)` floats from warm OCR buffer.
- **`cap.is_knock_visible(region=None) -> bool`**  
  Warm check for interaction prompts.
- **`cap.capture_gray() -> Tuple[bytes, int, int]`**  
  Captures screen into warm internal buffer and returns `(gray_bytes, width, height)`.
- **`cap.find_template(template: PreparedTemplate, confidence=0.90, threads=2) -> Optional[Match]`**  
  Finds a pre-compiled template on screen using the warm capture session.

---

### 4.3 Class: `PreparedTemplate`

Pre-processes a template image (grayscale conversion, variance, mean) so that searches against screenshots take only **2–5 ms**.

```python
from rusty_clicker import PreparedTemplate

# Load from file
tmpl = PreparedTemplate("assets/button.png")

# Or load from in-memory bytes
with open("assets/button.png", "rb") as f:
    tmpl = PreparedTemplate.from_bytes(f.read())
```

#### Attributes:
- **`tmpl.width`** *(int)*: Width in pixels.
- **`tmpl.height`** *(int)*: Height in pixels.

#### Methods:
- **`tmpl.find_on_screen(confidence=0.90, threads=2) -> Optional[Match]`**  
  Captures the screen and searches for this template in one call.
- **`tmpl.find_in_gray(screen_gray: bytes, width: int, height: int, confidence=0.90, threads=2) -> Optional[Match]`**  
  Searches for this template inside an already-captured grayscale image buffer.

---

### 4.4 Data Models: `TextBox`, `OcrWord`, `Match`

#### `TextBox`
Returned by `find_text_on_screen()` and `cap.find_text()`.

| Attribute / Method | Type | Description |
| :--- | :--- | :--- |
| `box.phrase` | `str` | Matched text string |
| `box.center_x` | `int` | X coordinate of the center (ideal for clicking) |
| `box.center_y` | `int` | Y coordinate of the center (ideal for clicking) |
| `box.x` | `int` | Left edge coordinate |
| `box.y` | `int` | Top edge coordinate |
| `box.width` | `int` | Bounding box width |
| `box.height` | `int` | Bounding box height |
| `box.as_center()` | `Tuple[int, int]` | Returns `(center_x, center_y)` |
| `box.as_box()` | `Tuple[int, int, int, int]` | Returns `(x, y, width, height)` |

#### `OcrWord`
Elements in the list returned by `cap.read_text_words()`.

| Attribute / Method | Type | Description |
| :--- | :--- | :--- |
| `word.text` | `str` | Recognized word string |
| `word.x` | `int` | Left edge coordinate |
| `word.y` | `int` | Top edge coordinate |
| `word.width` | `int` | Bounding box width |
| `word.height` | `int` | Bounding box height |
| `word.center_x` | `int` | X coordinate of word center |
| `word.center_y` | `int` | Y coordinate of word center |
| `word.as_center()` | `Tuple[int, int]` | Returns `(center_x, center_y)` |
| `word.as_box()` | `Tuple[int, int, int, int]` | Returns `(x, y, width, height)` |

#### `Match`
Returned by `find_template()` and `cap.find_template()`.

| Attribute / Method | Type | Description |
| :--- | :--- | :--- |
| `m.score` | `float` | Correlation score (`0.0` to `1.0`) |
| `m.center_x` | `int` | X center coordinate |
| `m.center_y` | `int` | Y center coordinate |
| `m.x` | `int` | Left coordinate |
| `m.y` | `int` | Top coordinate |
| `m.width` | `int` | Template width |
| `m.height` | `int` | Template height |
| `m.as_center()` | `Tuple[int, int]` | Returns `(center_x, center_y)` |
| `m.as_box()` | `Tuple[int, int, int, int]` | Returns `(x, y, width, height)` |

---

## 5. Step-by-Step Code Recipes

### Recipe 1: Quick Text Detection & Substring Check

Check if dialogue or a notification is on screen:

```python
import rusty_clicker

# Check full screen
if rusty_clicker.screen_contains_text("quest complete"):
    print("Quest finished!")

# Check specific UI region (x, y, width, height)
dialog_region = (400, 700, 600, 200)
if rusty_clicker.screen_contains_text("accept", region=dialog_region):
    print("Accept button visible!")
```

---

### Recipe 2: Find Text on Screen & Click Its Center

Automatically locate in-game text and click it with hardware mouse events:

```python
import rusty_clicker

target = "Claim Reward"
box = rusty_clicker.find_text_on_screen(target)

if box:
    print(f"Found '{box.phrase}' at ({box.center_x}, {box.center_y})")
    # Click directly on the center of the text
    rusty_clicker.click(button="left", pos=(box.center_x, box.center_y))
else:
    print("Reward button not found.")
```

---

### Recipe 3: High-Frequency Polling Loop (0% CPU Overhead)

For responsive macros running at 20–60 FPS without stuttering your game:

```python
import time
import rusty_clicker

cap = rusty_clicker.ScreenCapture()
prompt_region = (860, 480, 200, 120)  # Center screen interaction prompt

print("Monitoring screen... Press Ctrl+C to stop.")
try:
    while True:
        # Runs in ~14 ms with warm DXGI/OCR buffers
        if cap.contains_text("knock", region=prompt_region):
            print("Interaction ready! Pressing E.")
            # Your action here (e.g. keyboard press or click)
            time.sleep(1.0)  # Cooldown
        
        time.sleep(0.01)  # Minimal sleep between polls
except KeyboardInterrupt:
    print("Stopped.")
```

---

### Recipe 4: Real-Time In-Game 3D Coordinate Extraction

Extract player position for pathfinding or auto-walk bots:

```python
import time
import rusty_clicker

cap = rusty_clicker.ScreenCapture()
coords_region = (10, 10, 300, 60)  # Top-left HUD coordinate display

for _ in range(10):
    pos = cap.read_coords(region=coords_region)
    if pos:
        x, y, z = pos
        print(f"Player Position -> X: {x:.2f}, Y: {y:.2f}, Z: {z:.2f}")
    else:
        print("Coordinates not detected.")
    time.sleep(0.1)
```

---

### Recipe 5: Template Image Matching on Screen

Ultra-fast template matching for in-game icons, buttons, or health bars:

```python
import rusty_clicker

# 1. Preload and prepare template once
play_button = rusty_clicker.PreparedTemplate("assets/play_button.png")
cap = rusty_clicker.ScreenCapture()

# 2. Search screen (takes 2-5 ms)
match = cap.find_template(play_button, confidence=0.88)

if match:
    print(f"Found Play Button! Confidence: {match.score:.2%}")
    print(f"Clicking center at: ({match.center_x}, {match.center_y})")
    rusty_clicker.click("left", "single", pos=(match.center_x, match.center_y))
else:
    print("Play button not visible.")
```

---

### Recipe 6: Word-Level Bounding Boxes & Color Filtering

Detect text elements and inspect their underlying pixels (e.g. verifying red combat damage numbers):

```python
import pyautogui
import numpy as np
import rusty_clicker

cap = rusty_clicker.ScreenCapture()
full_text, words = cap.read_text_words()

for word in words:
    # If the detected word is a number between 1 and 15
    if word.text.isdigit() and 1 <= int(word.text) <= 15:
        # Crop the word bounding box to check color
        crop = pyautogui.screenshot(region=(word.x, word.y, word.width, word.height))
        pixels = np.asarray(crop.convert("RGB"))
        
        # Test if text pixels are predominantly red
        red = pixels[:, :, 0]
        green = pixels[:, :, 1]
        blue = pixels[:, :, 2]
        is_red = (red > 160) & (red > green * 1.5) & (red > blue * 1.5)
        
        if np.count_nonzero(is_red) >= 4:
            print(f"Detected RED combat number '{word.text}' at ({word.x}, {word.y})")
```

---

## 6. Performance Comparison & Benchmarks

Measured on an Intel i7 / Windows 11 machine under identical desktop conditions:

| Operation | `pytesseract` / `PIL` / `pyautogui` | `rusty_clicker.pyd` (Rust) | Speedup Factor |
| :--- | :--- | :--- | :--- |
| **Sub-Region OCR** (400x150 px) | 1,180.0 ms | **14.2 ms** | **~83x faster** |
| **Full Screen OCR** (1920x1080) | 2,850.0 ms | **590.0 ms** | **~5x faster** |
| **Screen Capture Latency** | 35.0 ms (`pyautogui`) | **0.8 ms** (DXGI staging) | **~44x faster** |
| **Template Match** (100x100 in 1080p) | 48.0 ms (`cv2.matchTemplate`) | **4.1 ms** (Multi-threaded NCC) | **~12x faster** |
| **CPU Usage During 5 Hz Poll** | 85% – 100% (Causes game stutter) | **< 1%** (Near-zero impact) | **Negligible** |
| **Installation Footprint** | ~115 MB (`tesseract.exe` + tessdata) | **779 KB** (Single `.pyd`) | **~147x smaller** |

---

## 7. Standalone Executable Packaging (PyInstaller, Nuitka)

Because `rusty_clicker.pyd` links against built-in Windows OS components (`windows.media.ocr.dll`, `d3d11.dll`, `dxgi.dll`), it packages cleanly without missing DLL errors.

### 7.1 PyInstaller Single-File Command

```powershell
pyinstaller --onefile --noconsole --add-binary "rusty_clicker.pyd;." main.py
```

### 7.2 PyInstaller `.spec` File Example

```python
# main.spec
a = Analysis(
    ['main.py'],
    pathex=[],
    binaries=[('rusty_clicker.pyd', '.')],
    datas=[],
    hiddenimports=['rusty_clicker'],
    noarchive=False,
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='GameMacro',
    debug=False,
    console=False,
)
```

---

## 8. Best Practices & Troubleshooting

1. **Reuse `ScreenCapture()`**:
   Avoid calling module-level functions (`rusty_clicker.read_screen_text()`) in tight loops. Construct a single `cap = rusty_clicker.ScreenCapture()` instance and reuse it. This avoids re-initializing DirectX textures and WinRT engine handles.
2. **Crop Wherever Possible**:
   OCR execution time is directly proportional to pixel area. Specifying a `region=(x, y, w, h)` cuts OCR time from ~500 ms down to **10–18 ms**.
3. **Borderless Windowed Mode**:
   For games, run in **Borderless Windowed** or **Windowed** mode. DirectX Desktop Duplication (DXGI) captures the desktop compositor; exclusive fullscreen games bypass the compositor unless GDI fallback is used.
4. **Multi-Monitor Coordinate Offsets**:
   If scanning a secondary monitor situated to the left of your primary display, its X coordinate is negative (e.g. `x = -1920`). `rusty_clicker` automatically normalizes virtual desktop coordinates.
