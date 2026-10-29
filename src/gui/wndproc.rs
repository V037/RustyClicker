use super::{
    ID_ALWAYS_ON_TOP, ID_BRAND_ICON, ID_CLASSICAL_ALWAYS_ON_TOP, ID_CLASSICAL_BUTTON_COMBO,
    ID_CLASSICAL_BUTTON_LABEL, ID_CLASSICAL_CLICK_TYPE_COMBO, ID_CLASSICAL_CLOSE,
    ID_CLASSICAL_DELAY_INPUT, ID_CLASSICAL_DELAY_LABEL, ID_CLASSICAL_HOURS_INPUT,
    ID_CLASSICAL_LOCATION_CURRENT_RADIO, ID_CLASSICAL_LOCATION_PICK_RADIO, ID_CLASSICAL_MINS_INPUT,
    ID_CLASSICAL_NAV_BUTTON, ID_CLASSICAL_PICK_BUTTON, ID_CLASSICAL_PICK_X_INPUT,
    ID_CLASSICAL_PICK_Y_INPUT, ID_CLASSICAL_RANDOM_CHECK, ID_CLASSICAL_RANDOM_INPUT,
    ID_CLASSICAL_REPEAT_COUNT_INPUT, ID_CLASSICAL_REPEAT_COUNT_RADIO,
    ID_CLASSICAL_REPEAT_STOP_RADIO, ID_CLASSICAL_SECS_INPUT, ID_CLASSICAL_START,
    ID_CLASSICAL_STATS, ID_CLASSICAL_STATUS_LABEL, ID_CLASSICAL_STOP, ID_CLASSICAL_TITLE,
    ID_CLEAR_IMAGE, ID_CLICKER_BUTTON, ID_CLOSE, ID_CREDIT_LABEL, ID_DELAY_INPUT, ID_EMPTY_PREVIEW,
    ID_HIDE_WHEN_ACTIVE, ID_HOTKEY_CLOSE_LABEL, ID_HOTKEY_INSTRUCTION, ID_HOTKEY_START_LABEL,
    ID_HOTKEY_STOP_LABEL, ID_IMAGES_NAV_BUTTON, ID_MENU_HOTKEY_LABEL, ID_MENU_STATUS_TITLE,
    ID_MENU_TIPS_LABEL, ID_PREVIEW_LABEL, ID_SELECT_IMAGE, ID_SETTINGS, ID_SETTINGS_HEADING,
    ID_SLOT_COMBO, ID_SLOT_COUNT_INPUT, ID_SLOT_LABEL, ID_START, ID_STATUS_LABEL, ID_STOP,
    ID_THEME_COMBO, ID_THREAD_COUNT_COMBO, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH,
    PICK_LOCATION_TIMER_ID, TIMER_ID,
    controls::create_controls,
    drawing::{draw_button, draw_cursor_icon, draw_empty_preview},
    layout::{layout_controls, scale, update_ui_font},
    preferences::{ClassicalLocationMode, ClassicalRepeatMode, rgb, vk_to_string},
    settings::{
        ID_SETTINGS_DELAY_LABEL, ID_SETTINGS_SLOT_COUNT_LABEL, ID_SETTINGS_THEME_LABEL,
        ID_SETTINGS_THREADS_LABEL,
    },
    state::{ActivePanel, AppState, set_text},
};
use crate::input::{ClickButton, ClickType};
use std::sync::atomic::Ordering;
use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{FillRect, HDC, ScreenToClient, SetBkColor, SetTextColor},
    UI::{
        Controls::{BST_CHECKED, DRAWITEMSTRUCT, NMHDR, NMTREEVIEWW, TVN_SELCHANGEDW},
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, MOD_NOREPEAT, RegisterHotKey, ReleaseCapture, UnregisterHotKey,
            VK_ESCAPE, VK_LBUTTON,
        },
        WindowsAndMessaging::{
            BM_GETCHECK, BM_SETCHECK, CB_GETCURSEL, CBN_SELCHANGE, CallWindowProcW, DefWindowProcW,
            DestroyWindow, EN_KILLFOCUS, GWLP_USERDATA, GWLP_WNDPROC, GetClientRect, GetCursorPos,
            GetDlgCtrlID, GetParent, GetWindowLongPtrW, GetWindowTextW, HTCAPTION, KillTimer,
            MB_ICONERROR, MB_ICONWARNING, MINMAXINFO, PostMessageW, PostQuitMessage,
            SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetTimer, SetWindowLongPtrW, SetWindowPos,
            WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX,
            WM_CTLCOLORSTATIC, WM_DESTROY, WM_DPICHANGED, WM_DRAWITEM, WM_ERASEBKGND,
            WM_GETMINMAXINFO, WM_HOTKEY, WM_KEYDOWN, WM_LBUTTONDOWN, WM_NCCREATE, WM_NCHITTEST,
            WM_NOTIFY, WM_SIZE, WM_TIMER, WNDPROC,
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
            for (id, key) in [
                (1, (*state_ptr).start_hotkey),
                (2, (*state_ptr).stop_hotkey),
                (3, (*state_ptr).close_hotkey),
            ] {
                if RegisterHotKey(Some(window), id, MOD_NOREPEAT, key).is_err() {
                    unavailable_hotkeys.push(vk_to_string(key));
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
                    &format!(
                        "Ready. Global [{}], [{}], and [{}] hotkeys are active.",
                        vk_to_string((*state_ptr).start_hotkey),
                        vk_to_string((*state_ptr).stop_hotkey),
                        vk_to_string((*state_ptr).close_hotkey),
                    ),
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
                ID_MENU_STATUS_TITLE,
                ID_MENU_HOTKEY_LABEL,
                ID_MENU_TIPS_LABEL,
                ID_CLASSICAL_TITLE,
                ID_CLASSICAL_STATS,
                ID_CLASSICAL_STATUS_LABEL,
                ID_CLASSICAL_DELAY_LABEL,
                ID_CLASSICAL_BUTTON_LABEL,
                ID_SETTINGS_HEADING,
                ID_HOTKEY_START_LABEL,
                ID_HOTKEY_STOP_LABEL,
                ID_HOTKEY_CLOSE_LABEL,
                ID_HOTKEY_INSTRUCTION,
                super::logic_gui::ID_LOGIC_TREE_TITLE,
                super::logic_gui::ID_LOGIC_PROPS_TITLE,
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
            sizing.ptMinTrackSize.x = scale(MIN_WINDOW_WIDTH, state.dpi_scale);
            sizing.ptMinTrackSize.y = scale(MIN_WINDOW_HEIGHT, state.dpi_scale);
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
                if state.active_panel == ActivePanel::Clicker
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
                    state.stop(window);
                } else if id == ID_CLOSE {
                    let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
                } else if id == ID_CLASSICAL_START {
                    state.start_classical(window);
                } else if id == ID_CLASSICAL_STOP {
                    state.stop_classical(window);
                } else if id == ID_CLASSICAL_CLOSE {
                    let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
                } else if id == ID_CLASSICAL_NAV_BUTTON {
                    state.set_panel(window, ActivePanel::Classical);
                } else if id == ID_IMAGES_NAV_BUTTON || id == ID_CLICKER_BUTTON {
                    state.set_panel(window, ActivePanel::Clicker);
                } else if id == ID_SETTINGS {
                    if state.active_panel == ActivePanel::MainMenu {
                        state.set_panel(window, ActivePanel::Settings);
                    } else {
                        state.set_panel(window, ActivePanel::MainMenu);
                    }
                } else if id == super::logic_gui::ID_LOGIC_BUTTON {
                    if state.active_panel == ActivePanel::Logic {
                        state.set_panel(window, ActivePanel::MainMenu);
                    } else {
                        state.set_panel(window, ActivePanel::Logic);
                    }
                } else if (super::logic_gui::ID_LOGIC_ADD_CLICK_BTN
                    ..=super::logic_gui::ID_LOGIC_LOAD_BTN)
                    .contains(&id)
                {
                    state.handle_logic_command(window, id);
                } else if id == ID_ALWAYS_ON_TOP {
                    let checked = SendMessageW(
                        state.always_on_top_checkbox,
                        BM_GETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0 == BST_CHECKED.0 as isize;
                    state.set_always_on_top(window, checked);
                    let _ = SendMessageW(
                        state.classical_always_on_top_checkbox,
                        BM_SETCHECK,
                        Some(WPARAM(if checked { BST_CHECKED.0 as usize } else { 0 })),
                        Some(LPARAM(0)),
                    );
                } else if id == ID_CLASSICAL_ALWAYS_ON_TOP {
                    let checked = SendMessageW(
                        state.classical_always_on_top_checkbox,
                        BM_GETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0 == BST_CHECKED.0 as isize;
                    state.set_always_on_top(window, checked);
                    let _ = SendMessageW(
                        state.always_on_top_checkbox,
                        BM_SETCHECK,
                        Some(WPARAM(if checked { BST_CHECKED.0 as usize } else { 0 })),
                        Some(LPARAM(0)),
                    );
                } else if id == ID_HIDE_WHEN_ACTIVE {
                    let checked = SendMessageW(
                        state.hide_when_active_checkbox,
                        BM_GETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0 == BST_CHECKED.0 as isize;
                    state.set_hide_when_active(checked);
                } else if id == ID_CLASSICAL_HOURS_INPUT && notification == EN_KILLFOCUS as usize {
                    state.classical_hours =
                        read_edit_u32(state.classical_hours_input, state.classical_hours);
                    state.sync_classical_interval();
                    state.update_menu_summary();
                    state.save_preferences();
                } else if id == ID_CLASSICAL_MINS_INPUT && notification == EN_KILLFOCUS as usize {
                    state.classical_mins =
                        read_edit_u32(state.classical_mins_input, state.classical_mins);
                    state.sync_classical_interval();
                    state.update_menu_summary();
                    state.save_preferences();
                } else if id == ID_CLASSICAL_SECS_INPUT && notification == EN_KILLFOCUS as usize {
                    state.classical_secs =
                        read_edit_u32(state.classical_secs_input, state.classical_secs);
                    state.sync_classical_interval();
                    state.update_menu_summary();
                    state.save_preferences();
                } else if id == ID_CLASSICAL_DELAY_INPUT && notification == EN_KILLFOCUS as usize {
                    state.classical_millis =
                        read_edit_u32(state.classical_millis_input, state.classical_millis);
                    state.sync_classical_interval();
                    state.update_menu_summary();
                    state.save_preferences();
                } else if id == ID_CLASSICAL_RANDOM_CHECK {
                    let checked = SendMessageW(
                        state.classical_random_checkbox,
                        BM_GETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0 == BST_CHECKED.0 as isize;
                    state.classical_random_offset_enabled = checked;
                    state.save_preferences();
                } else if id == ID_CLASSICAL_RANDOM_INPUT && notification == EN_KILLFOCUS as usize {
                    state.classical_random_offset_ms = read_edit_u32(
                        state.classical_random_input,
                        state.classical_random_offset_ms,
                    );
                    state.save_preferences();
                } else if id == ID_CLASSICAL_BUTTON_COMBO && notification == CBN_SELCHANGE as usize
                {
                    let selection = SendMessageW(
                        state.classical_button_combo,
                        CB_GETCURSEL,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0;
                    let btn = match selection {
                        1 => ClickButton::Right,
                        2 => ClickButton::Middle,
                        _ => ClickButton::Left,
                    };
                    state.set_classical_button(btn);
                } else if id == ID_CLASSICAL_CLICK_TYPE_COMBO
                    && notification == CBN_SELCHANGE as usize
                {
                    let selection = SendMessageW(
                        state.classical_click_type_combo,
                        CB_GETCURSEL,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0;
                    let click_type = match selection {
                        1 => ClickType::Double,
                        _ => ClickType::Single,
                    };
                    state.classical_click_type = click_type;
                    state.save_preferences();
                } else if id == ID_CLASSICAL_REPEAT_STOP_RADIO {
                    state.classical_repeat_mode = ClassicalRepeatMode::UntilStopped;
                    let _ = SendMessageW(
                        state.classical_repeat_stopped_radio,
                        BM_SETCHECK,
                        Some(WPARAM(BST_CHECKED.0 as usize)),
                        Some(LPARAM(0)),
                    );
                    let _ = SendMessageW(
                        state.classical_repeat_count_radio,
                        BM_SETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    );
                    state.save_preferences();
                } else if id == ID_CLASSICAL_REPEAT_COUNT_RADIO {
                    state.classical_repeat_mode = ClassicalRepeatMode::Count;
                    let _ = SendMessageW(
                        state.classical_repeat_stopped_radio,
                        BM_SETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    );
                    let _ = SendMessageW(
                        state.classical_repeat_count_radio,
                        BM_SETCHECK,
                        Some(WPARAM(BST_CHECKED.0 as usize)),
                        Some(LPARAM(0)),
                    );
                    state.save_preferences();
                } else if id == ID_CLASSICAL_REPEAT_COUNT_INPUT
                    && notification == EN_KILLFOCUS as usize
                {
                    let count = read_edit_u32(
                        state.classical_repeat_count_input,
                        state.classical_repeat_count as u32,
                    );
                    state.classical_repeat_count = (count as u64).max(1);
                    state.save_preferences();
                } else if id == ID_CLASSICAL_LOCATION_CURRENT_RADIO {
                    state.classical_location_mode = ClassicalLocationMode::Current;
                    let _ = SendMessageW(
                        state.classical_location_current_radio,
                        BM_SETCHECK,
                        Some(WPARAM(BST_CHECKED.0 as usize)),
                        Some(LPARAM(0)),
                    );
                    let _ = SendMessageW(
                        state.classical_location_pick_radio,
                        BM_SETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    );
                    state.save_preferences();
                } else if id == ID_CLASSICAL_LOCATION_PICK_RADIO {
                    state.classical_location_mode = ClassicalLocationMode::Pick;
                    let _ = SendMessageW(
                        state.classical_location_current_radio,
                        BM_SETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    );
                    let _ = SendMessageW(
                        state.classical_location_pick_radio,
                        BM_SETCHECK,
                        Some(WPARAM(BST_CHECKED.0 as usize)),
                        Some(LPARAM(0)),
                    );
                    state.save_preferences();
                } else if id == ID_CLASSICAL_PICK_X_INPUT && notification == EN_KILLFOCUS as usize {
                    state.classical_pick_x =
                        read_edit_i32(state.classical_pick_x_input, state.classical_pick_x);
                    state.save_preferences();
                } else if id == ID_CLASSICAL_PICK_Y_INPUT && notification == EN_KILLFOCUS as usize {
                    state.classical_pick_y =
                        read_edit_i32(state.classical_pick_y_input, state.classical_pick_y);
                    state.save_preferences();
                } else if id == ID_CLASSICAL_PICK_BUTTON {
                    state.is_picking_location = true;
                    state.pick_released = false;
                    set_text(
                        state.classical_status_label,
                        "Click anywhere to pick coordinate (or Esc to cancel)...",
                    );
                    let _ = SetTimer(Some(window), PICK_LOCATION_TIMER_ID, 30, None);
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
                } else if id == ID_SLOT_COUNT_INPUT && notification == EN_KILLFOCUS as usize {
                    let mut value = [0u16; 32];
                    let length = GetWindowTextW(state.slot_count_input, &mut value);
                    let count = usize::try_from(length)
                        .ok()
                        .and_then(|length| String::from_utf16(&value[..length]).ok())
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or_default();
                    state.set_image_count(window, count);
                }
                LRESULT(0)
            }
            WM_NOTIFY => {
                let nmhdr = &*(lparam.0 as *const NMHDR);
                if nmhdr.idFrom == super::logic_gui::ID_LOGIC_TREE_VIEW
                    && nmhdr.code == TVN_SELCHANGEDW
                {
                    let tv = &*(lparam.0 as *const NMTREEVIEWW);
                    let node_id = tv.itemNew.lParam.0 as usize;
                    if node_id != 0 {
                        state.handle_logic_tree_selchanged(node_id);
                    }
                }
                LRESULT(0)
            }
            WM_HOTKEY => {
                match wparam.0 {
                    1 => match state.active_panel {
                        ActivePanel::Clicker => state.start(window),
                        ActivePanel::Classical => state.start_classical(window),
                        ActivePanel::MainMenu | ActivePanel::Logic | ActivePanel::Settings => {}
                    },
                    2 => state.stop(window),
                    3 => {
                        let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                let key = wparam.0 as u32;
                if key == state.start_hotkey {
                    match state.active_panel {
                        ActivePanel::Clicker => state.start(window),
                        ActivePanel::Classical => state.start_classical(window),
                        ActivePanel::MainMenu | ActivePanel::Logic | ActivePanel::Settings => {}
                    }
                } else if key == state.stop_hotkey {
                    state.stop(window);
                } else if key == state.close_hotkey {
                    let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
                } else {
                    return DefWindowProcW(window, message, wparam, lparam);
                }
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER_ID => {
                if let Some(ref worker) = state.classical_worker {
                    let clicks = worker.clicks();
                    let cps = worker.cps();
                    if !worker.is_running() {
                        state.restore_window(window);
                        set_text(
                            state.classical_stats_label,
                            &format!("Stopped | Clicks: {clicks} | Speed: {cps} CPS"),
                        );
                        let start_str = vk_to_string(state.start_hotkey);
                        set_text(
                            state.classical_status_label,
                            &format!("Stopped. Press {start_str} to start."),
                        );
                        state.classical_worker = None;
                    } else {
                        set_text(
                            state.classical_stats_label,
                            &format!("Clicks: {clicks} | Speed: {cps} CPS"),
                        );
                    }
                }
                let worker_finished = state
                    .worker
                    .as_ref()
                    .is_some_and(|worker| !worker.running.load(Ordering::Acquire));
                if worker_finished {
                    state.restore_window(window);
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
                        let start_str = vk_to_string(state.start_hotkey);
                        let status = if let Some(message) = error {
                            message
                        } else {
                            format!(
                                "Stopped | {clicks} clicks | Scan: {scan_ms:.2} ms | Press {start_str} to start"
                            )
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
                    state.update_menu_summary();
                }
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == PICK_LOCATION_TIMER_ID => {
                if state.is_picking_location {
                    if (GetAsyncKeyState(VK_ESCAPE.0 as i32) as u16 & 0x8000) != 0 {
                        state.is_picking_location = false;
                        let _ = KillTimer(Some(window), PICK_LOCATION_TIMER_ID);
                        let start_str = vk_to_string(state.start_hotkey);
                        set_text(
                            state.classical_status_label,
                            &format!("Cancelled. Press {start_str} to start."),
                        );
                    } else if !state.pick_released {
                        if (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) == 0 {
                            state.pick_released = true;
                        }
                    } else if (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 {
                        let mut pt = POINT::default();
                        let _ = GetCursorPos(&mut pt);
                        state.classical_pick_x = pt.x;
                        state.classical_pick_y = pt.y;
                        state.classical_location_mode = ClassicalLocationMode::Pick;
                        set_text(state.classical_pick_x_input, &pt.x.to_string());
                        set_text(state.classical_pick_y_input, &pt.y.to_string());
                        let _ = SendMessageW(
                            state.classical_location_current_radio,
                            BM_SETCHECK,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        );
                        let _ = SendMessageW(
                            state.classical_location_pick_radio,
                            BM_SETCHECK,
                            Some(WPARAM(BST_CHECKED.0 as usize)),
                            Some(LPARAM(0)),
                        );
                        state.is_picking_location = false;
                        let _ = KillTimer(Some(window), PICK_LOCATION_TIMER_ID);
                        let start_str = vk_to_string(state.start_hotkey);
                        set_text(
                            state.classical_status_label,
                            &format!("Picked ({}, {}). Press {start_str} to start.", pt.x, pt.y),
                        );
                        state.save_preferences();
                    }
                } else {
                    let _ = KillTimer(Some(window), PICK_LOCATION_TIMER_ID);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = KillTimer(Some(window), PICK_LOCATION_TIMER_ID);
                state.is_picking_location = false;
                state.restore_window(window);
                state.save_preferences();
                state.stop_classical(window);
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
                let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(
                    Some(window),
                    PICK_LOCATION_TIMER_ID,
                );
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

unsafe fn read_edit_u32(hwnd: HWND, default: u32) -> u32 {
    let mut value = [0u16; 32];
    let length = unsafe { GetWindowTextW(hwnd, &mut value) };
    usize::try_from(length)
        .ok()
        .and_then(|length| String::from_utf16(&value[..length]).ok())
        .and_then(|text| text.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

unsafe fn read_edit_i32(hwnd: HWND, default: i32) -> i32 {
    let mut value = [0u16; 32];
    let length = unsafe { GetWindowTextW(hwnd, &mut value) };
    usize::try_from(length)
        .ok()
        .and_then(|length| String::from_utf16(&value[..length]).ok())
        .and_then(|text| text.trim().parse::<i32>().ok())
        .unwrap_or(default)
}
