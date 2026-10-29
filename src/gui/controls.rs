use super::{
    BUTTON_STYLE, ID_ALWAYS_ON_TOP, ID_BRAND_ICON, ID_CLASSICAL_ALWAYS_ON_TOP,
    ID_CLASSICAL_BUTTON_COMBO, ID_CLASSICAL_BUTTON_LABEL, ID_CLASSICAL_CLICK_TYPE_COMBO,
    ID_CLASSICAL_CLOSE, ID_CLASSICAL_DELAY_INPUT, ID_CLASSICAL_DELAY_LABEL,
    ID_CLASSICAL_HOURS_INPUT, ID_CLASSICAL_LOCATION_CURRENT_RADIO,
    ID_CLASSICAL_LOCATION_PICK_RADIO, ID_CLASSICAL_MINS_INPUT, ID_CLASSICAL_NAV_BUTTON,
    ID_CLASSICAL_PICK_BUTTON, ID_CLASSICAL_PICK_X_INPUT, ID_CLASSICAL_PICK_Y_INPUT,
    ID_CLASSICAL_RANDOM_CHECK, ID_CLASSICAL_RANDOM_INPUT, ID_CLASSICAL_REPEAT_COUNT_INPUT,
    ID_CLASSICAL_REPEAT_COUNT_RADIO, ID_CLASSICAL_REPEAT_STOP_RADIO, ID_CLASSICAL_SECS_INPUT,
    ID_CLASSICAL_START, ID_CLASSICAL_STATS, ID_CLASSICAL_STATUS_LABEL, ID_CLASSICAL_STOP,
    ID_CLASSICAL_TITLE, ID_CLEAR_IMAGE, ID_CLOSE, ID_CREDIT_LABEL, ID_DELAY_INPUT,
    ID_EMPTY_PREVIEW, ID_HIDE_WHEN_ACTIVE, ID_HOTKEY_CLOSE_INPUT, ID_HOTKEY_CLOSE_LABEL,
    ID_HOTKEY_INSTRUCTION, ID_HOTKEY_START_INPUT, ID_HOTKEY_START_LABEL, ID_HOTKEY_STOP_INPUT,
    ID_HOTKEY_STOP_LABEL, ID_IMAGES_NAV_BUTTON, ID_MENU_HOTKEY_LABEL, ID_MENU_STATUS_TITLE,
    ID_MENU_SUMMARY_LABEL, ID_MENU_TIPS_LABEL, ID_PREVIEW_LABEL, ID_SELECT_IMAGE, ID_SETTINGS,
    ID_SETTINGS_HEADING, ID_SLOT_COMBO, ID_SLOT_COUNT_INPUT, ID_SLOT_LABEL, ID_START,
    ID_STATUS_LABEL, ID_STOP, ID_THEME_COMBO, ID_THREAD_COUNT_COMBO, ID_VERSION_LABEL,
    layout::{layout_controls, update_ui_font},
    logic_gui::{ID_LOGIC_BUTTON, create_logic_controls},
    preferences::{ClassicalLocationMode, ClassicalRepeatMode, Theme, vk_to_string},
    settings::{
        ID_SETTINGS_DELAY_LABEL, ID_SETTINGS_SLOT_COUNT_LABEL, ID_SETTINGS_THEME_LABEL,
        ID_SETTINGS_THREADS_LABEL, THEMES,
    },
    state::{AppState, set_text},
    wndproc::make_static_draggable,
};
use crate::input::{ClickButton, ClickType};
use crate::matching::MATCH_THREAD_OPTIONS;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
        UI::{
            Controls::BST_CHECKED,
            HiDpi::GetDpiForWindow,
            WindowsAndMessaging::{
                BM_SETCHECK, BS_AUTOCHECKBOX, BS_AUTORADIOBUTTON, CB_ADDSTRING, CB_SETCURSEL,
                CBS_DROPDOWNLIST, CallWindowProcW, CreateWindowExW, DefWindowProcW, ES_AUTOHSCROLL,
                ES_NUMBER, GWLP_USERDATA, GWLP_WNDPROC, GetDlgCtrlID, GetParent, GetPropW,
                GetWindowLongPtrW, HMENU, HWND_TOPMOST, MB_ICONERROR, MB_ICONWARNING, SW_HIDE,
                SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SendMessageW, SetPropW,
                SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE,
                WM_CHAR, WM_KEYDOWN, WM_LBUTTONDOWN, WM_SETFOCUS, WM_SYSKEYDOWN, WNDPROC,
                WS_BORDER, WS_CHILD, WS_VISIBLE,
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

pub(crate) unsafe extern "system" fn hotkey_input_proc(
    control: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let prev_proc_ptr = GetPropW(control, w!("PrevProc"));
        let prev_proc: WNDPROC = if prev_proc_ptr.0.is_null() {
            None
        } else {
            std::mem::transmute::<isize, WNDPROC>(prev_proc_ptr.0 as isize)
        };

        if message == WM_LBUTTONDOWN || message == WM_SETFOCUS {
            set_text(control, "[ Press a key... ]");
        } else if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
            let vk = wparam.0 as u32;
            if let Ok(parent) = GetParent(control) {
                let state_ptr = GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut AppState;
                if !state_ptr.is_null() {
                    let ctrl_id = usize::try_from(GetDlgCtrlID(control)).unwrap_or_default();
                    if vk == 0x1B {
                        let cur_vk = match ctrl_id {
                            ID_HOTKEY_START_INPUT => (*state_ptr).start_hotkey,
                            ID_HOTKEY_STOP_INPUT => (*state_ptr).stop_hotkey,
                            ID_HOTKEY_CLOSE_INPUT => (*state_ptr).close_hotkey,
                            _ => 0,
                        };
                        set_text(control, &vk_to_string(cur_vk));
                    } else {
                        let hotkey_id = match ctrl_id {
                            ID_HOTKEY_START_INPUT => 1,
                            ID_HOTKEY_STOP_INPUT => 2,
                            ID_HOTKEY_CLOSE_INPUT => 3,
                            _ => 0,
                        };
                        if hotkey_id != 0 {
                            (*state_ptr).set_hotkey(parent, hotkey_id, vk);
                        }
                    }
                }
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(parent));
            }
            return LRESULT(0);
        } else if message == WM_CHAR {
            return LRESULT(0);
        }

        if prev_proc.is_some() {
            CallWindowProcW(prev_proc, control, message, wparam, lparam)
        } else {
            DefWindowProcW(control, message, wparam, lparam)
        }
    }
}

pub(crate) fn make_hotkey_input(control: HWND) {
    unsafe {
        let prev_proc = SetWindowLongPtrW(
            control,
            GWLP_WNDPROC,
            hotkey_input_proc as *const () as isize,
        );
        if prev_proc != 0 {
            let _ = SetPropW(
                control,
                w!("PrevProc"),
                Some(windows::Win32::Foundation::HANDLE(prev_proc as *mut _)),
            );
        }
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
            "⚙",
            WINDOW_STYLE(BUTTON_STYLE),
            16,
            12,
            30,
            26,
            ID_SETTINGS,
        );
        state.classical_nav_button = create_control(
            window,
            w!("BUTTON"),
            "Classical",
            WINDOW_STYLE(BUTTON_STYLE),
            18,
            56,
            86,
            28,
            ID_CLASSICAL_NAV_BUTTON,
        );
        state.images_button = create_control(
            window,
            w!("BUTTON"),
            "Images",
            WINDOW_STYLE(BUTTON_STYLE),
            104,
            56,
            86,
            28,
            ID_IMAGES_NAV_BUTTON,
        );
        state.logic_button = create_control(
            window,
            w!("BUTTON"),
            "Logic",
            WINDOW_STYLE(BUTTON_STYLE),
            198,
            56,
            76,
            28,
            ID_LOGIC_BUTTON,
        );
        state.clicker_button = state.images_button;
        state.menu_status_title = create_control(
            window,
            w!("STATIC"),
            "SYSTEM OVERVIEW",
            WINDOW_STYLE(0),
            18,
            96,
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
            120,
            400,
            22,
            ID_MENU_SUMMARY_LABEL,
        );
        state.menu_hotkey_label = create_control(
            window,
            w!("STATIC"),
            "Global Hotkeys:  [F1] Start Autoclicker   •   [F2] Stop Worker   •   [F3] Close",
            WINDOW_STYLE(windows::Win32::System::SystemServices::SS_LEFTNOWORDWRAP.0),
            18,
            146,
            400,
            22,
            ID_MENU_HOTKEY_LABEL,
        );
        state.menu_tips_label = create_control(
            window,
            w!("STATIC"),
            "Tip: Open Classical or Images, then press F1 to start.",
            WINDOW_STYLE(windows::Win32::System::SystemServices::SS_LEFTNOWORDWRAP.0),
            18,
            172,
            400,
            22,
            ID_MENU_TIPS_LABEL,
        );
        state.menu_controls = vec![
            state.classical_nav_button,
            state.images_button,
            state.logic_button,
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
        let slot_count_label = create_control(
            window,
            w!("STATIC"),
            "Number of slots",
            WINDOW_STYLE(0),
            18,
            74,
            140,
            24,
            ID_SETTINGS_SLOT_COUNT_LABEL,
        );
        state.slot_count_input = create_control(
            window,
            w!("EDIT"),
            &state.image_count.to_string(),
            WINDOW_STYLE(ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            18,
            98,
            180,
            26,
            ID_SLOT_COUNT_INPUT,
        );
        state.settings_title = create_control(
            window,
            w!("STATIC"),
            "Click interval (ms)",
            WINDOW_STYLE(0),
            18,
            126,
            140,
            24,
            ID_SETTINGS_DELAY_LABEL,
        );
        state.click_delay_input = create_control(
            window,
            w!("EDIT"),
            &state.click_delay_ms.to_string(),
            WINDOW_STYLE(ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            18,
            150,
            180,
            26,
            ID_DELAY_INPUT,
        );
        let thread_count_label = create_control(
            window,
            w!("STATIC"),
            "Scanning threads",
            WINDOW_STYLE(0),
            18,
            178,
            140,
            24,
            ID_SETTINGS_THREADS_LABEL,
        );
        state.thread_count_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            18,
            202,
            180,
            110,
            ID_THREAD_COUNT_COMBO,
        );
        state.clicker_controls = vec![
            state.slot_label,
            state.slot_combo,
            slot_count_label,
            state.slot_count_input,
            state.settings_title,
            state.click_delay_input,
            thread_count_label,
            state.thread_count_combo,
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
        ];

        state.classical_title = create_control(
            window,
            w!("STATIC"),
            "CLASSICAL AUTOCLICKER",
            WINDOW_STYLE(0),
            18,
            56,
            300,
            22,
            ID_CLASSICAL_TITLE,
        );
        state.classical_subtitle = create_control(
            window,
            w!("STATIC"),
            "Ultra-fast direct cursor clicking at current mouse position",
            WINDOW_STYLE(0),
            18,
            80,
            400,
            18,
            0,
        );
        state.classical_delay_label = create_control(
            window,
            w!("STATIC"),
            "Click interval",
            WINDOW_STYLE(0),
            18,
            60,
            260,
            18,
            ID_CLASSICAL_DELAY_LABEL,
        );
        state.classical_hours_input = create_control(
            window,
            w!("EDIT"),
            &state.classical_hours.to_string(),
            WINDOW_STYLE(ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            18,
            80,
            40,
            22,
            ID_CLASSICAL_HOURS_INPUT,
        );
        state.classical_hours_label = create_control(
            window,
            w!("STATIC"),
            "hours",
            WINDOW_STYLE(0),
            60,
            80,
            38,
            18,
            0,
        );
        state.classical_mins_input = create_control(
            window,
            w!("EDIT"),
            &state.classical_mins.to_string(),
            WINDOW_STYLE(ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            100,
            80,
            40,
            22,
            ID_CLASSICAL_MINS_INPUT,
        );
        state.classical_mins_label = create_control(
            window,
            w!("STATIC"),
            "mins",
            WINDOW_STYLE(0),
            142,
            80,
            32,
            18,
            0,
        );
        state.classical_secs_input = create_control(
            window,
            w!("EDIT"),
            &state.classical_secs.to_string(),
            WINDOW_STYLE(ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            176,
            80,
            40,
            22,
            ID_CLASSICAL_SECS_INPUT,
        );
        state.classical_secs_label = create_control(
            window,
            w!("STATIC"),
            "secs",
            WINDOW_STYLE(0),
            218,
            80,
            32,
            18,
            0,
        );
        state.classical_millis_input = create_control(
            window,
            w!("EDIT"),
            &state.classical_millis.to_string(),
            WINDOW_STYLE(ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            252,
            80,
            45,
            22,
            ID_CLASSICAL_DELAY_INPUT,
        );
        state.classical_delay_input = state.classical_millis_input;
        state.classical_millis_label = create_control(
            window,
            w!("STATIC"),
            "milliseconds",
            WINDOW_STYLE(0),
            300,
            80,
            75,
            18,
            0,
        );

        state.classical_random_checkbox = create_control(
            window,
            w!("BUTTON"),
            "Random offset ±",
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            18,
            106,
            114,
            20,
            ID_CLASSICAL_RANDOM_CHECK,
        );
        state.classical_random_input = create_control(
            window,
            w!("EDIT"),
            &state.classical_random_offset_ms.to_string(),
            WINDOW_STYLE(ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            134,
            106,
            40,
            22,
            ID_CLASSICAL_RANDOM_INPUT,
        );
        state.classical_random_label = create_control(
            window,
            w!("STATIC"),
            "milliseconds",
            WINDOW_STYLE(0),
            178,
            106,
            75,
            18,
            0,
        );

        // Click options (Left column)
        state.classical_options_title = create_control(
            window,
            w!("STATIC"),
            "Click options",
            WINDOW_STYLE(0),
            18,
            132,
            150,
            18,
            0,
        );
        state.classical_button_label = create_control(
            window,
            w!("STATIC"),
            "Mouse button:",
            WINDOW_STYLE(0),
            18,
            152,
            80,
            18,
            ID_CLASSICAL_BUTTON_LABEL,
        );
        state.classical_button_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            100,
            150,
            68,
            100,
            ID_CLASSICAL_BUTTON_COMBO,
        );
        state.classical_click_type_label = create_control(
            window,
            w!("STATIC"),
            "Click type:",
            WINDOW_STYLE(0),
            18,
            176,
            80,
            18,
            0,
        );
        state.classical_click_type_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            100,
            174,
            68,
            100,
            ID_CLASSICAL_CLICK_TYPE_COMBO,
        );

        // Click repeat (Right column)
        state.classical_repeat_title = create_control(
            window,
            w!("STATIC"),
            "Click repeat",
            WINDOW_STYLE(0),
            180,
            132,
            150,
            18,
            0,
        );
        state.classical_repeat_count_radio = create_control(
            window,
            w!("BUTTON"),
            "Repeat",
            WINDOW_STYLE(BS_AUTORADIOBUTTON as u32),
            180,
            152,
            58,
            18,
            ID_CLASSICAL_REPEAT_COUNT_RADIO,
        );
        state.classical_repeat_count_input = create_control(
            window,
            w!("EDIT"),
            &state.classical_repeat_count.to_string(),
            WINDOW_STYLE(ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            240,
            150,
            38,
            22,
            ID_CLASSICAL_REPEAT_COUNT_INPUT,
        );
        state.classical_repeat_count_label = create_control(
            window,
            w!("STATIC"),
            "times",
            WINDOW_STYLE(0),
            282,
            152,
            36,
            18,
            0,
        );
        state.classical_repeat_stopped_radio = create_control(
            window,
            w!("BUTTON"),
            "Repeat until stopped",
            WINDOW_STYLE(BS_AUTORADIOBUTTON as u32),
            180,
            176,
            140,
            18,
            ID_CLASSICAL_REPEAT_STOP_RADIO,
        );

        // Cursor position (Bottom row)
        state.classical_location_title = create_control(
            window,
            w!("STATIC"),
            "Cursor position",
            WINDOW_STYLE(0),
            18,
            202,
            200,
            18,
            0,
        );
        state.classical_location_current_radio = create_control(
            window,
            w!("BUTTON"),
            "Current location",
            WINDOW_STYLE(BS_AUTORADIOBUTTON as u32),
            18,
            222,
            110,
            18,
            ID_CLASSICAL_LOCATION_CURRENT_RADIO,
        );
        state.classical_location_pick_radio = create_control(
            window,
            w!("BUTTON"),
            "",
            WINDOW_STYLE(BS_AUTORADIOBUTTON as u32),
            130,
            222,
            18,
            18,
            ID_CLASSICAL_LOCATION_PICK_RADIO,
        );
        state.classical_pick_button = create_control(
            window,
            w!("BUTTON"),
            "Pick location",
            WINDOW_STYLE(BUTTON_STYLE),
            150,
            220,
            84,
            22,
            ID_CLASSICAL_PICK_BUTTON,
        );
        state.classical_pick_x_label = create_control(
            window,
            w!("STATIC"),
            "X",
            WINDOW_STYLE(0),
            238,
            222,
            14,
            18,
            0,
        );
        state.classical_pick_x_input = create_control(
            window,
            w!("EDIT"),
            &state.classical_pick_x.to_string(),
            WINDOW_STYLE(ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            254,
            220,
            34,
            22,
            ID_CLASSICAL_PICK_X_INPUT,
        );
        state.classical_pick_y_label = create_control(
            window,
            w!("STATIC"),
            "Y",
            WINDOW_STYLE(0),
            292,
            222,
            14,
            18,
            0,
        );
        state.classical_pick_y_input = create_control(
            window,
            w!("EDIT"),
            &state.classical_pick_y.to_string(),
            WINDOW_STYLE(ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            308,
            220,
            34,
            22,
            ID_CLASSICAL_PICK_Y_INPUT,
        );

        state.classical_stats_label = create_control(
            window,
            w!("STATIC"),
            "Clicks: 0  |  Speed: 0 CPS",
            WINDOW_STYLE(0),
            18,
            250,
            300,
            22,
            ID_CLASSICAL_STATS,
        );
        state.classical_status_label = create_control(
            window,
            w!("STATIC"),
            "Ready  |  Press F1 to start, F2 to stop.",
            WINDOW_STYLE(windows::Win32::System::SystemServices::SS_LEFTNOWORDWRAP.0),
            20,
            516,
            500,
            28,
            ID_CLASSICAL_STATUS_LABEL,
        );
        state.classical_start_button = create_control(
            window,
            w!("BUTTON"),
            "F1 - Start",
            WINDOW_STYLE(BUTTON_STYLE),
            18,
            466,
            112,
            40,
            ID_CLASSICAL_START,
        );
        state.classical_stop_button = create_control(
            window,
            w!("BUTTON"),
            "F2 - Stop",
            WINDOW_STYLE(BUTTON_STYLE),
            18,
            466,
            112,
            40,
            ID_CLASSICAL_STOP,
        );
        state.classical_close_button = create_control(
            window,
            w!("BUTTON"),
            "F3 - Close",
            WINDOW_STYLE(BUTTON_STYLE),
            18,
            466,
            112,
            40,
            ID_CLASSICAL_CLOSE,
        );
        state.classical_always_on_top_checkbox = create_control(
            window,
            w!("BUTTON"),
            "Always on top",
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            18,
            466,
            112,
            40,
            ID_CLASSICAL_ALWAYS_ON_TOP,
        );
        state.classical_controls = vec![
            state.classical_title,
            state.classical_subtitle,
            state.classical_delay_label,
            state.classical_hours_input,
            state.classical_hours_label,
            state.classical_mins_input,
            state.classical_mins_label,
            state.classical_secs_input,
            state.classical_secs_label,
            state.classical_millis_input,
            state.classical_millis_label,
            state.classical_random_checkbox,
            state.classical_random_input,
            state.classical_random_label,
            state.classical_options_title,
            state.classical_button_label,
            state.classical_button_combo,
            state.classical_click_type_label,
            state.classical_click_type_combo,
            state.classical_repeat_title,
            state.classical_repeat_count_radio,
            state.classical_repeat_count_input,
            state.classical_repeat_count_label,
            state.classical_repeat_stopped_radio,
            state.classical_location_title,
            state.classical_location_current_radio,
            state.classical_location_pick_radio,
            state.classical_pick_button,
            state.classical_pick_x_label,
            state.classical_pick_x_input,
            state.classical_pick_y_label,
            state.classical_pick_y_input,
            state.classical_stats_label,
            state.classical_status_label,
            state.classical_start_button,
            state.classical_stop_button,
            state.classical_close_button,
            state.classical_always_on_top_checkbox,
        ];
        SendMessageW(
            state.classical_always_on_top_checkbox,
            BM_SETCHECK,
            Some(WPARAM(if state.always_on_top {
                BST_CHECKED.0 as usize
            } else {
                0
            })),
            Some(LPARAM(0)),
        );

        state.settings_heading = create_control(
            window,
            w!("STATIC"),
            "SETTINGS",
            WINDOW_STYLE(0),
            24,
            54,
            260,
            22,
            ID_SETTINGS_HEADING,
        );
        let theme_label = create_control(
            window,
            w!("STATIC"),
            "Window color",
            WINDOW_STYLE(0),
            24,
            80,
            260,
            20,
            ID_SETTINGS_THEME_LABEL,
        );
        state.theme_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            24,
            102,
            200,
            110,
            ID_THEME_COMBO,
        );

        state.start_hotkey_label = create_control(
            window,
            w!("STATIC"),
            "Start Clicker",
            WINDOW_STYLE(0),
            24,
            140,
            100,
            20,
            ID_HOTKEY_START_LABEL,
        );
        state.start_hotkey_input = create_control(
            window,
            w!("EDIT"),
            &vk_to_string(state.start_hotkey),
            WINDOW_STYLE(ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            24,
            162,
            80,
            26,
            ID_HOTKEY_START_INPUT,
        );
        make_hotkey_input(state.start_hotkey_input);

        state.stop_hotkey_label = create_control(
            window,
            w!("STATIC"),
            "Stop Worker",
            WINDOW_STYLE(0),
            114,
            140,
            100,
            20,
            ID_HOTKEY_STOP_LABEL,
        );
        state.stop_hotkey_input = create_control(
            window,
            w!("EDIT"),
            &vk_to_string(state.stop_hotkey),
            WINDOW_STYLE(ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            114,
            162,
            80,
            26,
            ID_HOTKEY_STOP_INPUT,
        );
        make_hotkey_input(state.stop_hotkey_input);

        state.close_hotkey_label = create_control(
            window,
            w!("STATIC"),
            "Close App",
            WINDOW_STYLE(0),
            204,
            140,
            100,
            20,
            ID_HOTKEY_CLOSE_LABEL,
        );
        state.close_hotkey_input = create_control(
            window,
            w!("EDIT"),
            &vk_to_string(state.close_hotkey),
            WINDOW_STYLE(ES_AUTOHSCROLL as u32 | WS_BORDER.0),
            204,
            162,
            80,
            26,
            ID_HOTKEY_CLOSE_INPUT,
        );
        make_hotkey_input(state.close_hotkey_input);

        state.hotkey_instruction = create_control(
            window,
            w!("STATIC"),
            "Click a hotkey field and press any key to rebind.",
            WINDOW_STYLE(0),
            24,
            196,
            300,
            20,
            ID_HOTKEY_INSTRUCTION,
        );

        state.always_on_top_checkbox = create_control(
            window,
            w!("BUTTON"),
            "Always on top",
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            24,
            222,
            200,
            24,
            ID_ALWAYS_ON_TOP,
        );
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

        state.hide_when_active_checkbox = create_control(
            window,
            w!("BUTTON"),
            "Hide window when active",
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
            24,
            248,
            200,
            24,
            ID_HIDE_WHEN_ACTIVE,
        );
        SendMessageW(
            state.hide_when_active_checkbox,
            BM_SETCHECK,
            Some(WPARAM(if state.hide_when_active {
                BST_CHECKED.0 as usize
            } else {
                0
            })),
            Some(LPARAM(0)),
        );

        let settings_note = create_control(
            window,
            w!("STATIC"),
            "Changes are saved automatically.",
            WINDOW_STYLE(0),
            24,
            278,
            440,
            20,
            0,
        );

        state.settings_controls = vec![
            state.settings_heading,
            theme_label,
            state.theme_combo,
            state.start_hotkey_label,
            state.start_hotkey_input,
            state.stop_hotkey_label,
            state.stop_hotkey_input,
            state.close_hotkey_label,
            state.close_hotkey_input,
            state.hotkey_instruction,
            state.always_on_top_checkbox,
            state.hide_when_active_checkbox,
            settings_note,
        ];

        state.update_hotkey_labels();

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
            state.settings_heading,
            state.start_hotkey_label,
            state.stop_hotkey_label,
            state.close_hotkey_label,
            state.hotkey_instruction,
            state.classical_title,
            state.classical_subtitle,
            state.classical_delay_label,
            state.classical_button_label,
            state.classical_stats_label,
            state.classical_status_label,
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
        for button in [ClickButton::Left, ClickButton::Right, ClickButton::Middle] {
            let label = button.name();
            let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            SendMessageW(
                state.classical_button_combo,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(wide.as_ptr() as isize)),
            );
        }
        let cur_sel = match state.classical_button {
            ClickButton::Left => 0,
            ClickButton::Right => 1,
            ClickButton::Middle => 2,
        };
        SendMessageW(
            state.classical_button_combo,
            CB_SETCURSEL,
            Some(WPARAM(cur_sel)),
            Some(LPARAM(0)),
        );

        for click_type in [ClickType::Single, ClickType::Double] {
            let label = click_type.name();
            let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            SendMessageW(
                state.classical_click_type_combo,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(wide.as_ptr() as isize)),
            );
        }
        let type_sel = match state.classical_click_type {
            ClickType::Single => 0,
            ClickType::Double => 1,
        };
        SendMessageW(
            state.classical_click_type_combo,
            CB_SETCURSEL,
            Some(WPARAM(type_sel)),
            Some(LPARAM(0)),
        );

        SendMessageW(
            state.classical_random_checkbox,
            BM_SETCHECK,
            Some(WPARAM(if state.classical_random_offset_enabled {
                BST_CHECKED.0 as usize
            } else {
                0
            })),
            Some(LPARAM(0)),
        );
        SendMessageW(
            state.classical_repeat_stopped_radio,
            BM_SETCHECK,
            Some(WPARAM(
                if state.classical_repeat_mode == ClassicalRepeatMode::UntilStopped {
                    BST_CHECKED.0 as usize
                } else {
                    0
                },
            )),
            Some(LPARAM(0)),
        );
        SendMessageW(
            state.classical_repeat_count_radio,
            BM_SETCHECK,
            Some(WPARAM(
                if state.classical_repeat_mode == ClassicalRepeatMode::Count {
                    BST_CHECKED.0 as usize
                } else {
                    0
                },
            )),
            Some(LPARAM(0)),
        );
        SendMessageW(
            state.classical_location_current_radio,
            BM_SETCHECK,
            Some(WPARAM(
                if state.classical_location_mode == ClassicalLocationMode::Current {
                    BST_CHECKED.0 as usize
                } else {
                    0
                },
            )),
            Some(LPARAM(0)),
        );
        SendMessageW(
            state.classical_location_pick_radio,
            BM_SETCHECK,
            Some(WPARAM(
                if state.classical_location_mode == ClassicalLocationMode::Pick {
                    BST_CHECKED.0 as usize
                } else {
                    0
                },
            )),
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
        for control in &state.classical_controls {
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
