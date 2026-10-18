use super::{
    gdi::OwnedGdiObject,
    state::{ActivePanel, AppState},
};
use std::sync::atomic::Ordering;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT, WPARAM},
        Graphics::Gdi::{
            CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH, DEFAULT_QUALITY,
            FW_NORMAL, OUT_DEFAULT_PRECIS,
        },
        UI::WindowsAndMessaging::{
            GetClientRect, MB_ICONERROR, MoveWindow, SendMessageW, WM_SETFONT,
        },
    },
    core::w,
};

pub(crate) fn scale(value: i32, factor: f32) -> i32 {
    (value as f32 * factor).round().max(1.0) as i32
}

pub(crate) fn update_ui_font(state: &mut AppState, dpi: u32) {
    let font = OwnedGdiObject::new(unsafe {
        CreateFontW(
            -((10 * dpi as i32 + 36) / 72),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        )
    });
    if font.is_null() {
        state.show_message("Could not create the application font.", MB_ICONERROR);
        return;
    }
    let heading_font = OwnedGdiObject::new(unsafe {
        CreateFontW(
            -((18 * dpi as i32 + 36) / 72),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        )
    });
    if heading_font.is_null() {
        state.show_message("Could not create the heading font.", MB_ICONERROR);
        return;
    }

    unsafe {
        for control in std::iter::once(state.main_heading)
            .chain(std::iter::once(state.version_label))
            .chain(std::iter::once(state.credit_label))
            .chain(std::iter::once(state.settings_button))
            .chain(std::iter::once(state.logic_button))
            .chain(std::iter::once(state.clicker_button))
            .chain(state.menu_controls.iter().copied())
            .chain(state.clicker_controls.iter().copied())
            .chain(state.settings_controls.iter().copied())
            .chain(state.logic_controls.iter().copied())
        {
            SendMessageW(
                control,
                WM_SETFONT,
                Some(WPARAM(font.get().0 as usize)),
                Some(LPARAM(1)),
            );
        }
        SendMessageW(
            state.main_heading,
            WM_SETFONT,
            Some(WPARAM(heading_font.get().0 as usize)),
            Some(LPARAM(1)),
        );
    }
    state.font = font;
    state.heading_font = heading_font;
}

pub(crate) fn move_control(control: HWND, x: i32, y: i32, width: i32, height: i32) {
    if !control.0.is_null() {
        unsafe {
            let _ = MoveWindow(control, x, y, width.max(1), height.max(1), true);
        }
    }
}

/// Positions header, menu, clicker, settings, and logic controls for the current client size and DPI.
pub(crate) fn layout_controls(window: HWND, state: &AppState) {
    unsafe {
        let mut rect = RECT::default();
        if GetClientRect(window, &mut rect).is_err() {
            return;
        }
        let width = (rect.right - rect.left).max(1);
        let height = (rect.bottom - rect.top).max(1);
        let s = state.dpi_scale;
        let margin = scale(14, s);
        let content_width = (width - margin * 2).max(1);

        // Top Navigation buttons coordinates (stay at top right)
        let btn_h = scale(26, s);
        let btn_y = scale(8, s);
        let x_buttons_left = match state.active_panel {
            ActivePanel::MainMenu => {
                let settings_btn_w = scale(68, s);
                let logic_btn_w = scale(54, s);
                let clicker_btn_w = scale(62, s);
                let btn_gap = scale(5, s);

                let x_settings = width - margin - settings_btn_w;
                let x_logic = x_settings - btn_gap - logic_btn_w;
                let x_clicker = x_logic - btn_gap - clicker_btn_w;

                move_control(
                    state.settings_button,
                    x_settings,
                    btn_y,
                    settings_btn_w,
                    btn_h,
                );
                move_control(state.logic_button, x_logic, btn_y, logic_btn_w, btn_h);
                move_control(state.clicker_button, x_clicker, btn_y, clicker_btn_w, btn_h);
                x_clicker
            }
            ActivePanel::Clicker => {
                let back_w = scale(76, s);
                let x_back = width - margin - back_w;
                move_control(state.clicker_button, x_back, btn_y, back_w, btn_h);
                move_control(state.logic_button, -1000, -1000, 1, 1);
                move_control(state.settings_button, -1000, -1000, 1, 1);
                x_back
            }
            ActivePanel::Logic => {
                let back_w = scale(76, s);
                let x_back = width - margin - back_w;
                move_control(state.logic_button, x_back, btn_y, back_w, btn_h);
                move_control(state.clicker_button, -1000, -1000, 1, 1);
                move_control(state.settings_button, -1000, -1000, 1, 1);
                x_back
            }
            ActivePanel::Settings => {
                let back_w = scale(76, s);
                let x_back = width - margin - back_w;
                move_control(state.settings_button, x_back, btn_y, back_w, btn_h);
                move_control(state.clicker_button, -1000, -1000, 1, 1);
                move_control(state.logic_button, -1000, -1000, 1, 1);
                x_back
            }
        };

        // Header - Row 1: Brand Icon & App Title (width strictly constrained to not overlap top-right buttons)
        let icon_size = scale(26, s);
        move_control(
            state.main_brand_icon,
            margin,
            scale(8, s),
            icon_size,
            icon_size,
        );

        let title_x = margin + icon_size + scale(8, s);
        let title_w = (x_buttons_left - title_x - scale(6, s)).max(scale(40, s));
        move_control(
            state.main_heading,
            title_x,
            scale(7, s),
            title_w,
            scale(28, s),
        );

        // Header - Row 2: Version number and Credits (placed cleanly below row 1, completely clear of top buttons)
        let sub_y = scale(35, s);
        let version_w = scale(44, s);
        move_control(state.version_label, title_x, sub_y, version_w, scale(16, s));
        let credit_x = title_x + version_w + scale(6, s);
        let credit_w = (width - margin - credit_x).max(scale(60, s));
        move_control(state.credit_label, credit_x, sub_y, credit_w, scale(16, s));

        layout_main_menu_controls(width, height, margin, content_width, s, state);
        layout_clicker_controls(width, height, margin, content_width, s, state);
        layout_settings_controls(width, height, margin, content_width, s, state);
        super::logic_gui::layout_logic_controls(width, height, margin, s, state);
    }
}

fn layout_main_menu_controls(
    _width: i32,
    _height: i32,
    margin: i32,
    content_width: i32,
    s: f32,
    state: &AppState,
) {
    let card_top = scale(60, s);
    move_control(
        state.menu_status_title,
        margin,
        card_top,
        content_width,
        scale(20, s),
    );
    move_control(
        state.menu_summary_label,
        margin,
        card_top + scale(24, s),
        content_width,
        scale(22, s),
    );
    move_control(
        state.menu_hotkey_label,
        margin,
        card_top + scale(50, s),
        content_width,
        scale(22, s),
    );
    move_control(
        state.menu_tips_label,
        margin,
        card_top + scale(76, s),
        content_width,
        scale(22, s),
    );
}

fn layout_clicker_controls(
    _width: i32,
    height: i32,
    margin: i32,
    content_width: i32,
    s: f32,
    state: &AppState,
) {
    let running = state
        .worker
        .as_ref()
        .is_some_and(|worker| worker.running.load(Ordering::Acquire));

    // Bottom action controls:
    // 1. Status label at the very bottom
    let status_h = scale(20, s);
    let status_y = height - scale(24, s);

    // 2. Three action buttons (Start, Stop, Close) side-by-side above status
    let action_h = scale(30, s);
    let action_y = status_y - action_h - scale(4, s);
    let action_gap = scale(5, s);
    let button_width = (content_width - action_gap * 2) / 3;
    let button_width_last = content_width - (button_width + action_gap) * 2;

    move_control(state.start_button, margin, action_y, button_width, action_h);
    move_control(
        state.stop_button,
        margin + button_width + action_gap,
        action_y,
        button_width,
        action_h,
    );
    move_control(
        state.close_button,
        margin + (button_width + action_gap) * 2,
        action_y,
        button_width_last,
        action_h,
    );

    // 3. "Always on top" checkbox directly ON TOP of the three buttons
    let checkbox_h = scale(22, s);
    let checkbox_y = action_y - checkbox_h - scale(4, s);
    move_control(
        state.always_on_top_checkbox,
        margin,
        checkbox_y,
        content_width,
        checkbox_h,
    );

    // Status label
    move_control(
        state.status_label,
        margin,
        status_y,
        content_width,
        status_h,
    );

    // Image selector controls directly above the checkbox
    let selector_h = scale(28, s);
    let selector_y = checkbox_y - selector_h - scale(6, s);
    let image_name_h = scale(20, s);
    let image_name_y = if running {
        scale(100, s)
    } else {
        selector_y - image_name_h - scale(4, s)
    };

    let select_width = (content_width * 3 / 5).min(scale(180, s));
    move_control(
        state.select_image_button,
        margin,
        selector_y,
        select_width,
        selector_h,
    );
    move_control(
        state.clear_image_button,
        margin + select_width + scale(6, s),
        selector_y,
        (content_width - select_width - scale(6, s)).max(scale(70, s)),
        selector_h,
    );
    move_control(
        state.image_name,
        margin,
        image_name_y,
        content_width,
        image_name_h,
    );

    // Target slot header controls
    move_control(
        state.slot_label,
        margin,
        scale(56, s),
        scale(130, s),
        scale(18, s),
    );
    move_control(
        state.slot_combo,
        margin,
        scale(76, s),
        content_width,
        scale(220, s),
    );
    move_control(
        state.preview_label,
        margin,
        scale(104, s),
        content_width,
        scale(18, s),
    );

    // Preview area - dynamic height that smoothly adapts to compact and tall windows
    let preview_top = scale(126, s);
    let preview_bottom_limit = if running {
        selector_y - scale(6, s)
    } else {
        image_name_y - scale(6, s)
    };
    let preview_height = (preview_bottom_limit - preview_top).max(scale(50, s));

    move_control(
        state.preview,
        margin,
        preview_top,
        content_width,
        preview_height,
    );
    move_control(
        state.empty_preview,
        margin,
        preview_top,
        content_width,
        preview_height,
    );
}

fn layout_settings_controls(
    _width: i32,
    _height: i32,
    margin: i32,
    content_width: i32,
    s: f32,
    state: &AppState,
) {
    move_control(
        state.settings_title,
        margin,
        scale(55, s),
        scale(260, s),
        scale(24, s),
    );
    move_control(
        state.click_delay_input,
        margin,
        scale(80, s),
        scale(200, s),
        scale(26, s),
    );
    if let Some(label) = state.settings_controls.get(2) {
        move_control(*label, margin, scale(116, s), scale(260, s), scale(24, s));
    }
    move_control(
        state.thread_count_combo,
        margin,
        scale(140, s),
        scale(200, s),
        scale(110, s),
    );
    if let Some(label) = state.settings_controls.get(4) {
        move_control(*label, margin, scale(178, s), scale(260, s), scale(24, s));
    }
    move_control(
        state.theme_combo,
        margin,
        scale(202, s),
        scale(200, s),
        scale(110, s),
    );
    if let Some(label) = state.settings_controls.get(6) {
        move_control(*label, margin, scale(240, s), scale(260, s), scale(24, s));
    }
    move_control(
        state.slot_count_combo,
        margin,
        scale(264, s),
        scale(200, s),
        scale(120, s),
    );
    if let Some(note) = state.settings_controls.get(8) {
        move_control(*note, margin, scale(298, s), content_width, scale(24, s));
    }
}
