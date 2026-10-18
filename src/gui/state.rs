use super::{
    DEFAULT_IMAGE_COUNT, IMAGE_BITMAP, MAX_IMAGE_COUNT, WINDOW_TITLE, gdi::OwnedGdiObject,
    layout::layout_controls, preferences::Theme,
};
use crate::matching::{DEFAULT_MATCH_THREADS, PreparedTemplate, default_click_delay_ms};
use crate::worker::{WorkerState, run_clicker};
use image::ImageReader;
use image::imageops::FilterType;
use std::{
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread,
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT, WPARAM},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, GetDC, HBITMAP,
            HBRUSH, HFONT, InvalidateRect, ReleaseDC, UpdateWindow,
        },
        UI::{
            Controls::BST_CHECKED,
            Controls::Dialogs::{
                GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
            },
            WindowsAndMessaging::{
                BM_SETCHECK, CB_ADDSTRING, CB_RESETCONTENT, CB_SETCURSEL, GetClientRect,
                HWND_NOTOPMOST, HWND_TOPMOST, MB_ICONERROR, MB_ICONWARNING, MB_OK, MessageBoxW,
                STM_SETIMAGE, SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                SendMessageW, SetWindowPos, SetWindowTextW, ShowWindow,
            },
        },
    },
    core::PCWSTR,
};

pub(crate) struct AppState {
    pub(crate) paths: [Option<PathBuf>; MAX_IMAGE_COUNT],
    pub(crate) image_count: usize,
    pub(crate) main_controls: Vec<HWND>,
    pub(crate) settings_controls: Vec<HWND>,
    pub(crate) status_label: HWND,
    pub(crate) credit_label: HWND,
    pub(crate) slot_label: HWND,
    pub(crate) slot_combo: HWND,
    pub(crate) preview_label: HWND,
    pub(crate) preview: HWND,
    pub(crate) empty_preview: HWND,
    pub(crate) image_name: HWND,
    pub(crate) select_image_button: HWND,
    pub(crate) clear_image_button: HWND,
    pub(crate) start_button: HWND,
    pub(crate) stop_button: HWND,
    pub(crate) close_button: HWND,
    pub(crate) always_on_top_checkbox: HWND,
    pub(crate) settings_button: HWND,
    pub(crate) settings_title: HWND,
    pub(crate) main_brand_icon: HWND,
    pub(crate) main_heading: HWND,
    pub(crate) version_label: HWND,
    pub(crate) click_delay_input: HWND,
    pub(crate) theme_combo: HWND,
    pub(crate) slot_count_combo: HWND,
    pub(crate) thread_count_combo: HWND,
    pub(crate) settings_open: bool,
    pub(crate) selected_slot: usize,
    pub(crate) preview_bitmap: OwnedGdiObject<HBITMAP>,
    pub(crate) preview_source: Option<image::DynamicImage>,
    pub(crate) always_on_top: bool,
    pub(crate) click_delay_ms: u64,
    pub(crate) match_threads: usize,
    pub(crate) theme: Theme,
    pub(crate) background_brush: OwnedGdiObject<HBRUSH>,
    pub(crate) surface_brush: OwnedGdiObject<HBRUSH>,
    pub(crate) font: OwnedGdiObject<HFONT>,
    pub(crate) heading_font: OwnedGdiObject<HFONT>,
    pub(crate) dpi_scale: f32,
    pub(crate) preferences_path: Option<PathBuf>,
    pub(crate) worker: Option<Arc<WorkerState>>,
}

impl AppState {
    /// Loads saved preferences and initializes native-window state before controls are created.
    pub(crate) fn new() -> Self {
        let preferences_path = std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("LOCALAPPDATA"))
            .map(|directory| {
                PathBuf::from(directory)
                    .join("RustyClicker")
                    .join("rusty_clicker.json")
            });
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
            click_delay_input: HWND::default(),
            theme_combo: HWND::default(),
            slot_count_combo: HWND::default(),
            thread_count_combo: HWND::default(),
            settings_open: false,
            selected_slot: 0,
            preview_bitmap: OwnedGdiObject::default(),
            preview_source: None,
            always_on_top: false,
            click_delay_ms: default_click_delay_ms(),
            match_threads: DEFAULT_MATCH_THREADS,
            theme: Theme::default(),
            background_brush: OwnedGdiObject::default(),
            surface_brush: OwnedGdiObject::default(),
            font: OwnedGdiObject::default(),
            heading_font: OwnedGdiObject::default(),
            dpi_scale: 1.0,
            preferences_path,
            worker: None,
        };
        state.load_preferences();
        state
    }

    pub(crate) fn show_message(
        &self,
        text: &str,
        icon: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE,
    ) {
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        unsafe {
            MessageBoxW(None, PCWSTR(text.as_ptr()), WINDOW_TITLE, MB_OK | icon);
        }
    }

    pub(crate) fn refresh_slot_names(&self) {
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

    pub(crate) fn set_running_ui(&mut self, window: HWND, running: bool) {
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

    pub(crate) fn refresh_preview(&mut self, window: HWND) -> Result<(), String> {
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

    pub(crate) fn clear_preview_bitmap(&mut self) {
        unsafe {
            SendMessageW(
                self.preview,
                STM_SETIMAGE,
                Some(WPARAM(IMAGE_BITMAP)),
                Some(LPARAM(0)),
            );
            self.preview_bitmap = OwnedGdiObject::default();
        }
    }

    pub(crate) fn update_preview_bitmap(&mut self, window: HWND) -> Result<(), String> {
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
        let bitmap = OwnedGdiObject::new(bitmap);
        if bits.is_null() {
            return Err("Windows did not provide image preview memory.".to_owned());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
            SendMessageW(
                self.preview,
                STM_SETIMAGE,
                Some(WPARAM(IMAGE_BITMAP)),
                Some(LPARAM(bitmap.get().0 as isize)),
            );
            let _ = InvalidateRect(Some(self.preview), None, true);
            let _ = UpdateWindow(self.preview);
        }
        self.preview_bitmap = bitmap;
        Ok(())
    }

    pub(crate) fn select_slot(&mut self, window: HWND, slot: usize) {
        if slot >= self.image_count {
            self.show_message("Select a valid image slot.", MB_ICONWARNING);
            return;
        }
        self.selected_slot = slot;
        if let Err(error) = self.refresh_preview(window) {
            self.show_message(&error, MB_ICONERROR);
        }
    }

    pub(crate) fn choose_image(&mut self, window: HWND, slot: usize) {
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

    pub(crate) fn clear_selected_image(&mut self, window: HWND) {
        self.paths[self.selected_slot] = None;
        self.refresh_slot_names();
        if let Err(error) = self.refresh_preview(window) {
            self.show_message(&error, MB_ICONERROR);
        }
        self.save_preferences();
    }

    /// Loads selected targets and launches the detection worker without blocking the UI thread.
    pub(crate) fn start(&mut self, window: HWND) {
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
            match_threads: AtomicUsize::new(self.match_threads),
            scan_duration_us: AtomicU64::new(0),
            error: Mutex::new(None),
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

    pub(crate) fn stop(&mut self) {
        if let Some(worker) = &self.worker
            && worker.running.load(Ordering::Acquire)
        {
            worker.stop.store(true, Ordering::Release);
            set_text(self.status_label, "Stopping...");
        } else {
            set_text(self.status_label, "Stopped.");
        }
    }

    pub(crate) fn set_always_on_top(&mut self, window: HWND, enabled: bool) {
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

pub(crate) fn set_text(window: HWND, text: &str) {
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let _ = SetWindowTextW(window, PCWSTR(text.as_ptr()));
    }
}
