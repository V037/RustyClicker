pub mod grayscale;
pub mod pool;
pub mod template;

pub use grayscale::{bgra_to_gray, rgb_pixel_to_gray, rgb_to_gray};
pub use pool::{
    DEFAULT_CLICK_DELAY_MS, DEFAULT_MATCH_THREADS, MATCH_CONFIDENCE, MATCH_THREAD_OPTIONS,
    MAX_CLICK_DELAY_MS, MIN_CLICK_DELAY_MS, SCAN_INTERVAL, default_click_delay_ms,
    normalize_click_delay_ms, normalize_match_threads,
};
pub use template::{MatchResult, PreparedTemplate};
