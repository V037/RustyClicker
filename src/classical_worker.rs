use crate::input::{ClickButton, ClickType, send_mouse_click_type};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ClassicalWorkerConfig {
    pub(crate) delay_ms: u64,
    pub(crate) random_offset_ms: u64,
    pub(crate) button: ClickButton,
    pub(crate) click_type: ClickType,
    pub(crate) repeat_limit: u64,
    pub(crate) target_pos: Option<(i32, i32)>,
}

pub(crate) struct ClassicalWorkerState {
    pub(crate) stop: AtomicBool,
    pub(crate) running: AtomicBool,
    pub(crate) clicks: AtomicU64,
    pub(crate) cps: AtomicU64,
    pub(crate) config: ClassicalWorkerConfig,
}

impl ClassicalWorkerState {
    pub(crate) fn new(config: ClassicalWorkerConfig) -> Self {
        Self {
            stop: AtomicBool::new(false),
            running: AtomicBool::new(true),
            clicks: AtomicU64::new(0),
            cps: AtomicU64::new(0),
            config,
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

#[derive(Debug)]
pub(crate) struct FastRng(pub(crate) u64);

impl FastRng {
    pub(crate) fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x853c49e6748fea9b);
        Self(seed ^ 0xda942042e4dd58b5)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    pub(crate) fn gen_range(&mut self, min: i64, max: i64) -> i64 {
        if min >= max {
            return min;
        }
        let span = (max - min + 1) as u64;
        min + (self.next_u64() % span) as i64
    }
}

/// Ultra-fast autoclicker loop operating at maximum possible speed and low latency.
pub(crate) fn run_classical_clicker(worker: Arc<ClassicalWorkerState>) {
    unsafe {
        let _ = timeBeginPeriod(1);
    }

    let config = worker.config;
    let mut total_clicks = 0u64;
    let mut cps_window_start = Instant::now();
    let mut cps_window_clicks = 0u64;
    let mut rng = FastRng::new();

    while !worker.stop.load(Ordering::Acquire) {
        if config.repeat_limit > 0 && total_clicks >= config.repeat_limit {
            break;
        }

        if send_mouse_click_type(config.button, config.click_type, config.target_pos).is_err() {
            break;
        }

        total_clicks += 1;
        cps_window_clicks += 1;
        worker.clicks.store(total_clicks, Ordering::Relaxed);

        if config.repeat_limit > 0 && total_clicks >= config.repeat_limit {
            break;
        }

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

        let base_delay = config.delay_ms;
        let delay = if config.random_offset_ms > 0 {
            let offset = rng.gen_range(
                -(config.random_offset_ms as i64),
                config.random_offset_ms as i64,
            );
            (base_delay as i64 + offset).max(0) as u64
        } else {
            base_delay
        };

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
