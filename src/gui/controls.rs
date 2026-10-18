use super::{
    BUTTON_STYLE, ID_ALWAYS_ON_TOP, ID_BRAND_ICON, ID_CLEAR_IMAGE, ID_CLICKER_BUTTON, ID_CLOSE,
    ID_CREDIT_LABEL, ID_DELAY_INPUT, ID_EMPTY_PREVIEW, ID_MENU_HOTKEY_LABEL, ID_MENU_STATUS_TITLE,
    ID_MENU_SUMMARY_LABEL, ID_MENU_TIPS_LABEL, ID_PREVIEW_LABEL, ID_SELECT_IMAGE, ID_SETTINGS,
    ID_SLOT_COMBO, ID_SLOT_COUNT_COMBO, ID_SLOT_LABEL, ID_START, ID_STATUS_LABEL, ID_STOP,
    ID_THEME_COMBO, ID_THREAD_COUNT_COMBO, ID_VERSION_LABEL, MAX_IMAGE_COUNT,
    layout::{layout_controls, update_ui_font},
    logic_gui::{ID_LOGIC_BUTTON, create_logic_controls},
    preferences::Theme,
    settings::{
        ID_SETTINGS_DELAY_LABEL, ID_SETTINGS_SLOT_COUNT_LABEL, ID_SETTINGS_THEME_LABEL,
        ID_SETTINGS_THREADS_LABEL, THEMES,
    },
    state::AppState,
    wndproc::make_static_draggable,
};
use crate::matching::MATCH_THREAD_OPTIONS;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
        UI::{
            Controls::BST_CHECKED,
            HiDpi::GetDpiForWindow,
            WindowsAndMessaging::{
                BM_SETCHECK, BS_AUTOCHECKBOX, CB_ADDSTRING, CB_SETCURSEL, CBS_DROPDOWNLIST,
                CreateWindowExW, ES_AUTOHSCROLL, ES_NUMBER, HMENU, HWND_TOPMOST, MB_ICONERROR,
                MB_ICONWARNING, SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                SendMessageW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WS_BORDER,
                WS_CHILD, WS_VISIBLE,
            },
        },
    },
    core::{PCWSTR, w},
};

pub(crate) unsafe fn create_control(
    parent: HWND,
    class: PCWSTR,
    text: &str,
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: usize,
) -> HWND {
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            PCWSTR(text.as_ptr()),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            width,
            height,
            Some(parent),
            Some(HMENU(id as *mut _)),
            None,
            None,
        )
        .unwrap_or_default()
    }
}

pub(crate) unsafe fn create_controls(window: HWND, state: &mut AppState) {
    unsafe {
        state.main_brand_icon = create_control(
            window,
            w!("STATIC"),
            "",
            WINDOW_STYLE(windows::Win32::System::SystemServices::SS_OWNERDRAW.0),
            16,
            12,
            30,
            30,
            ID_BRAND_ICON,
        );
        state.main_heading = create_control(
            window,
            w!("STATIC"),
            "RustyClicker",
            WINDOW_STYLE(0),
            52,
            12,
            300,
            30,
            0,
        );
        state.version_label = create_control(
            window,
            w!("STATIC"),
            concat!("v", env!("CARGO_PKG_VERSION")),
            WINDOW_STYLE(0),
            290,
            22,
            48,
            18,
            ID_VERSION_LABEL,
        );
        state.settings_button = create_control(
            window,
            w!("BUTTON"),
            "Settings",
            WINDOW_STYLE(BUTTON_STYLE),
            16,
            12,
            90,
            30,
            ID_SETTINGS,
        );
        state.logic_button = create_control(
            window,
            w!("BUTTON"),
            "Logic",
            WINDOW_STYLE(BUTTON_STYLE),
            16,
            12,
            80,
            30,
            ID_LOGIC_BUTTON,
        );
        state.clicker_button = create_control(
            window,
            w!("BUTTON"),
            "Clicker",
            WINDOW_STYLE(BUTTON_STYLE),
            16,
            12,
            80,
            30,
            ID_CLICKER_BUTTON,
        );
        state.menu_status_title = create_control(
            window,
            w!("STATIC"),
            "SYSTEM OVERVIEW",
            WINDOW_STYLE(0),
            18,
            64,
            400,
            20,
            ID_MENU_STATUS_TITLE,
        );
        state.menu_summary_label = create_control(
            window,
            w!("STATIC"),
            "Initializing...",
            WINDOW_STYLE(windows::Win32::System::SystemServices::SS_LEFTNOWORDWRAP.0),
            18,
            88,
            400,
            22,
            ID_MENU_SUMMARY_LABEL,
        );
        state.menu_hotkey_label = create_control(
            window,
            w!("STATIC"),
            "Global Hotkeys:  [F1] Start Detection   •   [F2] Stop Worker   •   [F3] Close",
            WINDOW_STYLE(windows::Win32::System::SystemServices::SS_LEFTNOWORDWRAP.0),
            18,
            116,
            400,
            22,
            ID_MENU_HOTKEY_LABEL,
        );
        state.menu_tips_label = create_control(
            window,
            w!("STATIC"),
            "Tip: Press F1 anytime to start scanning across all active monitors.",
            WINDOW_STYLE(windows::Win32::System::SystemServices::SS_LEFTNOWORDWRAP.0),
            18,
            144,
            400,
            22,
            ID_MENU_TIPS_LABEL,
        );
        state.menu_controls = vec![
            state.menu_status_title,
            state.menu_summary_label,
            state.menu_hotkey_label,
            state.menu_tips_label,
        ];
        state.slot_label = create_control(
            window,
            w!("STATIC"),
            "TARGET SLOT",
            WINDOW_STYLE(0),
            18,
            74,
            140,
            24,
            ID_SLOT_LABEL,
        );
        state.slot_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            18,
            98,
            180,
            240,
            ID_SLOT_COMBO,
        );
        state.refresh_slot_names();
        state.preview_label = create_control(
            window,
            w!("STATIC"),
            "IMAGE PREVIEW",
            WINDOW_STYLE(0),
            18,
            150,
            180,
            22,
            ID_PREVIEW_LABEL,
        );
        state.preview = create_control(
            window,
            w!("STATIC"),
            "",
            WINDOW_STYLE(
                windows::Win32::System::SystemServices::SS_BITMAP.0
                    | windows::Win32::System::SystemServices::SS_CENTERIMAGE.0,
            ),
            18,
            174,
            480,
            190,
            0,
        );
        state.empty_preview = create_control(
            window,
            w!("STATIC"),
            "No image selected",
            WINDOW_STYLE(windows::Win32::System::SystemServices::SS_OWNERDRAW.0),
            18,
            174,
            480,
            190,
            ID_EMPTY_PREVIEW,
        );
        state.image_name = create_control(
            window,
            w!("STATIC"),
            "No image selected for this slot",
            WINDOW_STYLE(0),
            18,
            372,
            480,
            28,
            0,
        );
        state.select_image_button = create_control(
            window,
            w!("BUTTON"),
            "Choose image",
            WINDOW_STYLE(BUTTON_STYLE),
            18,
            410,
            180,
            38,
            ID_SELECT_IMAGE,
        );
        state.clear_image_button = create_control(
            window,
            w!("BUTTON"),
            "Clear slot",
            WINDOW_STYLE(BUTTON_STYLE),
            210,
            410,
            140,
            38,
            ID_CLEAR_IMAGE,
        );
        state.status_label = create_control(
            window,
            w!("STATIC"),
            "Ready  |  Select at least one target image.",
            WINDOW_STYLE(windows::Win32::System::SystemServices::SS_LEFTNOWORDWRAP.0),
            20,
            516,
            500,
            28,
            ID_STATUS_LABEL,
        );
        state.credit_label = create_control(
            window,
            w!("STATIC"),
            "by V037_",
            WINDOW_STYLE(0),
            56,
            44,
            150,
            18,
            ID_CREDIT_LABEL,
        );
        state.start_button = create_control(
            window,
            w!("BUTTON"),
            "F1 - Start",
            WINDOW_STYLE(BUTTON_STYLE),
            18,
            466,
            112,
            40,
            ID_START,
        );
        state.stop_button = create_control(
            window,
            w!("BUTTON"),
            "F2 - Stop",
            WINDOW_STYLE(BUTTON_STYLE),
            18,
            466,
            112,
            40,
            ID_STOP,
        );
        state.close_button = create_control(
            window,
            w!("BUTTON"),
            "F3 - Close",
            WINDOW_STYLE(BUTTON_STYLE),
            18,
            466,
            112,
            40,
            ID_CLOSE,
        );
        state.always_on_top_checkbox = create_control(
            window,
            w!("BUTTON"),
            "Always on top",
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            18,
            466,
            112,
            40,
            ID_ALWAYS_ON_TOP,
        );
        state.clicker_controls = vec![
            state.slot_label,
            state.slot_combo,
            state.preview_label,
            state.preview,
            state.empty_preview,
            state.image_name,
            state.select_image_button,
            state.clear_image_button,
            state.status_label,
            state.start_button,
            state.stop_button,
            state.close_button,
            state.always_on_top_checkbox,
        ];
        SendMessageW(
            state.always_on_top_checkbox,
            BM_SETCHECK,
            Some(WPARAM(if state.always_on_top {
                BST_CHECKED.0 as usize
            } else {
                0
            })),
            Some(LPARAM(0)),
        );

        state.settings_title = create_control(
            window,
            w!("STATIC"),
            "Click interval (ms)",
            WINDOW_STYLE(0),
            24,
            55,
            260,
            24,
            ID_SETTINGS_DELAY_LABEL,
        );
        state.click_delay_input = create_control(
            window,
            w!("EDIT"),
            &state.click_delay_ms.to_string(),
            WINDOW_STYLE(ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            24,
            80,
            200,
            26,
            ID_DELAY_INPUT,
        );
        let thread_count_label = create_control(
            window,
            w!("STATIC"),
            "Scanning threads",
            WINDOW_STYLE(0),
            24,
            116,
            260,
            24,
            ID_SETTINGS_THREADS_LABEL,
        );
        state.thread_count_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            24,
            140,
            200,
            110,
            ID_THREAD_COUNT_COMBO,
        );
        let theme_label = create_control(
            window,
            w!("STATIC"),
            "Window color",
            WINDOW_STYLE(0),
            24,
            178,
            260,
            24,
            ID_SETTINGS_THEME_LABEL,
        );
        state.theme_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            24,
            202,
            200,
            110,
            ID_THEME_COMBO,
        );
        let slot_count_label = create_control(
            window,
            w!("STATIC"),
            "Number of slots",
            WINDOW_STYLE(0),
            24,
            240,
            260,
            24,
            ID_SETTINGS_SLOT_COUNT_LABEL,
        );
        state.slot_count_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            24,
            264,
            200,
            120,
            ID_SLOT_COUNT_COMBO,
        );
        let settings_note = create_control(
            window,
            w!("STATIC"),
            "Changes are saved automatically.",
            WINDOW_STYLE(0),
            24,
            298,
            440,
            24,
            0,
        );
        state.settings_controls = vec![
            state.settings_title,
            state.click_delay_input,
            thread_count_label,
            state.thread_count_combo,
            theme_label,
            state.theme_combo,
            slot_count_label,
            state.slot_count_combo,
            settings_note,
        ];
        create_logic_controls(window, state);
        let mut drag_setup_failed = false;
        for control in [
            state.main_brand_icon,
            state.main_heading,
            state.version_label,
            state.credit_label,
            state.menu_status_title,
            state.menu_summary_label,
            state.menu_hotkey_label,
            state.menu_tips_label,
            state.slot_label,
            state.preview_label,
            state.preview,
            state.empty_preview,
            state.image_name,
            state.status_label,
            state.settings_title,
            thread_count_label,
            theme_label,
            slot_count_label,
            settings_note,
        ] {
            drag_setup_failed |= !make_static_draggable(control);
        }
        if drag_setup_failed {
            state.show_message("Some labels could not be made draggable.", MB_ICONWARNING);
        }
        for (index, threads) in MATCH_THREAD_OPTIONS.iter().enumerate() {
            let label = threads.to_string();
            let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            SendMessageW(
                state.thread_count_combo,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(wide.as_ptr() as isize)),
            );
            if *threads == state.match_threads {
                SendMessageW(
                    state.thread_count_combo,
                    CB_SETCURSEL,
                    Some(WPARAM(index)),
                    Some(LPARAM(0)),
                );
            }
        }
        for (index, theme) in THEMES.iter().enumerate() {
            let wide: Vec<u16> = theme.name().encode_utf16().chain(Some(0)).collect();
            SendMessageW(
                state.theme_combo,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(wide.as_ptr() as isize)),
            );
            if *theme == state.theme {
                SendMessageW(
                    state.theme_combo,
                    CB_SETCURSEL,
                    Some(WPARAM(index)),
                    Some(LPARAM(0)),
                );
            }
        }
        for count in 1..=MAX_IMAGE_COUNT {
            let label = count.to_string();
            let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            SendMessageW(
                state.slot_count_combo,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(wide.as_ptr() as isize)),
            );
        }
        SendMessageW(
            state.slot_count_combo,
            CB_SETCURSEL,
            Some(WPARAM(state.image_count - 1)),
            Some(LPARAM(0)),
        );
        state.background_brush = state.theme.brush();
        state.surface_brush = state.theme.surface_brush();
        if state.background_brush.is_null() || state.surface_brush.is_null() {
            state.show_message("Could not create the window background.", MB_ICONERROR);
        }
        for control in &state.clicker_controls {
            let _ = ShowWindow(*control, SW_HIDE);
        }
        for control in &state.settings_controls {
            let _ = ShowWindow(*control, SW_HIDE);
        }
        for control in &state.logic_controls {
            let _ = ShowWindow(*control, SW_HIDE);
        }
        for control in &state.menu_controls {
            let _ = ShowWindow(*control, SW_SHOW);
        }
        state.update_menu_summary();
        let dark_titlebar = i32::from(state.theme != Theme::Light);
        let _ = DwmSetWindowAttribute(
            window,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_titlebar as *const i32 as *const _,
            std::mem::size_of_val(&dark_titlebar) as u32,
        );
        if state.always_on_top {
            let _ = SetWindowPos(
                window,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        let dpi = GetDpiForWindow(window);
        state.dpi_scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
        update_ui_font(state, dpi.max(96));
        layout_controls(window, state);
        if state.preview_source.is_some()
            && let Err(error) = state.update_preview_bitmap(window)
        {
            state.show_message(&error, MB_ICONERROR);
        }
    }
}
