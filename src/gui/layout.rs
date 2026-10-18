use super::{gdi::OwnedGdiObject, state::AppState};
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
            .chain(std::iter::once(state.settings_button))
            .chain(state.main_controls.iter().copied())
            .chain(state.settings_controls.iter().copied())
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

/// Positions both main and settings controls for the current client size and DPI.
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
        move_control(
            state.main_brand_icon,
            margin,
            scale(10, s),
            scale(30, s),
            scale(30, s),
        );
        move_control(
            state.main_heading,
            margin + scale(38, s),
            scale(10, s),
            scale(190, s),
            scale(32, s),
        );
        move_control(
            state.version_label,
            margin + scale(232, s),
            scale(18, s),
            scale(48, s),
            scale(20, s),
        );
        move_control(
            state.credit_label,
            margin + scale(38, s),
            scale(44, s),
            scale(150, s),
            scale(16, s),
        );
        move_control(
            state.settings_button,
            width - margin - scale(92, s),
            scale(10, s),
            scale(92, s),
            scale(30, s),
        );
        let running = state
            .worker
            .as_ref()
            .is_some_and(|worker| worker.running.load(Ordering::Acquire));
        let preview_top = scale(138, s);
        let preview_height =
            (height - preview_top - scale(160, s)).clamp(scale(80, s), scale(132, s));
        let preview_bottom = preview_top + preview_height;
        move_control(
            state.slot_label,
            margin,
            scale(62, s),
            scale(130, s),
            scale(20, s),
        );
        move_control(
            state.slot_combo,
            margin,
            scale(84, s),
            content_width,
            scale(220, s),
        );
        move_control(
            state.preview_label,
            margin,
            scale(114, s),
            content_width,
            scale(18, s),
        );
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
        move_control(
            state.image_name,
            margin,
            if running {
                scale(104, s)
            } else {
                preview_bottom + scale(4, s)
            },
            content_width,
            scale(22, s),
        );
        let selector_y = if running {
            scale(132, s)
        } else {
            preview_bottom + scale(28, s)
        };
        let select_width = (content_width * 3 / 5).min(scale(190, s));
        move_control(
            state.select_image_button,
            margin,
            selector_y,
            select_width,
            scale(32, s),
        );
        move_control(
            state.clear_image_button,
            margin + select_width + scale(8, s),
            selector_y,
            (content_width - select_width - scale(8, s)).max(scale(88, s)),
            scale(32, s),
        );

        let action_y = height - scale(68, s);
        let status_y = height - scale(30, s);
        let action_gap = scale(5, s);
        let button_width = scale(74, s);
        let check_width = scale(116, s);
        move_control(
            state.start_button,
            margin,
            action_y,
            button_width,
            scale(32, s),
        );
        move_control(
            state.stop_button,
            margin + button_width + action_gap,
            action_y,
            button_width,
            scale(32, s),
        );
        move_control(
            state.close_button,
            margin + (button_width + action_gap) * 2,
            action_y,
            button_width,
            scale(32, s),
        );
        move_control(
            state.always_on_top_checkbox,
            (width - margin - check_width).max(margin),
            action_y,
            check_width,
            scale(32, s),
        );
        move_control(
            state.status_label,
            margin,
            status_y,
            content_width,
            scale(20, s),
        );
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
}
