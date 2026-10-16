use crate::algorithms::{
    CLICK_DELAYS_MS, PreparedTemplate, WorkerState, default_click_delay_ms,
    normalize_click_delay_ms, run_clicker,
};
#[path = "settings_gui.rs"]
mod settings_gui;
use image::ImageReader;
use image::imageops::FilterType;
use settings_gui::{
    DEFAULT_IMAGE_COUNT, ID_SETTINGS_DELAY_LABEL, ID_SETTINGS_SLOT_COUNT_LABEL,
    ID_SETTINGS_THEME_LABEL, THEMES,
};
pub(crate) use settings_gui::{Preferences, Theme};
use std::{
    ffi::OsString,
    fs,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};
use windows::{
    Win32::{
        Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::{
            Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
            Gdi::{
                BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS, COLOR_WINDOW,
                CreateDIBSection, CreateFontW, CreatePen, CreateSolidBrush, DEFAULT_CHARSET,
                DEFAULT_PITCH, DEFAULT_QUALITY, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE,
                DT_VCENTER, DeleteObject, DrawTextW, FW_NORMAL, FillRect, GetDC, GetSysColorBrush,
                HBRUSH, HDC, HFONT, HGDIOBJ, OUT_DEFAULT_PRECIS, PS_SOLID, Polygon, ReleaseDC,
                RoundRect, ScreenToClient, SelectObject, SetBkColor, SetTextColor, UpdateWindow,
            },
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
            SetProcessDpiAwarenessContext,
        },
        UI::{
            Controls::Dialogs::{
                GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
            },
            Controls::{BST_CHECKED, DRAWITEMSTRUCT, ODS_SELECTED},
            Input::KeyboardAndMouse::{
                MOD_NOREPEAT, RegisterHotKey, ReleaseCapture, UnregisterHotKey,
            },
            WindowsAndMessaging::{
                BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, CB_ADDSTRING, CB_GETCURSEL,
                CB_RESETCONTENT, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST, CS_HREDRAW,
                CS_VREDRAW, CW_USEDEFAULT, CallWindowProcW, CreateWindowExW, DefWindowProcW,
                DestroyWindow, DispatchMessageW, GWLP_USERDATA, GWLP_WNDPROC, GetClientRect,
                GetDlgCtrlID, GetMessageW, GetParent, GetWindowLongPtrW, GetWindowTextW, HMENU,
                HTCAPTION, HWND_NOTOPMOST, HWND_TOPMOST, IDC_ARROW, LoadCursorW, LoadIconW,
                MB_ICONERROR, MB_ICONWARNING, MB_OK, MINMAXINFO, MSG, MessageBoxW, MoveWindow,
                PostMessageW, PostQuitMessage, RegisterClassW, STM_SETIMAGE, SW_HIDE, SW_SHOW,
                SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetTimer,
                SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, TranslateMessage,
                WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORBTN,
                WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DPICHANGED,
                WM_DRAWITEM, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_HOTKEY, WM_KEYDOWN,
                WM_LBUTTONDOWN, WM_NCCREATE, WM_NCHITTEST, WM_SETFONT, WM_SIZE, WM_TIMER,
                WNDCLASSW, WNDPROC, WS_CHILD, WS_CLIPCHILDREN, WS_POPUP, WS_THICKFRAME, WS_VISIBLE,
            },
        },
    },
    core::{PCWSTR, w},
};

const WINDOW_CLASS: PCWSTR = w!("RustImageClickerWindow");
const WINDOW_TITLE: PCWSTR = w!("RustyClicker");
const MAX_IMAGE_COUNT: usize = 99;
const TIMER_ID: usize = 1;
const ID_START: usize = 10;
const ID_STOP: usize = 11;
const ID_CLOSE: usize = 12;
const ID_ALWAYS_ON_TOP: usize = 13;
const ID_SETTINGS: usize = 14;
const ID_SETTINGS_BACK: usize = 15;
const ID_DELAY_COMBO: usize = 16;
const ID_THEME_COMBO: usize = 17;
const ID_SLOT_COMBO: usize = 18;
const ID_SELECT_IMAGE: usize = 19;
const ID_CLEAR_IMAGE: usize = 20;
const ID_BRAND_ICON: usize = 22;
const ID_SLOT_COUNT_COMBO: usize = 23;
const ID_SLOT_LABEL: usize = 302;
const ID_PREVIEW_LABEL: usize = 303;
const ID_STATUS_LABEL: usize = 304;
const ID_EMPTY_PREVIEW: usize = 306;
const ID_CREDIT_LABEL: usize = 307;
const ID_VERSION_LABEL: usize = 308;
const BUTTON_STYLE: u32 = windows::Win32::UI::WindowsAndMessaging::BS_OWNERDRAW as u32;
const IMAGE_BITMAP: usize = 0;

const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | (green as u32) << 8 | (blue as u32) << 16
}
struct AppState {
    paths: [Option<PathBuf>; MAX_IMAGE_COUNT],
    image_count: usize,
    main_controls: Vec<HWND>,
    settings_controls: Vec<HWND>,
    status_label: HWND,
    credit_label: HWND,
    slot_label: HWND,
    slot_combo: HWND,
    preview_label: HWND,
    preview: HWND,
    empty_preview: HWND,
    image_name: HWND,
    select_image_button: HWND,
    clear_image_button: HWND,
    start_button: HWND,
    stop_button: HWND,
    close_button: HWND,
    always_on_top_checkbox: HWND,
    settings_button: HWND,
    settings_title: HWND,
    main_brand_icon: HWND,
    main_heading: HWND,
    version_label: HWND,
    delay_combo: HWND,
    theme_combo: HWND,
    slot_count_combo: HWND,
    settings_open: bool,
    selected_slot: usize,
    preview_bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    preview_source: Option<image::DynamicImage>,
    always_on_top: bool,
    click_delay_ms: u64,
    theme: Theme,
    background_brush: HBRUSH,
    surface_brush: HBRUSH,
    font: HFONT,
    heading_font: HFONT,
    dpi_scale: f32,
    preferences_path: PathBuf,
    worker: Option<Arc<WorkerState>>,
}

impl AppState {
    /// Loads saved preferences and initializes native-window state before controls are created.
    fn new() -> Self {
        let application_dir = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));
        let preferences_path = application_dir.join("image_clicker_preferences.json");
        let mut state = Self {
            paths: std::array::from_fn(|_| None),
            image_count: DEFAULT_IMAGE_COUNT,
            main_controls: Vec::with_capacity(16),
            settings_controls: Vec::with_capacity(8),
            status_label: HWND::default(),
            credit_label: HWND::default(),
            slot_label: HWND::default(),
            slot_combo: HWND::default(),
            preview_label: HWND::default(),
            preview: HWND::default(),
            empty_preview: HWND::default(),
            image_name: HWND::default(),
            select_image_button: HWND::default(),
            clear_image_button: HWND::default(),
            start_button: HWND::default(),
            stop_button: HWND::default(),
            close_button: HWND::default(),
            always_on_top_checkbox: HWND::default(),
            settings_button: HWND::default(),
            settings_title: HWND::default(),
            main_brand_icon: HWND::default(),
            main_heading: HWND::default(),
            version_label: HWND::default(),
            delay_combo: HWND::default(),
            theme_combo: HWND::default(),
            slot_count_combo: HWND::default(),
            settings_open: false,
            selected_slot: 0,
            preview_bitmap: windows::Win32::Graphics::Gdi::HBITMAP::default(),
            preview_source: None,
            always_on_top: false,
            click_delay_ms: default_click_delay_ms(),
            theme: Theme::default(),
            background_brush: HBRUSH::default(),
            surface_brush: HBRUSH::default(),
            font: HFONT::default(),
            heading_font: HFONT::default(),
            dpi_scale: 1.0,
            preferences_path,
            worker: None,
        };
        state.load_preferences();
        state
    }

    fn load_preferences(&mut self) {
        let contents = match fs::read(&self.preferences_path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                self.show_message(
                    &format!("Could not read saved preferences: {error}"),
                    MB_ICONWARNING,
                );
                return;
            }
        };

        let preferences: Preferences = match serde_json::from_slice(&contents) {
            Ok(preferences) => preferences,
            Err(error) => {
                self.show_message(
                    &format!("Could not load saved preferences: {error}"),
                    MB_ICONWARNING,
                );
                return;
            }
        };
        self.always_on_top = preferences.always_on_top;
        self.click_delay_ms = normalize_click_delay_ms(preferences.click_delay_ms);
        self.theme = preferences.theme;
        self.image_count = preferences.image_count.clamp(1, MAX_IMAGE_COUNT);

        let mut missing_images = false;
        for (index, path) in preferences
            .image_paths
            .iter()
            .take(MAX_IMAGE_COUNT)
            .enumerate()
        {
            if let Some(path) = path {
                let path = PathBuf::from(path);
                if path.is_file() {
                    self.paths[index] = Some(path);
                } else {
                    missing_images = true;
                }
            }
        }
        if missing_images {
            self.show_message(
                "Some saved image files could not be found and were cleared.",
                MB_ICONWARNING,
            );
        }
    }

    fn save_preferences(&self) {
        let preferences = Preferences {
            image_paths: self
                .paths
                .iter()
                .map(|path| {
                    path.as_ref()
                        .map(|path| path.to_string_lossy().into_owned())
                })
                .collect(),
            always_on_top: self.always_on_top,
            click_delay_ms: self.click_delay_ms,
            theme: self.theme,
            image_count: self.image_count,
        };
        let contents = match serde_json::to_vec_pretty(&preferences) {
            Ok(contents) => contents,
            Err(error) => {
                self.show_message(
                    &format!("Could not encode preferences: {error}"),
                    MB_ICONERROR,
                );
                return;
            }
        };
        if let Err(error) = fs::write(&self.preferences_path, contents) {
            self.show_message(
                &format!(
                    "Could not save preferences to {}: {error}",
                    self.preferences_path.display()
                ),
                MB_ICONERROR,
            );
        }
    }

    fn show_message(
        &self,
        text: &str,
        icon: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE,
    ) {
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        unsafe {
            MessageBoxW(None, PCWSTR(text.as_ptr()), WINDOW_TITLE, MB_OK | icon);
        }
    }

    fn refresh_slot_names(&self) {
        unsafe {
            SendMessageW(
                self.slot_combo,
                CB_RESETCONTENT,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
            for (index, path) in self.paths.iter().take(self.image_count).enumerate() {
                let name = path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(|name| name.to_string_lossy().into_owned());
                let label = match name {
                    Some(name) => format!("Slot {:02} - {name}", index + 1),
                    None => format!("Slot {:02} - (empty)", index + 1),
                };
                let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                SendMessageW(
                    self.slot_combo,
                    CB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(wide.as_ptr() as isize)),
                );
            }
            SendMessageW(
                self.slot_combo,
                CB_SETCURSEL,
                Some(WPARAM(self.selected_slot)),
                Some(LPARAM(0)),
            );
        }
    }

    fn set_running_ui(&mut self, window: HWND, running: bool) {
        unsafe {
            let visibility = if running { SW_HIDE } else { SW_SHOW };
            let _ = ShowWindow(self.preview_label, visibility);
            let _ = ShowWindow(self.preview, visibility);
            let _ = ShowWindow(self.empty_preview, visibility);
        }
        if running {
            self.clear_preview_bitmap();
            self.preview_source = None;
        }
        layout_controls(window, self);
        if !running && let Err(error) = self.refresh_preview(window) {
            self.show_message(&error, MB_ICONERROR);
        }
    }

    fn refresh_preview(&mut self, window: HWND) -> Result<(), String> {
        self.refresh_slot_names();
        let path = self.paths[self.selected_slot].clone();
        self.clear_preview_bitmap();
        let Some(path) = path else {
            set_text(self.image_name, "");
            self.preview_source = None;
            let running = self
                .worker
                .as_ref()
                .is_some_and(|worker| worker.running.load(Ordering::Acquire));
            unsafe {
                let _ = ShowWindow(self.empty_preview, if running { SW_HIDE } else { SW_SHOW });
            }
            return Ok(());
        };

        unsafe {
            let _ = ShowWindow(self.empty_preview, SW_HIDE);
        }
        let title = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        set_text(self.image_name, &title);

        if self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.running.load(Ordering::Acquire))
        {
            self.preview_source = None;
            return Ok(());
        }

        let decoded = ImageReader::open(&path)
            .and_then(|reader| reader.decode().map_err(std::io::Error::other))
            .map_err(|error| {
                format!("Could not decode preview image {}: {error}", path.display())
            })?;
        if decoded.width() == 0 || decoded.height() == 0 {
            return Err(format!("Preview image {} is empty.", path.display()));
        }
        self.preview_source = Some(decoded);
        self.update_preview_bitmap(window)
    }

    fn clear_preview_bitmap(&mut self) {
        unsafe {
            SendMessageW(
                self.preview,
                STM_SETIMAGE,
                Some(WPARAM(IMAGE_BITMAP)),
                Some(LPARAM(0)),
            );
            if !self.preview_bitmap.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.preview_bitmap.0));
                self.preview_bitmap = windows::Win32::Graphics::Gdi::HBITMAP::default();
            }
        }
    }

    fn update_preview_bitmap(&mut self, window: HWND) -> Result<(), String> {
        let Some(source) = self.preview_source.as_ref() else {
            return Ok(());
        };
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(self.preview, &mut rect)
                .map_err(|error| format!("Could not measure the image preview: {error}"))?;
        }
        let width = (rect.right - rect.left).max(1) as u32;
        let height = (rect.bottom - rect.top).max(1) as u32;
        let preview = source
            .resize(width, height, FilterType::Triangle)
            .to_rgba8();
        let bitmap_width = i32::try_from(preview.width())
            .map_err(|_| "Preview image width exceeds the Windows limit.".to_owned())?;
        let bitmap_height = i32::try_from(preview.height())
            .map_err(|_| "Preview image height exceeds the Windows limit.".to_owned())?;
        let mut pixels = Vec::with_capacity(preview.as_raw().len());
        for pixel in preview.pixels() {
            pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
        }
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: bitmap_width,
                biHeight: -bitmap_height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = unsafe { GetDC(Some(window)) };
        if dc.0.is_null() {
            return Err("Could not create the image preview device context.".to_owned());
        }
        let mut bits = std::ptr::null_mut();
        let bitmap =
            unsafe { CreateDIBSection(Some(dc), &bitmap_info, DIB_RGB_COLORS, &mut bits, None, 0) };
        unsafe {
            ReleaseDC(Some(window), dc);
        }
        let bitmap = bitmap.map_err(|error| format!("Could not create image preview: {error}"))?;
        if bits.is_null() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            return Err("Windows did not provide image preview memory.".to_owned());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
            SendMessageW(
                self.preview,
                STM_SETIMAGE,
                Some(WPARAM(IMAGE_BITMAP)),
                Some(LPARAM(bitmap.0 as isize)),
            );
            if !self.preview_bitmap.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.preview_bitmap.0));
            }
        }
        self.preview_bitmap = bitmap;
        Ok(())
    }

    fn select_slot(&mut self, window: HWND, slot: usize) {
        if slot >= self.image_count {
            self.show_message("Select a valid image slot.", MB_ICONWARNING);
            return;
        }
        self.selected_slot = slot;
        if let Err(error) = self.refresh_preview(window) {
            self.show_message(&error, MB_ICONERROR);
        }
    }

    fn choose_image(&mut self, window: HWND, slot: usize) {
        if slot >= self.image_count {
            self.show_message("Select a valid image slot.", MB_ICONWARNING);
            return;
        }
        let mut file_buffer = [0u16; 32_768];
        const FILTER: &[u16] = &[
            b'I' as u16,
            b'm' as u16,
            b'a' as u16,
            b'g' as u16,
            b'e' as u16,
            b' ' as u16,
            b'f' as u16,
            b'i' as u16,
            b'l' as u16,
            b'e' as u16,
            b's' as u16,
            0,
            b'*' as u16,
            b'.' as u16,
            b'p' as u16,
            b'n' as u16,
            b'g' as u16,
            b';' as u16,
            b'*' as u16,
            b'.' as u16,
            b'j' as u16,
            b'p' as u16,
            b'g' as u16,
            b';' as u16,
            b'*' as u16,
            b'.' as u16,
            b'j' as u16,
            b'p' as u16,
            b'e' as u16,
            b'g' as u16,
            b';' as u16,
            b'*' as u16,
            b'.' as u16,
            b'b' as u16,
            b'm' as u16,
            b'p' as u16,
            0,
            b'A' as u16,
            b'l' as u16,
            b'l' as u16,
            b' ' as u16,
            b'f' as u16,
            b'i' as u16,
            b'l' as u16,
            b'e' as u16,
            b's' as u16,
            0,
            b'*' as u16,
            b'.' as u16,
            b'*' as u16,
            0,
            0,
        ];
        let mut dialog = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: window,
            lpstrFilter: PCWSTR(FILTER.as_ptr()),
            lpstrFile: windows::core::PWSTR(file_buffer.as_mut_ptr()),
            nMaxFile: file_buffer.len() as u32,
            Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
            ..Default::default()
        };
        let selected = unsafe { GetOpenFileNameW(&mut dialog).as_bool() };
        if !selected {
            return;
        }
        let end = file_buffer
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(file_buffer.len());
        let path = PathBuf::from(OsString::from_wide(&file_buffer[..end]));
        self.paths[slot] = Some(path);
        self.refresh_slot_names();
        if let Err(error) = self.refresh_preview(window) {
            self.show_message(&error, MB_ICONERROR);
        }
        self.save_preferences();
    }

    fn clear_selected_image(&mut self, window: HWND) {
        self.paths[self.selected_slot] = None;
        self.refresh_slot_names();
        if let Err(error) = self.refresh_preview(window) {
            self.show_message(&error, MB_ICONERROR);
        }
        self.save_preferences();
    }

    /// Loads selected targets and launches the detection worker without blocking the UI thread.
    fn start(&mut self, window: HWND) {
        if let Some(worker) = &self.worker
            && worker.running.load(Ordering::Acquire)
        {
            self.set_always_on_top(window, true);
            return;
        }

        let mut paths = Vec::new();
        for path in self.paths[..self.image_count].iter().flatten() {
            if !paths.contains(path) {
                paths.push(path.clone());
            }
        }
        if paths.is_empty() {
            self.show_message("Choose at least one image first.", MB_ICONWARNING);
            return;
        }

        let mut templates = Vec::with_capacity(paths.len());
        for path in paths {
            let template = match ImageReader::open(&path)
                .and_then(|reader| reader.decode().map_err(std::io::Error::other))
            {
                Ok(image) => match PreparedTemplate::new(image.to_rgba8()) {
                    Ok(template) => template,
                    Err(error) => {
                        self.show_message(
                            &format!("Could not prepare image {}: {error}", path.display()),
                            MB_ICONERROR,
                        );
                        return;
                    }
                },
                Err(error) => {
                    self.show_message(
                        &format!("Could not load image {}: {error}", path.display()),
                        MB_ICONERROR,
                    );
                    return;
                }
            };
            if !template.is_valid() {
                self.show_message(&format!("Image {} is empty.", path.display()), MB_ICONERROR);
                return;
            }
            templates.push(template);
        }

        self.set_always_on_top(window, true);
        let worker = Arc::new(WorkerState {
            stop: AtomicBool::new(false),
            running: AtomicBool::new(true),
            clicks: AtomicU64::new(0),
            click_delay_ms: AtomicU64::new(self.click_delay_ms),
            scan_duration_us: AtomicU64::new(0),
            message: Mutex::new(String::new()),
        });
        self.worker = Some(Arc::clone(&worker));
        set_text(self.status_label, "Running | Scan: waiting");

        let spawn_result = thread::Builder::new()
            .name("rusty-clicker".to_owned())
            .spawn(move || run_clicker(templates, worker));
        if let Err(error) = spawn_result {
            if let Some(worker) = self.worker.take() {
                worker.running.store(false, Ordering::Release);
            }
            self.show_message(
                &format!("Could not start image detection: {error}"),
                MB_ICONERROR,
            );
            set_text(self.status_label, "Stopped.");
        } else {
            self.set_running_ui(window, true);
        }
    }

    fn stop(&mut self) {
        if let Some(worker) = &self.worker
            && worker.running.load(Ordering::Acquire)
        {
            worker.stop.store(true, Ordering::Release);
            set_text(self.status_label, "Stopping...");
        } else {
            set_text(self.status_label, "Stopped.");
        }
    }

    fn set_always_on_top(&mut self, window: HWND, enabled: bool) {
        self.always_on_top = enabled;
        unsafe {
            let _ = SetWindowPos(
                window,
                if enabled {
                    Some(HWND_TOPMOST)
                } else {
                    Some(HWND_NOTOPMOST)
                },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        if !self.always_on_top_checkbox.0.is_null() {
            unsafe {
                SendMessageW(
                    self.always_on_top_checkbox,
                    BM_SETCHECK,
                    Some(WPARAM(if enabled { BST_CHECKED.0 as usize } else { 0 })),
                    Some(LPARAM(0)),
                );
            }
        }
        self.save_preferences();
    }
}

fn set_text(window: HWND, text: &str) {
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let _ = SetWindowTextW(window, PCWSTR(text.as_ptr()));
    }
}

fn make_static_draggable(control: HWND) -> bool {
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

unsafe extern "system" fn static_control_proc(
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

unsafe fn create_control(
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

unsafe fn create_controls(window: HWND, state: &mut AppState) {
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
            concat!(
                "v",
                env!("CARGO_PKG_VERSION_MAJOR"),
                ".",
                env!("CARGO_PKG_VERSION_MINOR")
            ),
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
        state.main_controls = vec![
            state.slot_label,
            state.slot_combo,
            state.preview_label,
            state.preview,
            state.empty_preview,
            state.image_name,
            state.select_image_button,
            state.clear_image_button,
            state.status_label,
            state.credit_label,
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
            "Click delay",
            WINDOW_STYLE(0),
            24,
            100,
            170,
            28,
            ID_SETTINGS_DELAY_LABEL,
        );
        state.delay_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            210,
            94,
            170,
            220,
            ID_DELAY_COMBO,
        );
        let theme_label = create_control(
            window,
            w!("STATIC"),
            "Window color",
            WINDOW_STYLE(0),
            24,
            152,
            170,
            28,
            ID_SETTINGS_THEME_LABEL,
        );
        state.theme_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            210,
            146,
            170,
            180,
            ID_THEME_COMBO,
        );
        let slot_count_label = create_control(
            window,
            w!("STATIC"),
            "Number of slots",
            WINDOW_STYLE(0),
            24,
            202,
            170,
            28,
            ID_SETTINGS_SLOT_COUNT_LABEL,
        );
        state.slot_count_combo = create_control(
            window,
            w!("COMBOBOX"),
            "",
            WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
            210,
            196,
            170,
            220,
            ID_SLOT_COUNT_COMBO,
        );
        let settings_note = create_control(
            window,
            w!("STATIC"),
            "Changes are saved automatically.",
            WINDOW_STYLE(0),
            24,
            248,
            440,
            26,
            0,
        );
        let settings_back = create_control(
            window,
            w!("BUTTON"),
            "Back to images",
            WINDOW_STYLE(BUTTON_STYLE),
            24,
            292,
            160,
            32,
            ID_SETTINGS_BACK,
        );
        state.settings_controls = vec![
            state.settings_title,
            state.delay_combo,
            theme_label,
            state.theme_combo,
            slot_count_label,
            state.slot_count_combo,
            settings_note,
            settings_back,
        ];
        let mut drag_setup_failed = false;
        for control in [
            state.main_brand_icon,
            state.main_heading,
            state.slot_label,
            state.preview_label,
            state.preview,
            state.empty_preview,
            state.image_name,
            state.status_label,
            state.settings_title,
            theme_label,
            slot_count_label,
            settings_note,
        ] {
            drag_setup_failed |= !make_static_draggable(control);
        }
        if drag_setup_failed {
            state.show_message("Some labels could not be made draggable.", MB_ICONWARNING);
        }
        for (index, delay) in CLICK_DELAYS_MS.iter().enumerate() {
            let label = format!("{delay} ms");
            let wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            SendMessageW(
                state.delay_combo,
                CB_ADDSTRING,
                Some(WPARAM(0)),
                Some(LPARAM(wide.as_ptr() as isize)),
            );
            if *delay == state.click_delay_ms {
                SendMessageW(
                    state.delay_combo,
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
        if state.background_brush.0.is_null() || state.surface_brush.0.is_null() {
            state.show_message("Could not create the window background.", MB_ICONERROR);
        }
        for control in &state.settings_controls {
            let _ = ShowWindow(*control, SW_HIDE);
        }
        if let Err(error) = state.refresh_preview(window) {
            state.show_message(&error, MB_ICONERROR);
        }
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

pub(crate) fn scale(value: i32, factor: f32) -> i32 {
    (value as f32 * factor).round().max(1.0) as i32
}

fn update_ui_font(state: &mut AppState, dpi: u32) {
    let font = unsafe {
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
    };
    if font.0.is_null() {
        state.show_message("Could not create the application font.", MB_ICONERROR);
        return;
    }
    let heading_font = unsafe {
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
    };
    if heading_font.0.is_null() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
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
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        for control in [state.main_heading, state.settings_title] {
            SendMessageW(
                control,
                WM_SETFONT,
                Some(WPARAM(heading_font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        if !state.font.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(state.font.0));
        }
        if !state.heading_font.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(state.heading_font.0));
        }
    }
    state.font = font;
    state.heading_font = heading_font;
}

fn move_control(control: HWND, x: i32, y: i32, width: i32, height: i32) {
    if !control.0.is_null() {
        unsafe {
            let _ = MoveWindow(control, x, y, width.max(1), height.max(1), true);
        }
    }
}

unsafe fn draw_button(state: &AppState, item: &DRAWITEMSTRUCT) {
    let selected = item.itemState.0 & ODS_SELECTED.0 != 0;
    let (fill, text, border) = state
        .theme
        .button_colors(usize::try_from(item.CtlID).unwrap_or_default(), selected);
    let fill_brush = unsafe { CreateSolidBrush(COLORREF(fill)) };
    let border_pen = unsafe { CreatePen(PS_SOLID, scale(1, state.dpi_scale), COLORREF(border)) };
    if fill_brush.0.is_null() || border_pen.0.is_null() {
        if !fill_brush.0.is_null() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(fill_brush.0));
            }
        }
        if !border_pen.0.is_null() {
            unsafe {
                let _ = DeleteObject(HGDIOBJ(border_pen.0));
            }
        }
        return;
    }

    unsafe {
        let old_brush = SelectObject(item.hDC, HGDIOBJ(fill_brush.0));
        let old_pen = SelectObject(item.hDC, HGDIOBJ(border_pen.0));
        let _ = RoundRect(
            item.hDC,
            item.rcItem.left + 1,
            item.rcItem.top + 1,
            item.rcItem.right - 1,
            item.rcItem.bottom - 1,
            scale(12, state.dpi_scale),
            scale(12, state.dpi_scale),
        );
        SelectObject(item.hDC, old_brush);
        SelectObject(item.hDC, old_pen);
        let mut text_rect = item.rcItem;
        text_rect.left += 8;
        text_rect.right -= 8;
        SetBkColor(item.hDC, COLORREF(fill));
        SetTextColor(item.hDC, COLORREF(text));
        let mut label = [0u16; 256];
        let length = GetWindowTextW(item.hwndItem, &mut label);
        if length > 0 {
            DrawTextW(
                item.hDC,
                &mut label[..length as usize],
                &mut text_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
        let _ = DeleteObject(HGDIOBJ(fill_brush.0));
        let _ = DeleteObject(HGDIOBJ(border_pen.0));
    }
}

unsafe fn draw_cursor_icon(state: &AppState, item: &DRAWITEMSTRUCT) {
    let x = item.rcItem.left;
    let y = item.rcItem.top;
    let width = item.rcItem.right - x;
    let height = item.rcItem.bottom - y;
    let outline = unsafe { CreateSolidBrush(COLORREF(rgb(52, 35, 24))) };
    let rust = unsafe { CreateSolidBrush(COLORREF(rgb(173, 83, 43))) };
    let outline_pen = unsafe {
        CreatePen(
            PS_SOLID,
            scale(1, state.dpi_scale),
            COLORREF(rgb(37, 27, 21)),
        )
    };
    if outline.0.is_null() || rust.0.is_null() || outline_pen.0.is_null() {
        for object in [HGDIOBJ(outline.0), HGDIOBJ(rust.0), HGDIOBJ(outline_pen.0)] {
            if !object.0.is_null() {
                unsafe {
                    let _ = DeleteObject(object);
                }
            }
        }
        return;
    }

    unsafe {
        if !state.background_brush.0.is_null() {
            let _ = FillRect(item.hDC, &item.rcItem, state.background_brush);
        }
        let old_brush = SelectObject(item.hDC, HGDIOBJ(outline.0));
        let old_pen = SelectObject(item.hDC, HGDIOBJ(outline_pen.0));
        let outer = [
            POINT { x: x + 3, y: y + 2 },
            POINT {
                x: x + 3,
                y: y + height - 2,
            },
            POINT {
                x: x + width / 3,
                y: y + height * 2 / 3,
            },
            POINT {
                x: x + width * 2 / 3,
                y: y + height - 3,
            },
            POINT {
                x: x + width * 4 / 5,
                y: y + height * 4 / 5,
            },
            POINT {
                x: x + width / 2,
                y: y + height * 3 / 5,
            },
            POINT { x: x + 3, y: y + 2 },
        ];
        let _ = Polygon(item.hDC, &outer);
        let inner = [
            POINT { x: x + 6, y: y + 7 },
            POINT {
                x: x + 6,
                y: y + height - 8,
            },
            POINT {
                x: x + width / 3,
                y: y + height * 2 / 3,
            },
            POINT {
                x: x + width * 2 / 3 - 3,
                y: y + height - 7,
            },
            POINT {
                x: x + width / 2,
                y: y + height * 3 / 5,
            },
            POINT { x: x + 6, y: y + 7 },
        ];
        SelectObject(item.hDC, HGDIOBJ(rust.0));
        let _ = Polygon(item.hDC, &inner);
        SelectObject(item.hDC, old_brush);
        SelectObject(item.hDC, old_pen);
        let _ = DeleteObject(HGDIOBJ(outline.0));
        let _ = DeleteObject(HGDIOBJ(rust.0));
        let _ = DeleteObject(HGDIOBJ(outline_pen.0));
    }
}

unsafe fn draw_empty_preview(state: &AppState, item: &DRAWITEMSTRUCT) {
    let (_, text_color) = state.theme.colors();
    let mut text: Vec<u16> = "No image selected".encode_utf16().collect();
    let mut text_rect = item.rcItem;
    unsafe {
        let _ = FillRect(item.hDC, &item.rcItem, state.background_brush);
        SetBkColor(item.hDC, state.theme.colors().0);
        SetTextColor(item.hDC, text_color);
        DrawTextW(
            item.hDC,
            &mut text,
            &mut text_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

/// Positions both main and settings controls for the current client size and DPI.
fn layout_controls(window: HWND, state: &AppState) {
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
            scale(70, s),
            scale(184, s),
            scale(30, s),
        );
        move_control(
            state.delay_combo,
            margin,
            scale(104, s),
            scale(200, s),
            scale(230, s),
        );
        let theme_label = state.settings_controls.get(2).copied().unwrap_or_default();
        move_control(
            theme_label,
            margin,
            scale(152, s),
            scale(184, s),
            scale(24, s),
        );
        move_control(
            state.theme_combo,
            margin,
            scale(178, s),
            scale(200, s),
            scale(180, s),
        );
        if let Some(label) = state.settings_controls.get(4) {
            move_control(*label, margin, scale(218, s), scale(184, s), scale(24, s));
        }
        move_control(
            state.slot_count_combo,
            margin,
            scale(244, s),
            scale(200, s),
            scale(220, s),
        );
        if let Some(note) = state.settings_controls.get(6) {
            move_control(*note, margin, scale(286, s), content_width, scale(24, s));
        }
        if let Some(back) = state.settings_controls.get(7) {
            move_control(
                *back,
                margin,
                height - scale(48, s),
                scale(148, s),
                scale(36, s),
            );
        }
    }
}

/// Routes Win32 window messages to app actions, hotkeys, drawing, and worker status updates.
unsafe extern "system" fn window_proc(
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
        if message == WM_ERASEBKGND && !state.background_brush.0.is_null() {
            let mut client = RECT::default();
            if GetClientRect(window, &mut client).is_ok() {
                FillRect(HDC(wparam.0 as *mut _), &client, state.background_brush);
            }
            return LRESULT(1);
        }
        if message == WM_CTLCOLORSTATIC && !state.background_brush.0.is_null() {
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
            return LRESULT(state.background_brush.0 as isize);
        }
        if message == WM_CTLCOLORBTN && !state.background_brush.0.is_null() {
            let (background, text) = state.theme.colors();
            SetBkColor(HDC(wparam.0 as *mut _), background);
            SetTextColor(HDC(wparam.0 as *mut _), text);
            return LRESULT(state.background_brush.0 as isize);
        }
        if (message == WM_CTLCOLOREDIT || message == WM_CTLCOLORLISTBOX)
            && !state.surface_brush.0.is_null()
        {
            let (_, text) = state.theme.colors();
            SetBkColor(
                HDC(wparam.0 as *mut _),
                COLORREF(state.theme.surface_color()),
            );
            SetTextColor(HDC(wparam.0 as *mut _), text);
            return LRESULT(state.surface_brush.0 as isize);
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
                } else if id == ID_SETTINGS_BACK {
                    state.set_settings_open(window, false);
                } else if id == ID_ALWAYS_ON_TOP {
                    let checked = SendMessageW(
                        state.always_on_top_checkbox,
                        BM_GETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0 == BST_CHECKED.0 as isize;
                    state.set_always_on_top(window, checked);
                } else if id == ID_DELAY_COMBO && notification == CBN_SELCHANGE as usize {
                    let selection = SendMessageW(
                        state.delay_combo,
                        CB_GETCURSEL,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0;
                    if let Ok(selection) = usize::try_from(selection) {
                        state.set_click_delay(selection);
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
                        let message = worker
                            .message
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        let status = if message.is_empty() {
                            format!("Stopped | {clicks} clicks | Scan: {scan_ms:.2} ms")
                        } else {
                            message
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
                if !state.background_brush.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(state.background_brush.0));
                    state.background_brush = HBRUSH::default();
                }
                if !state.surface_brush.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(state.surface_brush.0));
                    state.surface_brush = HBRUSH::default();
                }
                if !state.font.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(state.font.0));
                    state.font = HFONT::default();
                }
                if !state.heading_font.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(state.heading_font.0));
                    state.heading_font = HFONT::default();
                }
                if !state.preview_bitmap.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(state.preview_bitmap.0));
                    state.preview_bitmap = windows::Win32::Graphics::Gdi::HBITMAP::default();
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

/// Registers the window class and runs the native UI message loop.
pub(super) fn run() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
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
            lpfnWndProc: Some(window_proc),
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
            scale(450, initial_scale),
            scale(470, initial_scale),
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

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
