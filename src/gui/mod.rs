pub(crate) mod controls;
pub(crate) mod drawing;
pub(crate) mod gdi;
pub(crate) mod layout;
pub(crate) mod logic_gui;
pub(crate) mod preferences;
pub(crate) mod settings;
pub(crate) mod state;
pub(crate) mod wndproc;

#[cfg(test)]
pub(crate) use layout::scale;
#[cfg(test)]
pub(crate) use preferences::{Preferences, Theme};
pub(crate) use state::AppState;

use settings::DEFAULT_IMAGE_COUNT;
use windows::{
    Win32::{
        Foundation::HINSTANCE,
        Graphics::Gdi::{COLOR_WINDOW, GetSysColorBrush, UpdateWindow},
        System::LibraryLoader::GetModuleHandleW,
        UI::HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem,
            SetProcessDpiAwarenessContext,
        },
        UI::WindowsAndMessaging::{
            CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DispatchMessageW, GetMessageW,
            IDC_ARROW, LoadCursorW, LoadIconW, MB_ICONERROR, MB_OK, MSG, MessageBoxW,
            RegisterClassW, SW_SHOW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WNDCLASSW,
            WS_CLIPCHILDREN, WS_POPUP, WS_THICKFRAME, WS_VISIBLE,
        },
    },
    core::{PCWSTR, w},
};

pub(crate) const WINDOW_CLASS: PCWSTR = w!("RustImageClickerWindow");
pub(crate) const WINDOW_TITLE: PCWSTR = w!("RustyClicker");
pub(crate) const MAX_IMAGE_COUNT: usize = 99;
pub(crate) const TIMER_ID: usize = 1;
pub(crate) const ID_START: usize = 10;
pub(crate) const ID_STOP: usize = 11;
pub(crate) const ID_CLOSE: usize = 12;
pub(crate) const ID_ALWAYS_ON_TOP: usize = 13;
pub(crate) const ID_SETTINGS: usize = 14;
pub(crate) const ID_DELAY_INPUT: usize = 16;
pub(crate) const ID_THEME_COMBO: usize = 17;
pub(crate) const ID_SLOT_COMBO: usize = 18;
pub(crate) const ID_SELECT_IMAGE: usize = 19;
pub(crate) const ID_CLEAR_IMAGE: usize = 20;
pub(crate) const ID_BRAND_ICON: usize = 22;
pub(crate) const ID_SLOT_COUNT_COMBO: usize = 23;
pub(crate) const ID_THREAD_COUNT_COMBO: usize = 24;
pub(crate) const ID_CLICKER_BUTTON: usize = 25;
pub(crate) const ID_SLOT_LABEL: usize = 302;
pub(crate) const ID_PREVIEW_LABEL: usize = 303;
pub(crate) const ID_STATUS_LABEL: usize = 304;
pub(crate) const ID_EMPTY_PREVIEW: usize = 306;
pub(crate) const ID_CREDIT_LABEL: usize = 307;
pub(crate) const ID_VERSION_LABEL: usize = 308;
pub(crate) const ID_MENU_STATUS_TITLE: usize = 311;
pub(crate) const ID_MENU_SUMMARY_LABEL: usize = 312;
pub(crate) const ID_MENU_HOTKEY_LABEL: usize = 313;
pub(crate) const ID_MENU_TIPS_LABEL: usize = 314;
pub(crate) const BUTTON_STYLE: u32 = windows::Win32::UI::WindowsAndMessaging::BS_OWNERDRAW as u32;
pub(crate) const IMAGE_BITMAP: usize = 0;

/// Registers the window class and runs the native UI message loop.
pub(crate) fn run() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let icc = windows::Win32::UI::Controls::INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<windows::Win32::UI::Controls::INITCOMMONCONTROLSEX>()
                as u32,
            dwICC: windows::Win32::UI::Controls::ICC_TREEVIEW_CLASSES,
        };
        let _ = windows::Win32::UI::Controls::InitCommonControlsEx(&icc);
        let instance = match GetModuleHandleW(None) {
            Ok(module) => HINSTANCE(module.0),
            Err(error) => {
                MessageBoxW(
                    None,
                    PCWSTR(
                        error
                            .to_string()
                            .encode_utf16()
                            .chain(Some(0))
                            .collect::<Vec<_>>()
                            .as_ptr(),
                    ),
                    WINDOW_TITLE,
                    MB_OK | MB_ICONERROR,
                );
                return;
            }
        };
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        let icon = match LoadIconW(Some(instance), PCWSTR(1usize as *const u16)) {
            Ok(icon) => icon,
            Err(error) => {
                let text: Vec<u16> = error.to_string().encode_utf16().chain(Some(0)).collect();
                MessageBoxW(
                    None,
                    PCWSTR(text.as_ptr()),
                    WINDOW_TITLE,
                    MB_OK | MB_ICONERROR,
                );
                return;
            }
        };
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc::window_proc),
            hInstance: instance,
            hCursor: cursor,
            hIcon: icon,
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpszClassName: WINDOW_CLASS,
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            MessageBoxW(
                None,
                w!("Could not register the application window."),
                WINDOW_TITLE,
                MB_OK | MB_ICONERROR,
            );
            return;
        }

        let mut state = Box::new(AppState::new());
        let initial_dpi = GetDpiForSystem().max(96);
        let initial_scale = initial_dpi as f32 / 96.0;
        let window = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            WINDOW_CLASS,
            WINDOW_TITLE,
            WS_POPUP | WS_THICKFRAME | WS_CLIPCHILDREN | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            layout::scale(450, initial_scale),
            layout::scale(470, initial_scale),
            None,
            None,
            Some(instance),
            Some((&mut *state as *mut AppState).cast()),
        ) {
            Ok(window) => window,
            Err(error) => {
                let text: Vec<u16> = error.to_string().encode_utf16().chain(Some(0)).collect();
                MessageBoxW(
                    None,
                    PCWSTR(text.as_ptr()),
                    WINDOW_TITLE,
                    MB_OK | MB_ICONERROR,
                );
                return;
            }
        };
        let _ = ShowWindow(window, SW_SHOW);
        let _ = UpdateWindow(window);
        if let Err(error) = state.refresh_preview(window) {
            state.show_message(&error, MB_ICONERROR);
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
