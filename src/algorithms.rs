#[cfg(test)]
use image::RgbImage;
use image::RgbaImage;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::POINT,
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleDC,
        CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, HGDIOBJ,
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
const MATCH_THREADS: usize = 2;
pub(crate) const CLICK_DELAYS_MS: [u64; 7] = [25, 50, 100, 250, 500, 1_000, 2_000];

pub(crate) struct PreparedTemplate {
    pub(crate) width: u32,
    pub(crate) height: u32,
    gray_energy: f64,
    matcher: Option<corrmatch::Matcher>,
}

impl PreparedTemplate {
    /// Prepares a grayscale, mean-centered template for normalized correlation.
    pub(crate) fn new(source: RgbaImage) -> Result<Self, String> {
        let (width, height) = source.dimensions();
        if width == 0 || height == 0 {
            return Err("Image dimensions must be non-zero.".to_owned());
        }
        let rgb: Vec<u8> = source
            .pixels()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        let gray = rgb_to_gray(&rgb, width, height)?;
        let pixel_count = gray.len() as i64;
        let gray_sum: i64 = gray.iter().map(|&pixel| i64::from(pixel)).sum();
        let integer_mean = gray_sum / pixel_count;
        let gray_energy: i64 = gray
            .iter()
            .map(|&pixel| {
                let centered = i64::from(pixel) - integer_mean;
                centered * centered
            })
            .sum();
        let gray_energy = gray_energy as f64;
        let width_usize =
            usize::try_from(width).map_err(|_| "Image width exceeds platform limits.")?;
        let height_usize =
            usize::try_from(height).map_err(|_| "Image height exceeds platform limits.")?;
        let matcher = if gray_energy > 1e-12 {
            let matcher_template = corrmatch::Template::new(gray, width_usize, height_usize)
                .map_err(|error| format!("Could not create image matcher template: {error}"))?;
            let mut compile_config = corrmatch::CompileConfigNoRot::default();
            compile_config.max_levels = 4;
            let compiled_template =
                corrmatch::CompiledTemplate::compile_unrotated(&matcher_template, compile_config)
                    .map_err(|error| format!("Could not compile image matcher template: {error}"))?;
            let mut match_config = corrmatch::MatchConfig::default();
            match_config.parallel = true;
            match_config.max_image_levels = 4;
            Some(corrmatch::Matcher::new(compiled_template).with_config(match_config))
        } else {
            None
        };
        Ok(Self {
            width,
            height,
            gray_energy,
            matcher,
        })
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0 && self.gray_energy > 1e-12 && self.matcher.is_some()
    }

    fn find_match(
        &self,
        screen_gray: &[u8],
        screen_width: u32,
        screen_height: u32,
        confidence: f32,
        stop: Option<&AtomicBool>,
    ) -> Result<Option<(u32, u32, u32, u32)>, String> {
        if !self.is_valid()
            || self.width > screen_width
            || self.height > screen_height
            || stop.is_some_and(|flag| flag.load(Ordering::Acquire))
        {
            return Ok(None);
        }
        let Some(matcher) = &self.matcher else {
            return Ok(None);
        };
        let width = usize::try_from(screen_width)
            .map_err(|_| "Screenshot width exceeds platform limits.")?;
        let height = usize::try_from(screen_height)
            .map_err(|_| "Screenshot height exceeds platform limits.")?;
        let image = corrmatch::ImageView::from_slice(screen_gray, width, height)
            .map_err(|error| format!("Could not prepare screenshot for matching: {error}"))?;
        let pool = match_pool()?;
        let result = pool
            .install(|| matcher.match_image(image))
            .map_err(|error| format!("Image matching failed: {error}"))?;
        if stop.is_some_and(|flag| flag.load(Ordering::Acquire)) || result.score <= confidence {
            return Ok(None);
        }
        let max_x = screen_width - self.width;
        let max_y = screen_height - self.height;
        let x = (result.x.round() as u32).min(max_x);
        let y = (result.y.round() as u32).min(max_y);
        Ok(Some((x, y, self.width, self.height)))
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
    pub(crate) scan_duration_us: AtomicU64,
    pub(crate) message: Mutex<String>,
}

static MATCH_POOL: OnceLock<Result<ThreadPool, String>> = OnceLock::new();
fn match_pool() -> Result<&'static ThreadPool, String> {
    match MATCH_POOL.get_or_init(|| {
        ThreadPoolBuilder::new()
            .num_threads(MATCH_THREADS)
            .thread_name(|index| format!("rusty-matcher-{index}"))
            .build()
            .map_err(|error| format!("Could not initialize image-matching workers: {error}"))
    }) {
        Ok(pool) => Ok(pool),
        Err(error) => Err(error.clone()),
    }
}

/// Captures, matches, and clicks in a loop using a persistent CPU thread pool.
pub(crate) fn run_clicker(templates: Vec<PreparedTemplate>, worker: Arc<WorkerState>) {
    let mut click_count = 0u64;
    let mut error_message = None;
    let mut next_template_index = 0;
    let mut capture = None;
    let mut screen_gray = Vec::new();
    while !worker.stop.load(Ordering::Acquire) {
        let scan_started = Instant::now();
        let (screen_width, screen_height, origin_x, origin_y) =
            match capture_screen(&mut capture, &mut screen_gray) {
                Ok(screen) => screen,
                Err(error) => {
                    error_message = Some(format!("Screen capture failed: {error}"));
                    break;
                }
            };

        let template = &templates[next_template_index];
        let found = match template.find_match(
            &screen_gray,
            screen_width,
            screen_height,
            MATCH_CONFIDENCE,
            Some(&worker.stop),
        ) {
            Ok(found) => found,
            Err(error) => {
                error_message = Some(error);
                break;
            }
        };
        worker.scan_duration_us.store(
            u64::try_from(scan_started.elapsed().as_micros()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
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
                    && ((cursor.x == origin_x || cursor.x == origin_x + screen_width as i32 - 1)
                        && (cursor.y == origin_y
                            || cursor.y == origin_y + screen_height as i32 - 1))
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
                .saturating_sub(Duration::from_micros(
                    worker.scan_duration_us.load(Ordering::Relaxed),
                ))
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

/// Converts RGB bytes to grayscale using OpenCV-compatible fixed-point coefficients.
fn rgb_to_gray(rgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let expected_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| "Image dimensions are too large.".to_owned())?;
    if rgb.len() != expected_bytes {
        return Err("RGB image data does not match its dimensions.".to_owned());
    }
    let mut gray = Vec::with_capacity(expected_bytes / 3);
    for pixel in rgb.chunks_exact(3) {
        gray.push(rgb_pixel_to_gray(pixel[0], pixel[1], pixel[2]));
    }
    Ok(gray)
}

#[inline]
fn rgb_pixel_to_gray(red: u8, green: u8, blue: u8) -> u8 {
    let gray = 4_899 * u32::from(red) + 9_617 * u32::from(green) + 1_868 * u32::from(blue) + 8_192;
    (gray >> 14) as u8
}

fn bgra_to_gray(bgra: &[u8], width: u32, height: u32, gray: &mut Vec<u8>) -> Result<(), String> {
    let expected_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "Screenshot dimensions are too large.".to_owned())?;
    if bgra.len() != expected_bytes {
        return Err("Captured pixel data does not match the screen dimensions.".to_owned());
    }
    let pixel_count = expected_bytes / 4;
    gray.resize(pixel_count, 0);
    for (pixel, output) in bgra.chunks_exact(4).zip(gray) {
        *output = rgb_pixel_to_gray(pixel[2], pixel[1], pixel[0]);
    }
    Ok(())
}

/// Finds the first row-major match using normalized cross-correlation.
#[cfg(test)]
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
    template.find_match(&screen_gray, screen_width, screen_height, confidence, stop)
}

struct ScreenCapture {
    screen_dc: HDC,
    memory_dc: HDC,
    bitmap: HBITMAP,
    previous_bitmap: HGDIOBJ,
    pixels: *mut u8,
    width: i32,
    height: i32,
}

impl ScreenCapture {
    fn new(width: i32, height: i32) -> Result<Self, String> {
        usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4))
            .filter(|&bytes| bytes <= isize::MAX as usize)
            .ok_or_else(|| "Screen dimensions are too large to capture safely.".to_owned())?;
        unsafe {
            let screen_dc = GetDC(None);
            if screen_dc.0.is_null() {
                return Err("Could not get the screen device context.".to_owned());
            }
            let memory_dc = CreateCompatibleDC(Some(screen_dc));
            if memory_dc.0.is_null() {
                ReleaseDC(None, screen_dc);
                return Err("Could not create a compatible device context.".to_owned());
            }
            let bitmap_info = BITMAPINFO {
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
            let mut pixels = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(
                Some(screen_dc),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut pixels,
                None,
                0,
            ) {
                Ok(bitmap) => bitmap,
                Err(error) => {
                    let _ = DeleteDC(memory_dc);
                    ReleaseDC(None, screen_dc);
                    return Err(format!(
                        "Could not create a direct-access screenshot bitmap: {error}"
                    ));
                }
            };
            if bitmap.0.is_null() || pixels.is_null() {
                if !bitmap.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(bitmap.0));
                }
                let _ = DeleteDC(memory_dc);
                ReleaseDC(None, screen_dc);
                return Err("Could not create a direct-access screenshot bitmap.".to_owned());
            }
            let previous_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
            if previous_bitmap.0.is_null() || previous_bitmap.0 as isize == -1 {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                let _ = DeleteDC(memory_dc);
                ReleaseDC(None, screen_dc);
                return Err("Could not select the screenshot bitmap.".to_owned());
            }
            Ok(Self {
                screen_dc,
                memory_dc,
                bitmap,
                previous_bitmap,
                pixels: pixels.cast(),
                width,
                height,
            })
        }
    }
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.memory_dc, self.previous_bitmap);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.memory_dc);
            ReleaseDC(None, self.screen_dc);
        }
    }
}

/// Reuses a top-down DIB so capture needs only BitBlt and a direct grayscale conversion.
fn capture_screen(
    capture: &mut Option<ScreenCapture>,
    gray: &mut Vec<u8>,
) -> Result<(u32, u32, i32, i32), String> {
    let origin_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let origin_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    if width <= 0 || height <= 0 {
        return Err("Windows reported an invalid screen size.".to_owned());
    }
    if capture
        .as_ref()
        .is_none_or(|capture| capture.width != width || capture.height != height)
    {
        *capture = Some(ScreenCapture::new(width, height)?);
    }
    let capture = capture
        .as_mut()
        .ok_or_else(|| "Could not initialize screen capture.".to_owned())?;
    unsafe {
        BitBlt(
            capture.memory_dc,
            0,
            0,
            width,
            height,
            Some(capture.screen_dc),
            origin_x,
            origin_y,
            SRCCOPY | CAPTUREBLT,
        )
        .map_err(|error| format!("Could not copy screen pixels: {error}"))?;
        let pixel_bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| "Screenshot dimensions are too large.".to_owned())?;
        let bgra = std::slice::from_raw_parts(capture.pixels, pixel_bytes);
        bgra_to_gray(bgra, width as u32, height as u32, gray)?;
    }
    Ok((width as u32, height as u32, origin_x, origin_y))
}

#[cfg(test)]
mod tests {
    use super::{
        PreparedTemplate, bgra_to_gray, default_click_delay_ms, find_template,
        normalize_click_delay_ms, rgb_to_gray,
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
    fn finds_a_match_when_the_target_appears_more_than_once() {
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
        let found = find_template(&screen, &template, 0.92, None).expect("image matching succeeds");
        assert!(matches!(found, Some((2, 10, 8, 8)) | Some((10, 1, 8, 8))));
    }

    #[test]
    fn matches_with_brightness_and_contrast_variation() {
        let source = RgbaImage::from_fn(12, 12, |x, y| {
            let value = ((x * 37 + y * 53 + x * y * 11) % 220 + 20) as u8;
            Rgba([value, value, value, 255])
        });
        let template = PreparedTemplate::new(source.clone()).expect("Rust prepares the image");
        let mut screen = RgbImage::from_pixel(32, 24, Rgb([0, 0, 0]));
        for y in 0..source.height() {
            for x in 0..source.width() {
                let original = source.get_pixel(x, y)[0];
                let adjusted = (f32::from(original) * 0.75 + 28.0).round() as u8;
                screen.put_pixel(x + 7, y + 5, Rgb([adjusted, adjusted, adjusted]));
            }
        }
        assert_eq!(
            find_template(&screen, &template, 0.90, None).expect("matching succeeds"),
            Some((7, 5, 12, 12))
        );
    }

    #[test]
    fn converts_rgb_to_grayscale_with_opencv_coefficients() {
        assert_eq!(
            rgb_to_gray(&[255, 0, 0, 0, 255, 0, 0, 0, 255], 3, 1)
                .expect("RGB buffer dimensions are valid"),
            vec![76, 150, 29]
        );
    }

    #[test]
    fn converts_captured_bgra_pixels_to_the_same_grayscale() {
        let mut gray = Vec::new();
        bgra_to_gray(&[0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0, 0], 3, 1, &mut gray)
            .expect("BGRA buffer dimensions are valid");
        assert_eq!(gray, vec![76, 150, 29]);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn release_full_hd_scan_benchmark() {
        let screenshot = RgbImage::from_fn(1920, 1080, |x, y| {
            let value = (x
                .wrapping_mul(1_664_525)
                .wrapping_add(y.wrapping_mul(1_013_904_223))
                .wrapping_add((x ^ y).wrapping_mul(101)))
            .rotate_left(13) as u8;
            Rgb([value, value, value])
        });
        let template_image = RgbaImage::from_fn(64, 64, |x, y| {
            let value = (x
                .wrapping_mul(2_654_435_761)
                .wrapping_add(y.wrapping_mul(2_246_822_519))
                .wrapping_add((x ^ y).wrapping_mul(313)))
            .rotate_left(9) as u8;
            Rgba([value, value, value, 255])
        });
        let template = PreparedTemplate::new(template_image.clone()).expect("template prepares");
        let started = std::time::Instant::now();
        let found = find_template(&screenshot, &template, 0.90, None).expect("matching completes");
        println!(
            "1920x1080, 64x64 no-match scan: {:.2} ms, result: {found:?}",
            started.elapsed().as_secs_f64() * 1_000.0
        );
        assert_eq!(found, None);

        let target_x = 917;
        let target_y = 511;
        let mut exact_screen = screenshot;
        for y in 0..template.height {
            for x in 0..template.width {
                let target_pixel = template_image.get_pixel(x, y);
                exact_screen.put_pixel(
                    target_x + x,
                    target_y + y,
                    Rgb([target_pixel[0], target_pixel[1], target_pixel[2]]),
                );
            }
        }
        let started = std::time::Instant::now();
        let found =
            find_template(&exact_screen, &template, 0.90, None).expect("matching completes");
        println!(
            "1920x1080, 64x64 exact-match scan: {:.2} ms, result: {found:?}",
            started.elapsed().as_secs_f64() * 1_000.0
        );
        assert_eq!(found, Some((target_x, target_y, 64, 64)));
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
    fn handles_a_single_candidate_row() {
        let source = RgbaImage::from_fn(8, 6, |x, y| {
            let value = ((x * 41 + y * 67 + x * y * 13) % 256) as u8;
            Rgba([value, value, value, 255])
        });
        let template = PreparedTemplate::new(source.clone()).expect("template prepares");
        let mut screen = RgbImage::from_pixel(14, 6, Rgb([0, 0, 0]));
        for y in 0..source.height() {
            for x in 0..source.width() {
                let value = source.get_pixel(x, y)[0];
                screen.put_pixel(x + 4, y, Rgb([value, value, value]));
            }
        }
        assert_eq!(
            find_template(&screen, &template, 0.90, None).expect("matching succeeds"),
            Some((4, 0, 8, 6))
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
    fn alpha_is_ignored_during_grayscale_template_loading() {
        let image = RgbaImage::from_fn(8, 8, |x, _y| {
            if x < 4 {
                Rgba([255, 100, 30, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        let template = PreparedTemplate::new(image).expect("Rust prepares the image");
        assert!(template.is_valid());
    }

    #[test]
    fn rejects_a_constant_template_without_texture() {
        let screen = RgbImage::from_pixel(12, 12, Rgb([255, 128, 32]));
        let template =
            PreparedTemplate::new(RgbaImage::from_pixel(4, 4, Rgba([255, 128, 32, 255])))
                .expect("Rust prepares the image");
        assert_eq!(
            find_template(&screen, &template, 0.90, None).expect("matching succeeds"),
            None
        );
        assert!(!template.is_valid());
    }

    #[test]
    fn rejects_zero_sized_templates_without_dividing_by_zero() {
        assert!(PreparedTemplate::new(RgbaImage::new(0, 0)).is_err());
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
