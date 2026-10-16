#[cfg(test)]
use image::RgbImage;
use image::RgbaImage;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    cell::RefCell,
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
    centered_gray: Vec<i16>,
    centered_sum: i64,
    gray_energy: f64,
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
        let centered_gray: Vec<i16> = gray
            .iter()
            .map(|&pixel| (i64::from(pixel) - integer_mean) as i16)
            .collect();
        let centered_sum = gray_sum - integer_mean * pixel_count;
        let square_sum: i64 = centered_gray
            .iter()
            .map(|&pixel| i64::from(pixel) * i64::from(pixel))
            .sum();
        let gray_energy =
            square_sum as f64 - (centered_sum * centered_sum) as f64 / pixel_count as f64;
        Ok(Self {
            width,
            height,
            centered_gray,
            centered_sum,
            gray_energy,
        })
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && !self.centered_gray.is_empty()
            && self.gray_energy > 1e-12
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

struct ColumnSums {
    sums: Vec<u64>,
    squares: Vec<u64>,
}

impl ColumnSums {
    const fn new() -> Self {
        Self {
            sums: Vec::new(),
            squares: Vec::new(),
        }
    }
}

thread_local! {
    static COLUMN_SUMS: RefCell<ColumnSums> = const { RefCell::new(ColumnSums::new()) };
}

static MATCH_POOL: OnceLock<Result<ThreadPool, String>> = OnceLock::new();
type DotProductKernel = unsafe fn(&[u8], &[i16]) -> i64;

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
        let found = match find_template_gray(
            &screen_gray,
            screen_width,
            screen_height,
            template,
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
    find_template_gray(
        &screen_gray,
        screen_width,
        screen_height,
        template,
        confidence,
        stop,
    )
}

fn find_template_gray(
    screen_gray: &[u8],
    screen_width: u32,
    screen_height: u32,
    template: &PreparedTemplate,
    confidence: f32,
    stop: Option<&AtomicBool>,
) -> Result<Option<(u32, u32, u32, u32)>, String> {
    if !template.is_valid() {
        return Ok(None);
    }
    if template.width > screen_width || template.height > screen_height {
        return Ok(None);
    }
    if stop.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Ok(None);
    }
    let expected_screen_pixels = usize::try_from(screen_width)
        .ok()
        .and_then(|width| {
            usize::try_from(screen_height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| "Screenshot dimensions are too large.".to_owned())?;
    if screen_gray.len() != expected_screen_pixels {
        return Err("Grayscale screen data does not match its dimensions.".to_owned());
    }
    let screen_width =
        usize::try_from(screen_width).map_err(|_| "Screenshot width exceeds platform limits.")?;
    let screen_height =
        usize::try_from(screen_height).map_err(|_| "Screenshot height exceeds platform limits.")?;
    let template_width =
        usize::try_from(template.width).map_err(|_| "Template width exceeds platform limits.")?;
    let template_height =
        usize::try_from(template.height).map_err(|_| "Template height exceeds platform limits.")?;
    let result_width = screen_width - template_width + 1;
    let result_height = screen_height - template_height + 1;
    let split = result_height.div_ceil(2);
    let found_in_top = AtomicBool::new(false);
    let pool = match_pool()?;
    // The fixed pool keeps the two row ranges parallel without spawning threads per scan.
    let (top_match, bottom_match) = pool.install(|| {
        rayon::join(
            || {
                let found = COLUMN_SUMS.with_borrow_mut(|columns| {
                    find_in_rows(
                        screen_gray,
                        screen_width,
                        result_width,
                        template_width,
                        template_height,
                        template,
                        0,
                        split,
                        confidence,
                        stop,
                        None,
                        columns,
                    )
                });
                if found.is_some() {
                    found_in_top.store(true, Ordering::Release);
                }
                found
            },
            || {
                COLUMN_SUMS.with_borrow_mut(|columns| {
                    find_in_rows(
                        screen_gray,
                        screen_width,
                        result_width,
                        template_width,
                        template_height,
                        template,
                        split,
                        result_height,
                        confidence,
                        stop,
                        Some(&found_in_top),
                        columns,
                    )
                })
            },
        )
    });
    let found = match (top_match, bottom_match) {
        (Some(top), Some(bottom)) => Some(if top.1 <= bottom.1 { top } else { bottom }),
        (Some(found), None) | (None, Some(found)) => Some(found),
        (None, None) => None,
    };
    Ok(found.map(|(x, y)| (x, y, template.width, template.height)))
}

fn find_in_rows(
    screen: &[u8],
    screen_width: usize,
    result_width: usize,
    template_width: usize,
    template_height: usize,
    template: &PreparedTemplate,
    start_y: usize,
    end_y: usize,
    confidence: f32,
    stop: Option<&AtomicBool>,
    earlier_match: Option<&AtomicBool>,
    columns: &mut ColumnSums,
) -> Option<(u32, u32)> {
    if start_y >= end_y {
        return None;
    }
    let sample_count = template.centered_gray.len() as u64;
    columns.sums.resize(screen_width, 0);
    columns.squares.resize(screen_width, 0);
    columns.sums.fill(0);
    columns.squares.fill(0);

    for row in start_y..start_y + template_height {
        let offset = row * screen_width;
        for x in 0..screen_width {
            let pixel = u64::from(screen[offset + x]);
            columns.sums[x] += pixel;
            columns.squares[x] += pixel * pixel;
        }
    }

    let confidence_squared = f64::from(confidence).powi(2);
    let dot_kernel = dot_product_kernel();
    for y in start_y..end_y {
        if stop.is_some_and(|flag| flag.load(Ordering::Acquire))
            || earlier_match.is_some_and(|flag| flag.load(Ordering::Acquire))
        {
            return None;
        }
        let mut sum = columns.sums[..template_width].iter().copied().sum::<u64>();
        let mut square_sum = columns.squares[..template_width]
            .iter()
            .copied()
            .sum::<u64>();

        for x in 0..result_width {
            // Rolling sums give each window's variance in constant time; only the dot product
            // still needs to inspect template pixels.
            let window_energy = square_sum as f64 - sum as f64 * sum as f64 / sample_count as f64;
            if window_energy > 1e-12 {
                let dot = dot_product(
                    &screen[y * screen_width + x..],
                    screen_width,
                    template_width,
                    &template.centered_gray,
                    dot_kernel,
                );
                let centered_dot =
                    dot as f64 - sum as f64 * template.centered_sum as f64 / sample_count as f64;
                // A squared comparison preserves the threshold while avoiding a sqrt per window.
                if centered_dot > 0.0
                    && centered_dot * centered_dot
                        > confidence_squared * template.gray_energy * window_energy
                {
                    return Some((x as u32, y as u32));
                }
            }
            if x + 1 < result_width {
                sum = sum - columns.sums[x] + columns.sums[x + template_width];
                square_sum = square_sum - columns.squares[x] + columns.squares[x + template_width];
            }
            if x & 63 == 0
                && (stop.is_some_and(|flag| flag.load(Ordering::Acquire))
                    || earlier_match.is_some_and(|flag| flag.load(Ordering::Acquire)))
            {
                return None;
            }
        }

        if y + 1 < end_y {
            let outgoing = y * screen_width;
            let incoming = (y + template_height) * screen_width;
            for x in 0..screen_width {
                let old = u64::from(screen[outgoing + x]);
                let new = u64::from(screen[incoming + x]);
                columns.sums[x] = columns.sums[x] - old + new;
                columns.squares[x] = columns.squares[x] - old * old + new * new;
            }
        }
    }
    None
}

/// Computes a SIMD dot product with AVX2/SSE2 where available.
#[inline]
fn dot_product(
    screen: &[u8],
    stride: usize,
    template_width: usize,
    template: &[i16],
    kernel: DotProductKernel,
) -> i64 {
    let mut sum = 0i64;
    for (row_index, template_row) in template.chunks_exact(template_width).enumerate() {
        let row_start = row_index * stride;
        sum += unsafe { kernel(&screen[row_start..row_start + template_width], template_row) };
    }
    sum
}

#[cfg(target_arch = "x86_64")]
fn dot_product_kernel() -> DotProductKernel {
    if std::arch::is_x86_feature_detected!("avx2") {
        dot_product_avx2
    } else {
        dot_product_sse2
    }
}

#[cfg(not(target_arch = "x86_64"))]
unsafe fn dot_product_scalar(screen: &[u8], template: &[i16]) -> i64 {
    screen
        .iter()
        .zip(template)
        .map(|(&pixel, &centered)| i64::from(pixel) * i64::from(centered))
        .sum()
}

#[cfg(not(target_arch = "x86_64"))]
fn dot_product_kernel() -> DotProductKernel {
    dot_product_scalar
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn dot_product_sse2(screen: &[u8], template: &[i16]) -> i64 {
    use std::arch::x86_64::*;
    let mut low_sum = _mm_setzero_si128();
    let mut high_sum = _mm_setzero_si128();
    let mut offset = 0;
    while offset + 8 <= template.len() {
        let pixel_bytes = unsafe { _mm_loadl_epi64(screen.as_ptr().add(offset).cast()) };
        let pixels = _mm_unpacklo_epi8(pixel_bytes, _mm_setzero_si128());
        let centered = unsafe { _mm_loadu_si128(template.as_ptr().add(offset).cast()) };
        let pairs = _mm_madd_epi16(pixels, centered);
        let signs = _mm_srai_epi32(pairs, 31);
        low_sum = _mm_add_epi64(low_sum, _mm_unpacklo_epi32(pairs, signs));
        let high_pairs = _mm_srli_si128(pairs, 8);
        let high_signs = _mm_srai_epi32(high_pairs, 31);
        high_sum = _mm_add_epi64(high_sum, _mm_unpacklo_epi32(high_pairs, high_signs));
        offset += 8;
    }
    let mut lanes = [0i64; 2];
    unsafe { _mm_storeu_si128(lanes.as_mut_ptr().cast(), low_sum) };
    let mut high_lanes = [0i64; 2];
    unsafe { _mm_storeu_si128(high_lanes.as_mut_ptr().cast(), high_sum) };
    let mut sum = lanes[0] + lanes[1] + high_lanes[0] + high_lanes[1];
    for index in offset..template.len() {
        sum += i64::from(screen[index]) * i64::from(template[index]);
    }
    sum
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn dot_product_avx2(screen: &[u8], template: &[i16]) -> i64 {
    use std::arch::x86_64::*;
    let mut low_sum = _mm256_setzero_si256();
    let mut high_sum = _mm256_setzero_si256();
    let mut offset = 0;
    while offset + 16 <= template.len() {
        let pixel_bytes = unsafe { _mm_loadu_si128(screen.as_ptr().add(offset).cast()) };
        let pixels = _mm256_cvtepu8_epi16(pixel_bytes);
        let centered = unsafe { _mm256_loadu_si256(template.as_ptr().add(offset).cast()) };
        let pairs = _mm256_madd_epi16(pixels, centered);
        let low_pairs = _mm256_castsi256_si128(pairs);
        let high_pairs = _mm256_extracti128_si256(pairs, 1);
        low_sum = _mm256_add_epi64(low_sum, _mm256_cvtepi32_epi64(low_pairs));
        high_sum = _mm256_add_epi64(high_sum, _mm256_cvtepi32_epi64(high_pairs));
        offset += 16;
    }
    let mut lanes = [0i64; 4];
    unsafe { _mm256_storeu_si256(lanes.as_mut_ptr().cast(), low_sum) };
    let mut high_lanes = [0i64; 4];
    unsafe { _mm256_storeu_si256(high_lanes.as_mut_ptr().cast(), high_sum) };
    let mut sum = lanes.iter().chain(&high_lanes).sum::<i64>();
    for index in offset..template.len() {
        sum += i64::from(screen[index]) * i64::from(template[index]);
    }
    sum
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
        PreparedTemplate, bgra_to_gray, default_click_delay_ms, dot_product_kernel, find_template,
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

    #[test]
    fn selected_simd_kernel_matches_scalar_dot_product() {
        let pixels: Vec<u8> = (0..67).map(|index| (index * 37 % 256) as u8).collect();
        let centered: Vec<i16> = (0..67)
            .map(|index| (index * 53 % 511) as i16 - 255)
            .collect();
        let expected = pixels
            .iter()
            .zip(&centered)
            .map(|(&pixel, &value)| i64::from(pixel) * i64::from(value))
            .sum::<i64>();
        let kernel = dot_product_kernel();
        assert_eq!(unsafe { kernel(&pixels, &centered) }, expected);
        #[cfg(target_arch = "x86_64")]
        unsafe {
            assert_eq!(super::dot_product_sse2(&pixels, &centered), expected);
            if std::arch::is_x86_feature_detected!("avx2") {
                assert_eq!(super::dot_product_avx2(&pixels, &centered), expected);
            }
        }
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
