use super::{
    ID_ALWAYS_ON_TOP, ID_BRAND_ICON, ID_CLEAR_IMAGE, ID_CLOSE, ID_CREDIT_LABEL, ID_DELAY_INPUT,
    ID_EMPTY_PREVIEW, ID_PREVIEW_LABEL, ID_SELECT_IMAGE, ID_SETTINGS, ID_SLOT_COMBO,
    ID_SLOT_COUNT_COMBO, ID_SLOT_LABEL, ID_START, ID_STATUS_LABEL, ID_STOP, ID_THEME_COMBO,
    ID_THREAD_COUNT_COMBO, TIMER_ID,
    controls::create_controls,
    drawing::{draw_button, draw_cursor_icon, draw_empty_preview},
    layout::{layout_controls, scale, update_ui_font},
    preferences::rgb,
    settings::{
        ID_SETTINGS_DELAY_LABEL, ID_SETTINGS_SLOT_COUNT_LABEL, ID_SETTINGS_THEME_LABEL,
        ID_SETTINGS_THREADS_LABEL,
    },
    state::{AppState, set_text},
};
use std::sync::atomic::Ordering;
use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{FillRect, HDC, ScreenToClient, SetBkColor, SetTextColor},
    UI::{
        Controls::{BST_CHECKED, DRAWITEMSTRUCT},
        Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, ReleaseCapture, UnregisterHotKey},
        WindowsAndMessaging::{
            BM_GETCHECK, CB_GETCURSEL, CBN_SELCHANGE, CallWindowProcW, DefWindowProcW,
            DestroyWindow, EN_KILLFOCUS, GWLP_USERDATA, GWLP_WNDPROC, GetClientRect, GetDlgCtrlID,
            GetParent, GetWindowLongPtrW, GetWindowTextW, HTCAPTION, MB_ICONERROR, MB_ICONWARNING,
            MINMAXINFO, PostMessageW, PostQuitMessage, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW,
            SetTimer, SetWindowLongPtrW, SetWindowPos, WM_CLOSE, WM_COMMAND, WM_CREATE,
            WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DESTROY,
            WM_DPICHANGED, WM_DRAWITEM, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_HOTKEY, WM_KEYDOWN,
            WM_LBUTTONDOWN, WM_NCCREATE, WM_NCHITTEST, WM_SIZE, WM_TIMER, WNDPROC,
        },
    },
};

pub(crate) fn make_static_draggable(control: HWND) -> bool {
    unsafe {
        let previous_proc = SetWindowLongPtrW(
            control,
            GWLP_WNDPROC,
            static_control_proc as *const () as isize,
        );
        if previous_proc == 0 {
            return false;
        }
        SetWindowLongPtrW(control, GWLP_USERDATA, previous_proc);
        true
    }
}

pub(crate) unsafe extern "system" fn static_control_proc(
    control: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_LBUTTONDOWN
            && let Ok(parent) = GetParent(control)
        {
            let _ = ReleaseCapture();
            SendMessageW(
                parent,
                windows::Win32::UI::WindowsAndMessaging::WM_NCLBUTTONDOWN,
                Some(WPARAM(HTCAPTION as usize)),
                Some(LPARAM(0)),
            );
            return LRESULT(0);
        }

        let previous_proc = GetWindowLongPtrW(control, GWLP_USERDATA);
        if previous_proc == 0 {
            return DefWindowProcW(control, message, wparam, lparam);
        }
        let previous_proc = std::mem::transmute::<isize, WNDPROC>(previous_proc);
        CallWindowProcW(previous_proc, control, message, wparam, lparam)
    }
}

/// Routes Win32 window messages to app actions, hotkeys, drawing, and worker status updates.
pub(crate) unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create_info =
                &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, create_info.lpCreateParams as isize);
        }
        let state_ptr = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut AppState;
        if message == WM_CREATE && !state_ptr.is_null() {
            create_controls(window, &mut *state_ptr);
            let _ = SetTimer(Some(window), TIMER_ID, 250, None);
            let mut unavailable_hotkeys = Vec::new();
            for (id, key, name) in [(1, 0x70, "F1"), (2, 0x71, "F2"), (3, 0x72, "F3")] {
                if RegisterHotKey(Some(window), id, MOD_NOREPEAT, key).is_err() {
                    unavailable_hotkeys.push(name);
                }
            }
            if !unavailable_hotkeys.is_empty() {
                (*state_ptr).show_message(
                    &format!(
                        "Could not register global {} hotkey(s). Other registered hotkeys remain active. Unavailable keys work only while RustyClicker is focused.",
                        unavailable_hotkeys.join(", ")
                    ),
                    MB_ICONWARNING,
                );
            } else {
                set_text(
                    (*state_ptr).status_label,
                    "Ready. Global F1, F2, and F3 hotkeys are active.",
                );
            }
            return LRESULT(0);
        }
        if state_ptr.is_null() {
            return DefWindowProcW(window, message, wparam, lparam);
        }
        let state = &mut *state_ptr;
        if message == WM_ERASEBKGND && !state.background_brush.is_null() {
            let mut client = RECT::default();
            if GetClientRect(window, &mut client).is_ok() {
                FillRect(
                    HDC(wparam.0 as *mut _),
                    &client,
                    state.background_brush.get(),
                );
            }
            return LRESULT(1);
        }
        if message == WM_CTLCOLORSTATIC && !state.background_brush.is_null() {
            let (background, default_text) = state.theme.colors();
            let control_id =
                usize::try_from(GetDlgCtrlID(HWND(lparam.0 as *mut _))).unwrap_or_default();
            let text = if control_id == ID_CREDIT_LABEL {
                let brightness = (background.0 & 0xff)
                    + ((background.0 >> 8) & 0xff)
                    + ((background.0 >> 16) & 0xff);
                let shade = if brightness > 3 * 127 { 190 } else { 112 };
                COLORREF(rgb(shade, shade, shade))
            } else if [
                ID_SLOT_LABEL,
                ID_PREVIEW_LABEL,
                ID_STATUS_LABEL,
                ID_SETTINGS_DELAY_LABEL,
                ID_SETTINGS_THREADS_LABEL,
                ID_SETTINGS_THEME_LABEL,
                ID_SETTINGS_SLOT_COUNT_LABEL,
            ]
            .contains(&control_id)
            {
                COLORREF(state.theme.accent())
            } else {
                default_text
            };
            SetBkColor(HDC(wparam.0 as *mut _), background);
            SetTextColor(HDC(wparam.0 as *mut _), text);
            return LRESULT(state.background_brush.get().0 as isize);
        }
        if message == WM_CTLCOLORBTN && !state.background_brush.is_null() {
            let (background, text) = state.theme.colors();
            SetBkColor(HDC(wparam.0 as *mut _), background);
            SetTextColor(HDC(wparam.0 as *mut _), text);
            return LRESULT(state.background_brush.get().0 as isize);
        }
        if (message == WM_CTLCOLOREDIT || message == WM_CTLCOLORLISTBOX)
            && !state.surface_brush.is_null()
        {
            let (_, text) = state.theme.colors();
            SetBkColor(
                HDC(wparam.0 as *mut _),
                COLORREF(state.theme.surface_color()),
            );
            SetTextColor(HDC(wparam.0 as *mut _), text);
            return LRESULT(state.surface_brush.get().0 as isize);
        }
        if message == WM_GETMINMAXINFO {
            let sizing = &mut *(lparam.0 as *mut MINMAXINFO);
            sizing.ptMinTrackSize.x = scale(410, state.dpi_scale);
            sizing.ptMinTrackSize.y = scale(390, state.dpi_scale);
            return LRESULT(0);
        }
        if message == WM_NCHITTEST {
            let mut point = POINT {
                x: lparam.0 as u16 as i16 as i32,
                y: (lparam.0 >> 16) as u16 as i16 as i32,
            };
            let mut client = RECT::default();
            if ScreenToClient(window, &mut point).as_bool()
                && GetClientRect(window, &mut client).is_ok()
                && (0..client.right).contains(&point.x)
                && (0..client.bottom).contains(&point.y)
            {
                return LRESULT(HTCAPTION as isize);
            }
        }
        match message {
            WM_SIZE => {
                layout_controls(window, state);
                if !state.settings_open
                    && state.preview_source.is_some()
                    && let Err(error) = state.update_preview_bitmap(window)
                {
                    state.show_message(&error, MB_ICONERROR);
                }
                LRESULT(0)
            }
            WM_DRAWITEM => {
                let item = &*(lparam.0 as *const DRAWITEMSTRUCT);
                if usize::try_from(item.CtlID).unwrap_or_default() == ID_BRAND_ICON {
                    draw_cursor_icon(state, item);
                } else if usize::try_from(item.CtlID).unwrap_or_default() == ID_EMPTY_PREVIEW {
                    draw_empty_preview(state, item);
                } else {
                    draw_button(state, item);
                }
                LRESULT(1)
            }
            WM_DPICHANGED => {
                let dpi = (wparam.0 & 0xffff) as u32;
                state.dpi_scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
                update_ui_font(state, dpi.max(96));
                let suggested = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    window,
                    None,
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                layout_controls(window, state);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let notification = (wparam.0 >> 16) & 0xffff;
                if id == ID_SLOT_COMBO && notification == CBN_SELCHANGE as usize {
                    let selected = SendMessageW(
                        state.slot_combo,
                        CB_GETCURSEL,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0;
                    if let Ok(selected) = usize::try_from(selected) {
                        state.select_slot(window, selected);
                    }
                } else if id == ID_SELECT_IMAGE {
                    state.choose_image(window, state.selected_slot);
                } else if id == ID_CLEAR_IMAGE {
                    state.clear_selected_image(window);
                } else if id == ID_START {
                    state.start(window);
                } else if id == ID_STOP {
                    state.stop();
                } else if id == ID_CLOSE {
                    let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
                } else if id == ID_SETTINGS {
                    state.set_settings_open(window, !state.settings_open);
                } else if id == ID_ALWAYS_ON_TOP {
                    let checked = SendMessageW(
                        state.always_on_top_checkbox,
                        BM_GETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0 == BST_CHECKED.0 as isize;
                    state.set_always_on_top(window, checked);
                } else if id == ID_DELAY_INPUT && notification == EN_KILLFOCUS as usize {
                    let mut value = [0u16; 32];
                    let length = GetWindowTextW(state.click_delay_input, &mut value);
                    let delay = usize::try_from(length)
                        .ok()
                        .and_then(|length| String::from_utf16(&value[..length]).ok())
                        .and_then(|value| value.parse::<u64>().ok())
                        .unwrap_or_default();
                    state.set_click_delay(delay);
                } else if id == ID_THREAD_COUNT_COMBO && notification == CBN_SELCHANGE as usize {
                    let selection = SendMessageW(
                        state.thread_count_combo,
                        CB_GETCURSEL,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0;
                    if let Ok(selection) = usize::try_from(selection) {
                        state.set_match_threads(selection);
                    }
                } else if id == ID_THEME_COMBO && notification == CBN_SELCHANGE as usize {
                    let selection = SendMessageW(
                        state.theme_combo,
                        CB_GETCURSEL,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0;
                    if let Ok(selection) = usize::try_from(selection) {
                        state.set_theme(window, selection);
                    }
                } else if id == ID_SLOT_COUNT_COMBO && notification == CBN_SELCHANGE as usize {
                    let selection = SendMessageW(
                        state.slot_count_combo,
                        CB_GETCURSEL,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0;
                    if let Ok(selection) = usize::try_from(selection) {
                        state.set_image_count(window, selection + 1);
                    }
                }
                LRESULT(0)
            }
            WM_HOTKEY => {
                match wparam.0 {
                    1 => state.start(window),
                    2 => state.stop(),
                    3 => {
                        let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                match wparam.0 {
                    0x70 => state.start(window),
                    0x71 => state.stop(),
                    0x72 => {
                        let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                    _ => return DefWindowProcW(window, message, wparam, lparam),
                }
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER_ID => {
                let worker_finished = state
                    .worker
                    .as_ref()
                    .is_some_and(|worker| !worker.running.load(Ordering::Acquire));
                if worker_finished {
                    if let Some(worker) = state.worker.take() {
                        let clicks = worker.clicks.load(Ordering::Relaxed);
                        let scan_ms =
                            worker.scan_duration_us.load(Ordering::Relaxed) as f64 / 1_000.0;
                        let error = worker
                            .error
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .as_ref()
                            .map(ToString::to_string);
                        let status = if let Some(message) = error {
                            message
                        } else {
                            format!("Stopped | {clicks} clicks | Scan: {scan_ms:.2} ms")
                        };
                        state.set_running_ui(window, false);
                        set_text(state.status_label, &status);
                    }
                } else if let Some(worker) = &state.worker {
                    let clicks = worker.clicks.load(Ordering::Relaxed);
                    let scan_ms = worker.scan_duration_us.load(Ordering::Relaxed) as f64 / 1_000.0;
                    set_text(
                        state.status_label,
                        &format!("Clicks: {clicks} | Scan: {scan_ms:.2} ms"),
                    );
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                state.save_preferences();
                if let Some(worker) = &state.worker {
                    worker.stop.store(true, Ordering::Release);
                }
                for id in 1..=3 {
                    let _ = UnregisterHotKey(Some(window), id);
                }
                let _ = DestroyWindow(window);
                LRESULT(0)
            }
            WM_DESTROY => {
                let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(Some(window), TIMER_ID);
                for id in 1..=3 {
                    let _ = UnregisterHotKey(Some(window), id);
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}
