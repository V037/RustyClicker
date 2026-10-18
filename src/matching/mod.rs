pub(crate) mod grayscale;
pub(crate) mod pool;
pub(crate) mod template;

#[cfg(test)]
pub(crate) use grayscale::rgb_to_gray;
pub(crate) use grayscale::{bgra_to_gray, rgb_pixel_to_gray};
#[cfg(test)]
pub(crate) use pool::DEFAULT_CLICK_DELAY_MS;
pub(crate) use pool::{
    DEFAULT_MATCH_THREADS, MATCH_CONFIDENCE, MATCH_THREAD_OPTIONS, MAX_CLICK_DELAY_MS,
    MIN_CLICK_DELAY_MS, SCAN_INTERVAL, default_click_delay_ms, normalize_click_delay_ms,
    normalize_match_threads,
};
pub(crate) use template::PreparedTemplate;
