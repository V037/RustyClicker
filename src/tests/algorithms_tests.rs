use crate::core::CoreError;
use crate::gui::{Preferences, Theme, scale};
use crate::matching::{
    DEFAULT_CLICK_DELAY_MS, DEFAULT_MATCH_THREADS, MATCH_THREAD_OPTIONS, MAX_CLICK_DELAY_MS,
    MIN_CLICK_DELAY_MS, PreparedTemplate, bgra_to_gray, default_click_delay_ms,
    normalize_click_delay_ms, normalize_match_threads, rgb_to_gray,
};
use image::{Rgb, RgbImage, Rgba, RgbaImage};
use std::sync::atomic::{AtomicBool, Ordering};

/// Finds the first row-major match using normalized cross-correlation.
fn find_template(
    screenshot: &RgbImage,
    template: &PreparedTemplate,
    confidence: f32,
    stop: Option<&AtomicBool>,
) -> Result<Option<(u32, u32, u32, u32)>, CoreError> {
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
    template.find_match(
        &screen_gray,
        screen_width,
        screen_height,
        confidence,
        stop,
        DEFAULT_MATCH_THREADS,
    )
}

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
fn reports_invalid_rgb_buffer_as_a_typed_error() {
    let error = rgb_to_gray(&[1, 2], 1, 1).expect_err("RGB buffer has the wrong length");
    assert_eq!(
        error,
        CoreError::InvalidPixelBuffer {
            format: "RGB",
            expected: 3,
            actual: 2,
        }
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
    let found = find_template(&exact_screen, &template, 0.90, None).expect("matching completes");
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
    let template = PreparedTemplate::new(RgbaImage::from_pixel(16, 16, Rgba([200, 100, 20, 255])))
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
    let template = PreparedTemplate::new(RgbaImage::from_pixel(4, 4, Rgba([242, 128, 15, 255])))
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
    let template = PreparedTemplate::new(RgbaImage::from_pixel(4, 4, Rgba([255, 128, 32, 255])))
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
    assert_eq!(preferences.match_threads, DEFAULT_MATCH_THREADS);
    assert_eq!(preferences.theme, Theme::Dark);
    assert_eq!(preferences.image_count, 10);
    assert!(preferences.always_on_top);
    let defaults = Preferences::default();
    assert_eq!(defaults.click_delay_ms, DEFAULT_CLICK_DELAY_MS);
    assert_eq!(defaults.match_threads, DEFAULT_MATCH_THREADS);
}

#[test]
fn selected_settings_round_trip_through_preferences() {
    let preferences = Preferences {
        image_paths: vec![Some("target.png".to_owned())],
        always_on_top: true,
        click_delay_ms: 500,
        match_threads: 8,
        theme: Theme::Purple,
        image_count: 37,
    };
    let encoded = serde_json::to_string(&preferences).expect("preferences serialize");
    let decoded: Preferences = serde_json::from_str(&encoded).expect("preferences deserialize");
    assert_eq!(decoded.click_delay_ms, 500);
    assert_eq!(decoded.match_threads, 8);
    assert_eq!(decoded.theme, Theme::Purple);
    assert_eq!(decoded.image_count, 37);
    assert_eq!(decoded.image_paths, preferences.image_paths);
}

#[test]
fn invalid_saved_delay_uses_the_default() {
    assert_eq!(default_click_delay_ms(), 100);
    assert_eq!(normalize_click_delay_ms(0), DEFAULT_CLICK_DELAY_MS);
    assert_eq!(
        normalize_click_delay_ms(MIN_CLICK_DELAY_MS),
        MIN_CLICK_DELAY_MS
    );
    assert_eq!(
        normalize_click_delay_ms(MAX_CLICK_DELAY_MS),
        MAX_CLICK_DELAY_MS
    );
    assert_eq!(
        normalize_click_delay_ms(MAX_CLICK_DELAY_MS + 1),
        DEFAULT_CLICK_DELAY_MS
    );
}

#[test]
fn match_thread_options_and_saved_values_are_validated() {
    assert_eq!(MATCH_THREAD_OPTIONS, [1, 2, 4, 8]);
    assert_eq!(normalize_match_threads(DEFAULT_MATCH_THREADS), 2);
    assert_eq!(normalize_match_threads(8), 8);
    assert_eq!(normalize_match_threads(3), DEFAULT_MATCH_THREADS);
}

#[test]
fn ui_dimensions_scale_with_windows_dpi() {
    assert_eq!(scale(100, 1.0), 100);
    assert_eq!(scale(100, 1.5), 150);
}

#[test]
fn corrmatch_on_blank_screen() {
    let template_img = RgbaImage::from_fn(16, 16, |x, y| {
        Rgba([((x * 47 + y * 17) % 256) as u8, 120, 200, 255])
    });
    let template = PreparedTemplate::new(template_img).unwrap();
    let blank_gray = vec![0u8; 100 * 100];
    let res = template.find_match(&blank_gray, 100, 100, 0.90, None, 1);
    assert_eq!(res, Ok(None));
}
