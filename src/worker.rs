use crate::capture::capture_screen;
use crate::core::CoreError;
use crate::input::send_left_click;
use crate::matching::{MATCH_CONFIDENCE, PreparedTemplate, SCAN_INTERVAL};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::POINT,
    UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos},
};

pub(crate) struct WorkerState {
    pub(crate) stop: AtomicBool,
    pub(crate) running: AtomicBool,
    pub(crate) clicks: AtomicU64,
    pub(crate) click_delay_ms: AtomicU64,
    pub(crate) match_threads: AtomicUsize,
    pub(crate) scan_duration_us: AtomicU64,
    pub(crate) error: Mutex<Option<CoreError>>,
}

/// Captures, matches, and clicks in a loop using a persistent CPU thread pool.
pub(crate) fn run_clicker(templates: Vec<PreparedTemplate>, worker: Arc<WorkerState>) {
    let mut click_count = 0u64;
    let mut error = None;
    let mut next_template_index = 0;
    let mut capture = None;
    let mut screen_gray = Vec::new();
    while !worker.stop.load(Ordering::Acquire) {
        let scan_started = Instant::now();
        let (screen_width, screen_height, origin_x, origin_y) =
            match capture_screen(&mut capture, &mut screen_gray) {
                Ok(screen) => screen,
                Err(capture_error) => {
                    error = Some(CoreError::ScreenCapture(Box::new(capture_error)));
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
            worker.match_threads.load(Ordering::Acquire),
        ) {
            Ok(found) => found,
            Err(match_error) => {
                error = Some(match_error);
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
                match GetCursorPos(&mut cursor) {
                    Ok(()) => {
                        if (cursor.x == origin_x || cursor.x == origin_x + screen_width as i32 - 1)
                            && (cursor.y == origin_y
                                || cursor.y == origin_y + screen_height as i32 - 1)
                        {
                            error = Some(CoreError::FailSafeTriggered);
                            break;
                        }
                    }
                    Err(_) => {
                        error = Some(CoreError::CursorPosition);
                        break;
                    }
                }
                if SetCursorPos(center_x, center_y).is_err() {
                    error = Some(CoreError::CursorMove);
                    break;
                }
                if let Err(input_error) = send_left_click() {
                    error = Some(input_error);
                    break;
                }
            }
            click_count += 1;
            worker.clicks.store(click_count, Ordering::Relaxed);
            next_template_index = (next_template_index + 1) % templates.len();
            clicked = true;
        }

        if error.is_some() {
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

    if let Some(error) = error {
        *worker
            .error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error);
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
