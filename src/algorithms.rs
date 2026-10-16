use image::{RgbImage, RgbaImage};
use opencv::{
    core::Mat,
    imgproc::{self, COLOR_RGB2GRAY, TM_CCOEFF_NORMED},
    prelude::*,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::POINT,
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleBitmap,
        CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HGDIOBJ,
        ReleaseDC, SRCCOPY, SelectObject,
    },
    UI::{
        Input::KeyboardAndMouse::{MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, mouse_event},
        WindowsAndMessaging::{
            GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
            SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SetCursorPos,
        },
    },
};

const MATCH_CONFIDENCE: f32 = 0.90;
const SCAN_INTERVAL: Duration = Duration::from_millis(10);
pub(crate) const CLICK_DELAYS_MS: [u64; 7] = [25, 50, 100, 250, 500, 1_000, 2_000];

pub(crate) struct PreparedTemplate {
    pub(crate) width: u32,
    pub(crate) height: u32,
    gray_pixels: Vec<u8>,
}

impl PreparedTemplate {
    /// Converts an image to grayscale with OpenCV, ignoring alpha like PyScreeze's image loader.
    pub(crate) fn new(source: RgbaImage) -> Result<Self, String> {
        let (width, height) = source.dimensions();
        let rgb: Vec<u8> = source
            .pixels()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        let gray_pixels = rgb_to_gray(&rgb, width, height)?;
        Ok(Self {
            width,
            height,
            gray_pixels,
        })
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0 && !self.gray_pixels.is_empty()
    }
}

pub(crate) fn default_click_delay_ms() -> u64 {
    25
}

pub(crate) fn normalize_click_delay_ms(delay: u64) -> u64 {
    if CLICK_DELAYS_MS.contains(&delay) {
        delay
    } else {
        default_click_delay_ms()
    }
}

pub(crate) struct WorkerState {
    pub(crate) stop: AtomicBool,
    pub(crate) running: AtomicBool,
    pub(crate) clicks: AtomicU64,
    pub(crate) click_delay_ms: AtomicU64,
    pub(crate) message: Mutex<String>,
}

/// Captures the desktop, searches the active target using OpenCV, and clicks the first match.
pub(crate) fn run_clicker(templates: Vec<PreparedTemplate>, worker: Arc<WorkerState>) {
    let mut click_count = 0u64;
    let mut error_message = None;
    let mut next_template_index = 0;
    while !worker.stop.load(Ordering::Acquire) {
        let scan_started = Instant::now();
        let (screenshot, origin_x, origin_y) = match capture_screen() {
            Ok(screenshot) => screenshot,
            Err(error) => {
                error_message = Some(format!("Screen capture failed: {error}"));
                break;
            }
        };

        let template = &templates[next_template_index];
        let found = match find_template(&screenshot, template, MATCH_CONFIDENCE, Some(&worker.stop))
        {
            Ok(found) => found,
            Err(error) => {
                error_message = Some(error);
                break;
            }
        };
        let mut clicked = false;
        if let Some((x, y, width, height)) = found {
            if worker.stop.load(Ordering::Acquire) {
                break;
            }
            let center_x = origin_x + x as i32 + (width / 2) as i32;
            let center_y = origin_y + y as i32 + (height / 2) as i32;
            unsafe {
                let mut cursor = POINT::default();
                if GetCursorPos(&mut cursor).is_ok()
                    && ((cursor.x == origin_x
                        || cursor.x == origin_x + screenshot.width() as i32 - 1)
                        && (cursor.y == origin_y
                            || cursor.y == origin_y + screenshot.height() as i32 - 1))
                {
                    error_message = Some(
                        "Stopped by the fail-safe (mouse moved to a screen corner).".to_owned(),
                    );
                    break;
                }
                if SetCursorPos(center_x, center_y).is_err() {
                    error_message = Some("Could not move the mouse cursor.".to_owned());
                    break;
                }
                mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
                mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
            }
            click_count += 1;
            worker.clicks.store(click_count, Ordering::Relaxed);
            next_template_index = (next_template_index + 1) % templates.len();
            clicked = true;
        }

        if error_message.is_some() {
            break;
        }
        let delay = if clicked {
            Duration::from_millis(worker.click_delay_ms.load(Ordering::Acquire))
        } else {
            SCAN_INTERVAL
                .saturating_sub(scan_started.elapsed())
                .max(Duration::from_millis(5))
        };
        sleep_interruptible(delay, &worker.stop);
    }

    if let Some(message) = error_message {
        *worker
            .message
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = message;
    }
    worker.running.store(false, Ordering::Release);
}

/// Sleeps in short intervals so stop requests remain responsive during configured click delays.
fn sleep_interruptible(duration: Duration, stop: &AtomicBool) {
    let deadline = Instant::now() + duration;
    while !stop.load(Ordering::Acquire) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

/// Converts RGB bytes to OpenCV grayscale, mirroring the Python detector's grayscale inputs.
fn rgb_to_gray(rgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let rows =
        i32::try_from(height).map_err(|_| "Image height exceeds OpenCV limits.".to_owned())?;
    let _columns =
        i32::try_from(width).map_err(|_| "Image width exceeds OpenCV limits.".to_owned())?;
    let storage = Mat::from_slice(rgb)
        .map_err(|error| format!("OpenCV could not read RGB image data: {error}"))?;
    let rgb_mat = storage
        .reshape(3, rows)
        .map_err(|error| format!("OpenCV could not shape RGB image data: {error}"))?;
    let mut gray = Mat::default();
    imgproc::cvt_color(
        &rgb_mat,
        &mut gray,
        COLOR_RGB2GRAY,
        0,
        opencv::core::AlgorithmHint::ALGO_HINT_DEFAULT,
    )
    .map_err(|error| format!("OpenCV could not convert RGB to grayscale: {error}"))?;
    gray.data_typed::<u8>()
        .map(|pixels| pixels.to_vec())
        .map_err(|error| format!("OpenCV returned invalid grayscale data: {error}"))
}

/// Applies OpenCV TM_CCOEFF_NORMED and returns the first row-major score above confidence,
/// as PyScreeze does for `locateAll(..., confidence=..., grayscale=True)`.
fn find_template(
    screenshot: &RgbImage,
    template: &PreparedTemplate,
    confidence: f32,
    stop: Option<&AtomicBool>,
) -> Result<Option<(u32, u32, u32, u32)>, String> {
    if !template.is_valid() {
        return Ok(None);
    }
    let (screen_width, screen_height) = screenshot.dimensions();
    if template.width > screen_width || template.height > screen_height {
        return Ok(None);
    }
    if stop.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Ok(None);
    }

    let screen_gray = rgb_to_gray(screenshot.as_raw(), screen_width, screen_height)?;
    let screen_rows =
        i32::try_from(screen_height).map_err(|_| "Screenshot height exceeds OpenCV limits.")?;
    let template_rows =
        i32::try_from(template.height).map_err(|_| "Template height exceeds OpenCV limits.")?;
    let screen_storage = Mat::from_slice(&screen_gray)
        .map_err(|error| format!("OpenCV could not prepare the screenshot: {error}"))?;
    let screen_mat = screen_storage
        .reshape(1, screen_rows)
        .map_err(|error| format!("OpenCV could not shape the screenshot: {error}"))?;
    let template_storage = Mat::from_slice(&template.gray_pixels)
        .map_err(|error| format!("OpenCV could not prepare the template: {error}"))?;
    let template_mat = template_storage
        .reshape(1, template_rows)
        .map_err(|error| format!("OpenCV could not shape the template: {error}"))?;
    let mut scores = Mat::default();
    imgproc::match_template_def(&screen_mat, &template_mat, &mut scores, TM_CCOEFF_NORMED)
        .map_err(|error| format!("OpenCV template matching failed: {error}"))?;

    if stop.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Ok(None);
    }
    let score_values = scores
        .data_typed::<f32>()
        .map_err(|error| format!("OpenCV returned invalid match scores: {error}"))?;
    let Some(index) = score_values.iter().position(|score| *score > confidence) else {
        return Ok(None);
    };
    let result_width = usize::try_from(scores.cols())
        .map_err(|_| "OpenCV returned an invalid match-score width.")?;
    if result_width == 0 {
        return Ok(None);
    }
    let x = u32::try_from(index % result_width)
        .map_err(|_| "OpenCV returned an invalid match position.")?;
    let y = u32::try_from(index / result_width)
        .map_err(|_| "OpenCV returned an invalid match position.")?;
    Ok(Some((x, y, template.width, template.height)))
}

/// Captures the virtual desktop and returns pixels together with its screen-space origin.
fn capture_screen() -> Result<(RgbImage, i32, i32), String> {
    unsafe {
        let origin_x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let origin_y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if width <= 0 || height <= 0 {
            return Err("Windows reported an invalid screen size.".to_owned());
        }

        let screen_dc = GetDC(None);
        if screen_dc.0.is_null() {
            return Err("Could not get the screen device context.".to_owned());
        }
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        if memory_dc.0.is_null() {
            ReleaseDC(None, screen_dc);
            return Err("Could not create a compatible device context.".to_owned());
        }
        let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
        if bitmap.0.is_null() {
            let _ = DeleteDC(memory_dc);
            ReleaseDC(None, screen_dc);
            return Err("Could not create a screenshot bitmap.".to_owned());
        }
        let previous_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));

        let capture_result = (|| {
            BitBlt(
                memory_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
                origin_x,
                origin_y,
                SRCCOPY | CAPTUREBLT,
            )
            .map_err(|error| format!("Could not copy screen pixels: {error}"))?;
            SelectObject(memory_dc, previous_bitmap);
            let pixel_bytes = (width as usize)
                .checked_mul(height as usize)
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or_else(|| "Screenshot dimensions are too large.".to_owned())?;
            let mut pixels = vec![0u8; pixel_bytes];
            let mut bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let copied_lines = GetDIBits(
                memory_dc,
                bitmap,
                0,
                height as u32,
                Some(pixels.as_mut_ptr().cast()),
                &mut bitmap_info,
                DIB_RGB_COLORS,
            );
            if copied_lines != height {
                return Err("Could not read screenshot pixels.".to_owned());
            }
            let rgb = RgbImage::from_fn(width as u32, height as u32, |x, y| {
                let offset = ((y as usize * width as usize) + x as usize) * 4;
                image::Rgb([pixels[offset + 2], pixels[offset + 1], pixels[offset]])
            });
            Ok((rgb, origin_x, origin_y))
        })();

        SelectObject(memory_dc, previous_bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        ReleaseDC(None, screen_dc);
        capture_result
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PreparedTemplate, default_click_delay_ms, find_template, normalize_click_delay_ms,
    };
    use crate::main_gui::{Preferences, Theme, scale};
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    #[test]
    fn finds_exact_grayscale_template_at_odd_pixel_offset() {
        let source = RgbaImage::from_fn(16, 16, |x, y| {
            Rgba([
                ((x * 47 + y * 17 + x * y * 13) % 256) as u8,
                ((x * 29 + y * 61 + x * y * 7) % 256) as u8,
                ((x * 83 + y * 37 + x * y * 19) % 256) as u8,
                255,
            ])
        });
        let template = PreparedTemplate::new(source.clone()).expect("OpenCV converts the image");
        let rgb = RgbImage::from_fn(source.width(), source.height(), |x, y| {
            let pixel = source.get_pixel(x, y);
            Rgb([pixel[0], pixel[1], pixel[2]])
        });
        let mut screen = RgbImage::from_pixel(40, 30, Rgb([0, 0, 0]));
        for y in 0..source.height() {
            for x in 0..source.width() {
                screen.put_pixel(x + 13, y + 9, *rgb.get_pixel(x, y));
            }
        }
        assert_eq!(
            find_template(&screen, &template, 0.92, None).expect("OpenCV matching succeeds"),
            Some((13, 9, 16, 16))
        );
    }

    #[test]
    fn chooses_the_first_row_major_match() {
        let source = RgbaImage::from_fn(8, 8, |x, y| {
            Rgba([
                ((x * 47 + y * 17 + x * y * 13) % 256) as u8,
                ((x * 29 + y * 61 + x * y * 7) % 256) as u8,
                ((x * 83 + y * 37 + x * y * 19) % 256) as u8,
                255,
            ])
        });
        let template = PreparedTemplate::new(source.clone()).expect("OpenCV converts the image");
        let rgb = RgbImage::from_fn(source.width(), source.height(), |x, y| {
            let pixel = source.get_pixel(x, y);
            Rgb([pixel[0], pixel[1], pixel[2]])
        });
        let mut screen = RgbImage::from_pixel(24, 20, Rgb([0, 0, 0]));
        for y in 0..source.height() {
            for x in 0..source.width() {
                screen.put_pixel(x + 10, y + 1, *rgb.get_pixel(x, y));
                screen.put_pixel(x + 2, y + 10, *rgb.get_pixel(x, y));
            }
        }
        assert_eq!(
            find_template(&screen, &template, 0.92, None).expect("OpenCV matching succeeds"),
            Some((10, 1, 8, 8))
        );
    }

    #[test]
    fn confidence_threshold_is_strict() {
        let source = RgbaImage::from_fn(8, 8, |x, y| {
            Rgba([
                ((x * 47 + y * 17 + x * y * 13) % 256) as u8,
                ((x * 29 + y * 61 + x * y * 7) % 256) as u8,
                ((x * 83 + y * 37 + x * y * 19) % 256) as u8,
                255,
            ])
        });
        let template = PreparedTemplate::new(source.clone()).expect("OpenCV converts the image");
        let rgb = RgbImage::from_fn(source.width(), source.height(), |x, y| {
            let pixel = source.get_pixel(x, y);
            Rgb([pixel[0], pixel[1], pixel[2]])
        });
        let mut screen = RgbImage::from_pixel(24, 20, Rgb([0, 0, 0]));
        for y in 0..source.height() {
            for x in 0..source.width() {
                screen.put_pixel(x + 10, y + 1, *rgb.get_pixel(x, y));
            }
        }
        assert_eq!(
            find_template(&screen, &template, 1.0, None).expect("OpenCV matching succeeds"),
            None
        );
    }

    #[test]
    fn rejects_template_larger_than_screenshot() {
        let screen = RgbImage::from_pixel(5, 5, Rgb([0, 0, 0]));
        let template =
            PreparedTemplate::new(RgbaImage::from_pixel(16, 16, Rgba([200, 100, 20, 255])))
                .expect("OpenCV converts the image");
        assert_eq!(
            find_template(&screen, &template, 0.92, None).expect("dimensions are handled"),
            None
        );
    }

    #[test]
    fn returns_no_match_when_stop_is_requested() {
        let screen = RgbImage::from_pixel(40, 30, Rgb([20, 35, 70]));
        let template =
            PreparedTemplate::new(RgbaImage::from_pixel(4, 4, Rgba([242, 128, 15, 255])))
                .expect("OpenCV converts the image");
        let stop = std::sync::atomic::AtomicBool::new(true);
        assert_eq!(
            find_template(&screen, &template, 0.92, Some(&stop))
                .expect("a stop request is not an error"),
            None
        );
    }

    #[test]
    fn alpha_is_ignored_like_py_screeze_template_loading() {
        let image = RgbaImage::from_fn(8, 8, |x, _y| {
            if x < 4 {
                Rgba([255, 100, 30, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        let template = PreparedTemplate::new(image).expect("OpenCV converts the image");
        let transparent_gray = template.gray_pixels[4];
        assert_eq!(transparent_gray, 0);
        assert!(template.is_valid());
    }

    #[test]
    fn old_preferences_use_default_click_delay_and_theme() {
        let preferences: Preferences =
            serde_json::from_str(r#"{"image_paths":[],"always_on_top":true}"#)
                .expect("existing preferences remain readable");
        assert_eq!(preferences.click_delay_ms, default_click_delay_ms());
        assert_eq!(preferences.theme, Theme::Dark);
        assert_eq!(preferences.image_count, 10);
        assert!(preferences.always_on_top);
    }

    #[test]
    fn selected_settings_round_trip_through_preferences() {
        let preferences = Preferences {
            image_paths: vec![Some("target.png".to_owned())],
            always_on_top: true,
            click_delay_ms: 500,
            theme: Theme::Purple,
            image_count: 37,
        };
        let encoded = serde_json::to_string(&preferences).expect("preferences serialize");
        let decoded: Preferences = serde_json::from_str(&encoded).expect("preferences deserialize");
        assert_eq!(decoded.click_delay_ms, 500);
        assert_eq!(decoded.theme, Theme::Purple);
        assert_eq!(decoded.image_count, 37);
        assert_eq!(decoded.image_paths, preferences.image_paths);
    }

    #[test]
    fn invalid_saved_delay_uses_the_default() {
        assert_eq!(normalize_click_delay_ms(0), 25);
        assert_eq!(normalize_click_delay_ms(250), 250);
    }

    #[test]
    fn ui_dimensions_scale_with_windows_dpi() {
        assert_eq!(scale(100, 1.0), 100);
        assert_eq!(scale(100, 1.5), 150);
    }
}
