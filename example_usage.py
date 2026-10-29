"""
RustyClicker Python Library - Example Usage
Demonstrates high-speed DirectX 11 / DXGI Desktop Capture and SIMD ZNCC Template Matching.
"""

import time
import rusty_clicker


def demo_screen_capture():
    print("=== 1. High-Performance Screen Capture Demo ===")
    session = rusty_clicker.ScreenCapture()

    # Capture 5 frames and measure speed
    for i in range(5):
        t0 = time.perf_counter()
        width, height, gray_bytes = session.capture_gray()
        elapsed_ms = (time.perf_counter() - t0) * 1000
        print(f"Frame {i + 1}: {width}x{height} captured in {elapsed_ms:.2f} ms ({len(gray_bytes)} bytes)")


def demo_template_matching():
    print("\n=== 2. Template Matching Demo ===")
    
    # Example template path (replace with your target image file)
    template_path = r"..\firefox_o4vumVr4Da.png"
    
    try:
        # Pre-compile multi-scale pyramid template once
        template = rusty_clicker.PreparedTemplate(template_path)
        print(f"Loaded template '{template_path}': {template.width}x{template.height} px")

        # Search directly on the live screen with 4 Rayon threads and 90% confidence
        t0 = time.perf_counter()
        match = template.find_on_screen(confidence=0.90, threads=4)
        elapsed_ms = (time.perf_counter() - t0) * 1000

        if match:
            print(f"Target found in {elapsed_ms:.2f} ms!")
            print(f"  Coordinates: top-left=({match.x}, {match.y}), dimensions={match.width}x{match.height}")
            print(f"  Center: ({match.center_x}, {match.center_y})")
            print(f"  Confidence Score: {match.score:.4f}")

            # Optional: Move mouse and click center
            # rusty_clicker.click("left", "single", (match.center_x, match.center_y))
        else:
            print(f"Target not currently visible on screen (scanned in {elapsed_ms:.2f} ms).")

    except Exception as e:
        print(f"Template demo note: {e}")


def demo_click():
    print("\n=== 3. Microsecond SendInput Mouse Click Demo ===")
    print("Example usage (commented out to avoid unintended clicks):")
    print("  rusty_clicker.click('left', 'single')")
    print("  rusty_clicker.click('right', 'double', pos=(500, 500))")


if __name__ == "__main__":
    demo_screen_capture()
    demo_template_matching()
    demo_click()

