use crate::core::CoreError;
use crate::gui::{
    DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, Preferences,
    Theme, scale,
};
use crate::input::ClickButton;
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
    assert_eq!(preferences.image_count, 3);
    assert!(preferences.always_on_top);
    assert_eq!(preferences.start_hotkey, 0x70);
    assert_eq!(preferences.stop_hotkey, 0x71);
    assert_eq!(preferences.close_hotkey, 0x72);
    assert!(!preferences.hide_when_active);
    let defaults = Preferences::default();
    assert_eq!(defaults.click_delay_ms, DEFAULT_CLICK_DELAY_MS);
    assert_eq!(defaults.match_threads, DEFAULT_MATCH_THREADS);
    assert_eq!(defaults.image_count, 3);
    assert_eq!(defaults.classical_delay_ms, 100);
    assert_eq!(defaults.classical_button, ClickButton::Left);
    assert_eq!(defaults.start_hotkey, 0x70);
    assert_eq!(defaults.stop_hotkey, 0x71);
    assert_eq!(defaults.close_hotkey, 0x72);
    assert!(!defaults.hide_when_active);
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
        classical_delay_ms: 25,
        classical_button: ClickButton::Right,
        classical_hours: 1,
        classical_mins: 2,
        classical_secs: 3,
        classical_millis: 456,
        classical_random_offset_enabled: true,
        classical_random_offset_ms: 25,
        classical_click_type: crate::input::ClickType::Double,
        classical_repeat_mode: crate::gui::preferences::ClassicalRepeatMode::Count,
        classical_repeat_count: 42,
        classical_location_mode: crate::gui::preferences::ClassicalLocationMode::Pick,
        classical_pick_x: 100,
        classical_pick_y: 200,
        start_hotkey: 0x73,
        stop_hotkey: 0x74,
        close_hotkey: 0x75,
        hide_when_active: true,
    };
    let encoded = serde_json::to_string(&preferences).expect("preferences serialize");
    let decoded: Preferences = serde_json::from_str(&encoded).expect("preferences deserialize");
    assert_eq!(decoded.click_delay_ms, 500);
    assert_eq!(decoded.match_threads, 8);
    assert_eq!(decoded.theme, Theme::Purple);
    assert_eq!(decoded.image_count, 37);
    assert_eq!(decoded.classical_delay_ms, 25);
    assert_eq!(decoded.classical_button, ClickButton::Right);
    assert_eq!(decoded.classical_hours, 1);
    assert_eq!(decoded.classical_mins, 2);
    assert_eq!(decoded.classical_secs, 3);
    assert_eq!(decoded.classical_millis, 456);
    assert!(decoded.classical_random_offset_enabled);
    assert_eq!(decoded.classical_random_offset_ms, 25);
    assert_eq!(
        decoded.classical_click_type,
        crate::input::ClickType::Double
    );
    assert_eq!(
        decoded.classical_repeat_mode,
        crate::gui::preferences::ClassicalRepeatMode::Count
    );
    assert_eq!(decoded.classical_repeat_count, 42);
    assert_eq!(
        decoded.classical_location_mode,
        crate::gui::preferences::ClassicalLocationMode::Pick
    );
    assert_eq!(decoded.classical_pick_x, 100);
    assert_eq!(decoded.classical_pick_y, 200);
    assert_eq!(decoded.start_hotkey, 0x73);
    assert_eq!(decoded.stop_hotkey, 0x74);
    assert_eq!(decoded.close_hotkey, 0x75);
    assert!(decoded.hide_when_active);
    assert_eq!(decoded.image_paths, preferences.image_paths);
}

#[test]
fn vk_to_string_formats_keys() {
    use crate::gui::preferences::vk_to_string;
    assert_eq!(vk_to_string(0x70), "F1");
    assert_eq!(vk_to_string(0x71), "F2");
    assert_eq!(vk_to_string(0x72), "F3");
    assert_eq!(vk_to_string(0x1B), "Esc");
    assert_eq!(vk_to_string(0x20), "Space");
}

#[test]
fn custom_hotkeys_format_labels_and_tips() {
    use crate::gui::preferences::vk_to_string;
    let start_str = vk_to_string(0x73);
    let stop_str = vk_to_string(0x74);
    let close_str = vk_to_string(0x75);
    let menu_tip = format!("Tip: Open Classical or Images, then press {start_str} to start.");
    let ready_text = format!("Ready  |  Press {start_str} to start, {stop_str} to stop.");
    let stopped_text = format!("Stopped. Press {start_str} to start.");
    let global_hotkeys = format!(
        "Global Hotkeys:  [{start_str}] Start   •   [{stop_str}] Stop   •   [{close_str}] Close"
    );

    assert_eq!(
        menu_tip,
        "Tip: Open Classical or Images, then press F4 to start."
    );
    assert_eq!(ready_text, "Ready  |  Press F4 to start, F5 to stop.");
    assert_eq!(stopped_text, "Stopped. Press F4 to start.");
    assert_eq!(
        global_hotkeys,
        "Global Hotkeys:  [F4] Start   •   [F5] Stop   •   [F6] Close"
    );
}

#[test]
fn classical_click_button_names() {
    assert_eq!(ClickButton::Left.name(), "Left Click");
    assert_eq!(ClickButton::Right.name(), "Right Click");
    assert_eq!(ClickButton::Middle.name(), "Middle Click");
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
    assert_eq!(DEFAULT_WINDOW_WIDTH, 360);
    assert_eq!(DEFAULT_WINDOW_HEIGHT, 440);
    assert_eq!(MIN_WINDOW_WIDTH, 330);
    assert_eq!(MIN_WINDOW_HEIGHT, 380);
    assert_eq!(scale(DEFAULT_WINDOW_WIDTH, 1.0), 360);
    assert_eq!(scale(DEFAULT_WINDOW_HEIGHT, 1.0), 440);
    assert_eq!(scale(MIN_WINDOW_WIDTH, 1.0), 330);
    assert_eq!(scale(MIN_WINDOW_HEIGHT, 1.0), 380);
}

#[test]
fn classical_click_type_names() {
    assert_eq!(crate::input::ClickType::Single.name(), "Single");
    assert_eq!(crate::input::ClickType::Double.name(), "Double");
    assert_eq!(
        crate::input::ClickType::default(),
        crate::input::ClickType::Single
    );
}

#[test]
fn classical_fast_rng_generates_within_bounds() {
    let mut rng = crate::classical_worker::FastRng::new();
    for _ in 0..100 {
        let val = rng.gen_range(-40, 40);
        assert!((-40..=40).contains(&val));
    }
    assert_eq!(rng.gen_range(10, 10), 10);
    assert_eq!(rng.gen_range(20, 10), 20);
}

#[test]
fn classical_worker_config_initialization() {
    let config = crate::classical_worker::ClassicalWorkerConfig {
        delay_ms: 100,
        random_offset_ms: 20,
        button: ClickButton::Left,
        click_type: crate::input::ClickType::Single,
        repeat_limit: 5,
        target_pos: Some((150, 250)),
    };
    let state = crate::classical_worker::ClassicalWorkerState::new(config);
    assert!(state.is_running());
    assert_eq!(state.clicks(), 0);
    assert_eq!(state.cps(), 0);
}

#[test]
fn test_live_screen_capture() {
    let mut cap = None;
    let mut gray = Vec::new();
    let res = crate::capture::capture_screen(&mut cap, &mut gray);
    println!("capture_screen result: {:?}", res);
    assert!(res.is_ok());
}

#[test]
fn test_windows_ocr_engine_available() {
    let engine = windows::Media::Ocr::OcrEngine::TryCreateFromUserProfileLanguages().unwrap();
    let width = 200i32;
    let height = 50i32;
    // 200x50 BGRA image
    let pixel_bytes = vec![255u8; (width * height * 4) as usize];
    let writer = windows::Storage::Streams::DataWriter::new().unwrap();
    writer.WriteBytes(&pixel_bytes).unwrap();
    let buffer = writer.DetachBuffer().unwrap();
    let bitmap = windows::Graphics::Imaging::SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        windows::Graphics::Imaging::BitmapPixelFormat::Bgra8,
        width,
        height,
    ).unwrap();
    let op = engine.RecognizeAsync(&bitmap).unwrap();
    while op.Status().unwrap().0 == 0 {
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let ocr_res = op.GetResults().unwrap();
    let text = ocr_res.Text().unwrap().to_string();
    println!("OCR Result on blank: '{}'", text);
    assert_eq!(text, "");
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
