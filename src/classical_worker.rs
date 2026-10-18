use crate::input::{ClickButton, send_mouse_click};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};

pub(crate) struct ClassicalWorkerState {
    pub(crate) stop: AtomicBool,
    pub(crate) running: AtomicBool,
    pub(crate) clicks: AtomicU64,
    pub(crate) cps: AtomicU64,
    pub(crate) delay_ms: AtomicU64,
    pub(crate) button: AtomicU8,
}

impl ClassicalWorkerState {
    pub(crate) fn new(delay_ms: u64, button: ClickButton) -> Self {
        Self {
            stop: AtomicBool::new(false),
            running: AtomicBool::new(true),
            clicks: AtomicU64::new(0),
            cps: AtomicU64::new(0),
            delay_ms: AtomicU64::new(delay_ms),
            button: AtomicU8::new(button as u8),
        }
    }

    pub(crate) fn get_button(&self) -> ClickButton {
        match self.button.load(Ordering::Relaxed) {
            1 => ClickButton::Right,
            2 => ClickButton::Middle,
            _ => ClickButton::Left,
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub(crate) fn clicks(&self) -> u64 {
        self.clicks.load(Ordering::Relaxed)
    }

    pub(crate) fn cps(&self) -> u64 {
        self.cps.load(Ordering::Relaxed)
    }
}

/// Ultra-fast autoclicker loop operating at maximum possible speed and low latency.
pub(crate) fn run_classical_clicker(worker: Arc<ClassicalWorkerState>) {
    unsafe {
        let _ = timeBeginPeriod(1);
    }

    let mut total_clicks = 0u64;
    let mut cps_window_start = Instant::now();
    let mut cps_window_clicks = 0u64;

    while !worker.stop.load(Ordering::Acquire) {
        let button = worker.get_button();
        if send_mouse_click(button).is_err() {
            break;
        }

        total_clicks += 1;
        cps_window_clicks += 1;
        worker.clicks.store(total_clicks, Ordering::Relaxed);

        // Update CPS periodically
        let elapsed = cps_window_start.elapsed();
        if elapsed >= Duration::from_millis(500) {
            let elapsed_secs = elapsed.as_secs_f64();
            if elapsed_secs > 0.0 {
                let current_cps = (cps_window_clicks as f64 / elapsed_secs).round() as u64;
                worker.cps.store(current_cps, Ordering::Relaxed);
            }
            cps_window_start = Instant::now();
            cps_window_clicks = 0;
        }

        let delay = worker.delay_ms.load(Ordering::Relaxed);
        if delay == 0 {
            // Uncapped maximum speed: yield occasionally to avoid starving thread scheduler
            if total_clicks % 32 == 0 {
                thread::yield_now();
            }
        } else {
            // High-precision delay
            let target = Instant::now() + Duration::from_millis(delay);
            while Instant::now() < target && !worker.stop.load(Ordering::Acquire) {
                let remaining = target.saturating_duration_since(Instant::now());
                if remaining >= Duration::from_millis(2) {
                    thread::sleep(Duration::from_millis(1));
                } else {
                    thread::yield_now();
                }
            }
        }
    }

    worker.running.store(false, Ordering::Release);
    worker.cps.store(0, Ordering::Relaxed);

    unsafe {
        let _ = timeEndPeriod(1);
    }
}
