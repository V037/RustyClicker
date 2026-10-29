use crate::core::CoreError;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::sync::OnceLock;
use std::time::Duration;

pub const MATCH_CONFIDENCE: f32 = 0.90;
pub const SCAN_INTERVAL: Duration = Duration::from_millis(10);
pub const MATCH_THREAD_OPTIONS: [usize; 4] = [1, 2, 4, 8];
pub const DEFAULT_MATCH_THREADS: usize = 2;
pub const DEFAULT_CLICK_DELAY_MS: u64 = 100;
pub const MIN_CLICK_DELAY_MS: u64 = 1;
pub const MAX_CLICK_DELAY_MS: u64 = 10_000;

pub fn default_click_delay_ms() -> u64 {
    DEFAULT_CLICK_DELAY_MS
}

pub fn normalize_click_delay_ms(delay: u64) -> u64 {
    if (MIN_CLICK_DELAY_MS..=MAX_CLICK_DELAY_MS).contains(&delay) {
        delay
    } else {
        default_click_delay_ms()
    }
}

pub fn normalize_match_threads(threads: usize) -> usize {
    if MATCH_THREAD_OPTIONS.contains(&threads) {
        threads
    } else {
        DEFAULT_MATCH_THREADS
    }
}

static MATCH_POOLS: [OnceLock<Result<ThreadPool, CoreError>>; MATCH_THREAD_OPTIONS.len()] =
    [const { OnceLock::new() }; MATCH_THREAD_OPTIONS.len()];

pub(crate) fn match_pool(thread_count: usize) -> Result<&'static ThreadPool, CoreError> {
    let Some(pool_index) = MATCH_THREAD_OPTIONS
        .iter()
        .position(|&option| option == thread_count)
    else {
        return Err(CoreError::UnsupportedThreadCount(thread_count));
    };
    match MATCH_POOLS[pool_index].get_or_init(|| {
        ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .thread_name(|index| format!("rusty-matcher-{index}"))
            .build()
            .map_err(|error| CoreError::MatcherPool(error.to_string()))
    }) {
        Ok(pool) => Ok(pool),
        Err(error) => Err(error.clone()),
    }
}
