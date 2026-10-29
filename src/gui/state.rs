use super::{
    DEFAULT_IMAGE_COUNT, IMAGE_BITMAP, MAX_IMAGE_COUNT, WINDOW_TITLE, gdi::OwnedGdiObject,
    layout::layout_controls, preferences::Theme,
};
use crate::classical_worker::{ClassicalWorkerConfig, ClassicalWorkerState, run_classical_clicker};
use crate::gui::preferences::{
    ClassicalLocationMode, ClassicalRepeatMode, DEFAULT_CLASSICAL_DELAY_MS, DEFAULT_CLOSE_HOTKEY,
    DEFAULT_START_HOTKEY, DEFAULT_STOP_HOTKEY, vk_to_string,
};
use crate::input::{ClickButton, ClickType};
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ActivePanel {
    MainMenu,
    Classical,
    Clicker,
    Logic,
    Settings,
}

pub(crate) struct AppState {
    pub(crate) active_panel: ActivePanel,
    pub(crate) paths: [Option<PathBuf>; MAX_IMAGE_COUNT],
    pub(crate) image_count: usize,
    pub(crate) clicker_controls: Vec<HWND>,
    pub(crate) menu_controls: Vec<HWND>,
    pub(crate) settings_controls: Vec<HWND>,
    pub(crate) classical_controls: Vec<HWND>,
    pub(crate) clicker_button: HWND,
    pub(crate) classical_nav_button: HWND,
    pub(crate) images_button: HWND,
    pub(crate) classical_title: HWND,
    pub(crate) classical_subtitle: HWND,
    pub(crate) classical_delay_label: HWND,
    pub(crate) classical_delay_input: HWND,
    pub(crate) classical_hours_input: HWND,
    pub(crate) classical_mins_input: HWND,
    pub(crate) classical_secs_input: HWND,
    pub(crate) classical_millis_input: HWND,
    pub(crate) classical_hours_label: HWND,
    pub(crate) classical_mins_label: HWND,
    pub(crate) classical_secs_label: HWND,
    pub(crate) classical_millis_label: HWND,
    pub(crate) classical_random_checkbox: HWND,
    pub(crate) classical_random_input: HWND,
    pub(crate) classical_random_label: HWND,
    pub(crate) classical_options_title: HWND,
    pub(crate) classical_button_label: HWND,
    pub(crate) classical_button_combo: HWND,
    pub(crate) classical_click_type_label: HWND,
    pub(crate) classical_click_type_combo: HWND,
    pub(crate) classical_repeat_title: HWND,
    pub(crate) classical_repeat_stopped_radio: HWND,
    pub(crate) classical_repeat_count_radio: HWND,
    pub(crate) classical_repeat_count_input: HWND,
    pub(crate) classical_repeat_count_label: HWND,
    pub(crate) classical_location_title: HWND,
    pub(crate) classical_location_current_radio: HWND,
    pub(crate) classical_location_pick_radio: HWND,
    pub(crate) classical_pick_button: HWND,
    pub(crate) classical_pick_x_label: HWND,
    pub(crate) classical_pick_x_input: HWND,
    pub(crate) classical_pick_y_label: HWND,
    pub(crate) classical_pick_y_input: HWND,
    pub(crate) classical_status_label: HWND,
    pub(crate) classical_stats_label: HWND,
    pub(crate) classical_start_button: HWND,
    pub(crate) classical_stop_button: HWND,
    pub(crate) classical_close_button: HWND,
    pub(crate) classical_always_on_top_checkbox: HWND,
    pub(crate) classical_delay_ms: u64,
    pub(crate) classical_hours: u32,
    pub(crate) classical_mins: u32,
    pub(crate) classical_secs: u32,
    pub(crate) classical_millis: u32,
    pub(crate) classical_random_offset_enabled: bool,
    pub(crate) classical_random_offset_ms: u32,
    pub(crate) classical_click_type: ClickType,
    pub(crate) classical_repeat_mode: ClassicalRepeatMode,
    pub(crate) classical_repeat_count: u64,
    pub(crate) classical_location_mode: ClassicalLocationMode,
    pub(crate) classical_pick_x: i32,
    pub(crate) classical_pick_y: i32,
    pub(crate) is_picking_location: bool,
    pub(crate) pick_released: bool,
    pub(crate) classical_button: ClickButton,
    pub(crate) classical_worker: Option<Arc<ClassicalWorkerState>>,
    pub(crate) menu_status_title: HWND,
    pub(crate) menu_summary_label: HWND,
    pub(crate) menu_hotkey_label: HWND,
    pub(crate) menu_tips_label: HWND,
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
    pub(crate) slot_count_input: HWND,
    pub(crate) thread_count_combo: HWND,
    pub(crate) settings_open: bool,
    pub(crate) start_hotkey: u32,
    pub(crate) stop_hotkey: u32,
    pub(crate) close_hotkey: u32,
    pub(crate) hide_when_active: bool,
    pub(crate) start_hotkey_input: HWND,
    pub(crate) stop_hotkey_input: HWND,
    pub(crate) close_hotkey_input: HWND,
    pub(crate) hide_when_active_checkbox: HWND,
    pub(crate) settings_heading: HWND,
    pub(crate) start_hotkey_label: HWND,
    pub(crate) stop_hotkey_label: HWND,
    pub(crate) close_hotkey_label: HWND,
    pub(crate) hotkey_instruction: HWND,
    pub(crate) logic_controls: Vec<HWND>,
    pub(crate) logic_button: HWND,
    pub(crate) logic_tree_title: HWND,
    pub(crate) logic_tree_view: HWND,
    pub(crate) logic_add_click_btn: HWND,
    pub(crate) logic_add_repeat_btn: HWND,
    pub(crate) logic_add_ifelse_btn: HWND,
    pub(crate) logic_delete_btn: HWND,
    pub(crate) logic_props_title: HWND,
    pub(crate) logic_node_type_label: HWND,
    pub(crate) logic_prop1_label: HWND,
    pub(crate) logic_prop1_edit: HWND,
    pub(crate) logic_prop2_label: HWND,
    pub(crate) logic_prop2_edit: HWND,
    pub(crate) logic_prop3_label: HWND,
    pub(crate) logic_prop3_edit: HWND,
    pub(crate) logic_apply_btn: HWND,
    pub(crate) logic_save_btn: HWND,
    pub(crate) logic_load_btn: HWND,
    pub(crate) logic_status_label: HWND,
    pub(crate) logic_open: bool,
    pub(crate) logic_selected_node_id: Option<usize>,
    pub(crate) logic_state: super::logic_gui::LogicState,
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
            active_panel: ActivePanel::MainMenu,
            paths: std::array::from_fn(|_| None),
            image_count: DEFAULT_IMAGE_COUNT,
            clicker_controls: Vec::with_capacity(16),
            menu_controls: Vec::with_capacity(12),
            settings_controls: Vec::with_capacity(9),
            classical_controls: Vec::with_capacity(12),
            clicker_button: HWND::default(),
            classical_nav_button: HWND::default(),
            images_button: HWND::default(),
            classical_title: HWND::default(),
            classical_subtitle: HWND::default(),
            classical_delay_label: HWND::default(),
            classical_delay_input: HWND::default(),
            classical_hours_input: HWND::default(),
            classical_mins_input: HWND::default(),
            classical_secs_input: HWND::default(),
            classical_millis_input: HWND::default(),
            classical_hours_label: HWND::default(),
            classical_mins_label: HWND::default(),
            classical_secs_label: HWND::default(),
            classical_millis_label: HWND::default(),
            classical_random_checkbox: HWND::default(),
            classical_random_input: HWND::default(),
            classical_random_label: HWND::default(),
            classical_options_title: HWND::default(),
            classical_button_label: HWND::default(),
            classical_button_combo: HWND::default(),
            classical_click_type_label: HWND::default(),
            classical_click_type_combo: HWND::default(),
            classical_repeat_title: HWND::default(),
            classical_repeat_stopped_radio: HWND::default(),
            classical_repeat_count_radio: HWND::default(),
            classical_repeat_count_input: HWND::default(),
            classical_repeat_count_label: HWND::default(),
            classical_location_title: HWND::default(),
            classical_location_current_radio: HWND::default(),
            classical_location_pick_radio: HWND::default(),
            classical_pick_button: HWND::default(),
            classical_pick_x_label: HWND::default(),
            classical_pick_x_input: HWND::default(),
            classical_pick_y_label: HWND::default(),
            classical_pick_y_input: HWND::default(),
            classical_status_label: HWND::default(),
            classical_stats_label: HWND::default(),
            classical_start_button: HWND::default(),
            classical_stop_button: HWND::default(),
            classical_close_button: HWND::default(),
            classical_always_on_top_checkbox: HWND::default(),
            classical_delay_ms: DEFAULT_CLASSICAL_DELAY_MS,
            classical_hours: 0,
            classical_mins: 0,
            classical_secs: 0,
            classical_millis: 100,
            classical_random_offset_enabled: false,
            classical_random_offset_ms: 40,
            classical_click_type: ClickType::default(),
            classical_repeat_mode: ClassicalRepeatMode::default(),
            classical_repeat_count: 1,
            classical_location_mode: ClassicalLocationMode::default(),
            classical_pick_x: 0,
            classical_pick_y: 0,
            is_picking_location: false,
            pick_released: false,
            classical_button: ClickButton::default(),
            classical_worker: None,
            menu_status_title: HWND::default(),
            menu_summary_label: HWND::default(),
            menu_hotkey_label: HWND::default(),
            menu_tips_label: HWND::default(),
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
            slot_count_input: HWND::default(),
            thread_count_combo: HWND::default(),
            settings_open: false,
            start_hotkey: DEFAULT_START_HOTKEY,
            stop_hotkey: DEFAULT_STOP_HOTKEY,
            close_hotkey: DEFAULT_CLOSE_HOTKEY,
            hide_when_active: false,
            start_hotkey_input: HWND::default(),
            stop_hotkey_input: HWND::default(),
            close_hotkey_input: HWND::default(),
            hide_when_active_checkbox: HWND::default(),
            settings_heading: HWND::default(),
            start_hotkey_label: HWND::default(),
            stop_hotkey_label: HWND::default(),
            close_hotkey_label: HWND::default(),
            hotkey_instruction: HWND::default(),
            logic_controls: Vec::with_capacity(20),
            logic_button: HWND::default(),
            logic_tree_title: HWND::default(),
            logic_tree_view: HWND::default(),
            logic_add_click_btn: HWND::default(),
            logic_add_repeat_btn: HWND::default(),
            logic_add_ifelse_btn: HWND::default(),
            logic_delete_btn: HWND::default(),
            logic_props_title: HWND::default(),
            logic_node_type_label: HWND::default(),
            logic_prop1_label: HWND::default(),
            logic_prop1_edit: HWND::default(),
            logic_prop2_label: HWND::default(),
            logic_prop2_edit: HWND::default(),
            logic_prop3_label: HWND::default(),
            logic_prop3_edit: HWND::default(),
            logic_apply_btn: HWND::default(),
            logic_save_btn: HWND::default(),
            logic_load_btn: HWND::default(),
            logic_status_label: HWND::default(),
            logic_open: false,
            logic_selected_node_id: None,
            logic_state: super::logic_gui::LogicState::default(),
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

    pub(crate) fn set_panel(&mut self, window: HWND, panel: ActivePanel) {
        self.active_panel = panel;
        self.settings_open = panel == ActivePanel::Settings;
        self.logic_open = panel == ActivePanel::Logic;

        let show_menu = if panel == ActivePanel::MainMenu {
            SW_SHOW
        } else {
            SW_HIDE
        };
        let show_classical = if panel == ActivePanel::Classical {
            SW_SHOW
        } else {
            SW_HIDE
        };
        let show_clicker = if panel == ActivePanel::Clicker {
            SW_SHOW
        } else {
            SW_HIDE
        };
        let show_logic = if panel == ActivePanel::Logic {
            SW_SHOW
        } else {
            SW_HIDE
        };
        let show_settings = if panel == ActivePanel::Settings {
            SW_SHOW
        } else {
            SW_HIDE
        };

        unsafe {
            for control in &self.menu_controls {
                let _ = ShowWindow(*control, show_menu);
            }
            for control in &self.classical_controls {
                let _ = ShowWindow(*control, show_classical);
            }
            for control in &self.clicker_controls {
                let _ = ShowWindow(*control, show_clicker);
            }
            for control in &self.logic_controls {
                let _ = ShowWindow(*control, show_logic);
            }
            for control in &self.settings_controls {
                let _ = ShowWindow(*control, show_settings);
            }
            let _ = ShowWindow(self.settings_button, SW_SHOW);
        }

        set_text(
            self.main_heading,
            match panel {
                ActivePanel::MainMenu => "RustyClicker",
                ActivePanel::Classical => "Classical Clicker",
                ActivePanel::Clicker => "Images Clicker",
                ActivePanel::Logic => "Logic Builder",
                ActivePanel::Settings => "Settings",
            },
        );
        set_text(
            self.settings_button,
            if panel == ActivePanel::MainMenu {
                "⚙"
            } else {
                "← Back"
            },
        );

        self.update_menu_summary();
        let start_str = vk_to_string(self.start_hotkey);
        let stop_str = vk_to_string(self.stop_hotkey);

        if panel == ActivePanel::MainMenu {
            set_text(
                self.menu_tips_label,
                &format!("Tip: Open Classical or Images, then press {start_str} to start."),
            );
        } else if panel == ActivePanel::Classical {
            let classical_running = self
                .classical_worker
                .as_ref()
                .is_some_and(|w| w.is_running());
            if !classical_running {
                set_text(
                    self.classical_status_label,
                    &format!("Ready  |  Press {start_str} to start, {stop_str} to stop."),
                );
            }
        } else if panel == ActivePanel::Clicker {
            let images_running = self
                .worker
                .as_ref()
                .is_some_and(|w| w.running.load(Ordering::Acquire));
            if !images_running {
                let has_images = self.paths[..self.image_count].iter().any(|p| p.is_some());
                if has_images {
                    set_text(
                        self.status_label,
                        &format!("Ready  |  Press {start_str} to start, {stop_str} to stop."),
                    );
                } else {
                    set_text(
                        self.status_label,
                        &format!("Ready  |  Select target image, then press {start_str} to start."),
                    );
                }
            }
        }

        layout_controls(window, self);

        if panel == ActivePanel::Clicker {
            if let Err(error) = self.refresh_preview(window) {
                self.show_message(&error, MB_ICONERROR);
            }
        } else if panel == ActivePanel::Logic {
            super::logic_gui::populate_treeview(self.logic_tree_view, &self.logic_state);
        }
    }

    pub(crate) fn update_menu_summary(&self) {
        if self.menu_summary_label.0.is_null() {
            return;
        }
        let loaded_targets = self.paths[..self.image_count]
            .iter()
            .filter(|p| p.is_some())
            .count();
        let is_image_running = self
            .worker
            .as_ref()
            .is_some_and(|w| w.running.load(Ordering::Acquire));
        let is_classical_running = self
            .classical_worker
            .as_ref()
            .is_some_and(|w| w.running.load(Ordering::Acquire));
        let summary = if is_classical_running {
            if let Some(worker) = &self.classical_worker {
                let clicks = worker.clicks.load(Ordering::Relaxed);
                let cps = worker.cps.load(Ordering::Relaxed);
                format!(
                    "Classical: RUNNING • {clicks} clicks • {cps} CPS • {}",
                    self.classical_button.name()
                )
            } else {
                "Classical: RUNNING".to_owned()
            }
        } else if is_image_running {
            if let Some(worker) = &self.worker {
                let clicks = worker.clicks.load(Ordering::Relaxed);
                let scan_ms = worker.scan_duration_us.load(Ordering::Relaxed) as f64 / 1_000.0;
                format!("Images: RUNNING • {clicks} clicks • Scan: {scan_ms:.2} ms")
            } else {
                "Images: RUNNING".to_owned()
            }
        } else {
            let delay_str = if self.classical_delay_ms == 0 {
                "uncapped".to_owned()
            } else {
                format!("{}ms", self.classical_delay_ms)
            };
            format!(
                "Classical: {} ({delay_str})  •  Images: {loaded_targets}/{count} ready",
                self.classical_button.name(),
                count = self.image_count,
            )
        };
        set_text(self.menu_summary_label, &summary);
    }

    #[allow(dead_code)]
    pub(crate) fn set_classical_delay(&mut self, delay: u64) {
        let delay = delay.min(10000);
        self.classical_delay_ms = delay;
        set_text(self.classical_delay_input, &delay.to_string());
        self.update_menu_summary();
        self.save_preferences();
    }

    pub(crate) fn set_classical_button(&mut self, button: ClickButton) {
        self.classical_button = button;
        self.update_menu_summary();
        self.save_preferences();
    }

    pub(crate) fn sync_classical_interval(&mut self) {
        self.classical_delay_ms = (self.classical_hours as u64 * 3_600_000)
            + (self.classical_mins as u64 * 60_000)
            + (self.classical_secs as u64 * 1_000)
            + (self.classical_millis as u64);
    }

    pub(crate) fn start_classical(&mut self, window: HWND) {
        if let Some(worker) = &self.classical_worker
            && worker.running.load(Ordering::Acquire)
        {
            return;
        }

        if let Some(worker) = self.worker.take() {
            worker.stop.store(true, Ordering::Release);
        }

        self.set_always_on_top(window, true);
        if self.hide_when_active {
            unsafe {
                let _ = ShowWindow(window, SW_HIDE);
            }
        }

        self.sync_classical_interval();
        let random_offset_ms = if self.classical_random_offset_enabled {
            self.classical_random_offset_ms as u64
        } else {
            0
        };
        let repeat_limit = match self.classical_repeat_mode {
            ClassicalRepeatMode::UntilStopped => 0,
            ClassicalRepeatMode::Count => self.classical_repeat_count,
        };
        let target_pos = match self.classical_location_mode {
            ClassicalLocationMode::Current => None,
            ClassicalLocationMode::Pick => Some((self.classical_pick_x, self.classical_pick_y)),
        };

        let config = ClassicalWorkerConfig {
            delay_ms: self.classical_delay_ms,
            random_offset_ms,
            button: self.classical_button,
            click_type: self.classical_click_type,
            repeat_limit,
            target_pos,
        };
        let worker = Arc::new(ClassicalWorkerState::new(config));
        self.classical_worker = Some(Arc::clone(&worker));
        set_text(self.classical_status_label, "Running  |  Clicking...");
        self.update_menu_summary();

        let _ = thread::Builder::new()
            .name("classical-clicker".to_owned())
            .spawn(move || run_classical_clicker(worker));
    }

    pub(crate) fn stop_classical(&mut self, window: HWND) {
        if let Some(worker) = self.classical_worker.take() {
            worker.stop.store(true, Ordering::Release);
        }
        let start_str = vk_to_string(self.start_hotkey);
        set_text(
            self.classical_status_label,
            &format!("Stopped. Press {start_str} to start."),
        );
        self.restore_window(window);
        self.update_menu_summary();
    }

    pub(crate) fn set_running_ui(&mut self, window: HWND, running: bool) {
        unsafe {
            let visibility = if running || self.active_panel != ActivePanel::Clicker {
                SW_HIDE
            } else {
                SW_SHOW
            };
            let _ = ShowWindow(self.preview_label, visibility);
            let _ = ShowWindow(self.preview, visibility);
            let _ = ShowWindow(self.empty_preview, visibility);
        }
        if running {
            self.clear_preview_bitmap();
            self.preview_source = None;
        }
        self.update_menu_summary();
        layout_controls(window, self);
        if !running
            && self.active_panel == ActivePanel::Clicker
            && let Err(error) = self.refresh_preview(window)
        {
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
                let _ = ShowWindow(
                    self.empty_preview,
                    if running || self.active_panel != ActivePanel::Clicker {
                        SW_HIDE
                    } else {
                        SW_SHOW
                    },
                );
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
        self.update_menu_summary();
        let images_running = self
            .worker
            .as_ref()
            .is_some_and(|w| w.running.load(Ordering::Acquire));
        if !images_running {
            let start_str = vk_to_string(self.start_hotkey);
            let stop_str = vk_to_string(self.stop_hotkey);
            set_text(
                self.status_label,
                &format!("Ready  |  Press {start_str} to start, {stop_str} to stop."),
            );
        }
        if let Err(error) = self.refresh_preview(window) {
            self.show_message(&error, MB_ICONERROR);
        }
        self.save_preferences();
    }

    pub(crate) fn clear_selected_image(&mut self, window: HWND) {
        self.paths[self.selected_slot] = None;
        self.refresh_slot_names();
        self.update_menu_summary();
        let images_running = self
            .worker
            .as_ref()
            .is_some_and(|w| w.running.load(Ordering::Acquire));
        if !images_running {
            let start_str = vk_to_string(self.start_hotkey);
            let stop_str = vk_to_string(self.stop_hotkey);
            let has_images = self.paths[..self.image_count].iter().any(|p| p.is_some());
            if has_images {
                set_text(
                    self.status_label,
                    &format!("Ready  |  Press {start_str} to start, {stop_str} to stop."),
                );
            } else {
                set_text(
                    self.status_label,
                    &format!("Ready  |  Select target image, then press {start_str} to start."),
                );
            }
        }
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
        if self.hide_when_active {
            unsafe {
                let _ = ShowWindow(window, SW_HIDE);
            }
        }
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
        self.update_menu_summary();

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
            self.update_menu_summary();
        } else {
            self.set_running_ui(window, true);
        }
    }

    pub(crate) fn restore_window(&self, window: HWND) {
        if !window.0.is_null() {
            unsafe {
                let _ = ShowWindow(window, SW_SHOW);
                let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(window);
            }
        }
    }

    pub(crate) fn stop(&mut self, window: HWND) {
        let start_str = vk_to_string(self.start_hotkey);
        if let Some(worker) = &self.worker
            && worker.running.load(Ordering::Acquire)
        {
            worker.stop.store(true, Ordering::Release);
            set_text(self.status_label, "Stopping...");
        } else {
            set_text(
                self.status_label,
                &format!("Stopped. Press {start_str} to start."),
            );
        }
        self.stop_classical(window);
        self.restore_window(window);
        self.update_menu_summary();
    }

    pub(crate) fn set_hide_when_active(&mut self, enabled: bool) {
        self.hide_when_active = enabled;
        if !self.hide_when_active_checkbox.0.is_null() {
            unsafe {
                let _ = SendMessageW(
                    self.hide_when_active_checkbox,
                    BM_SETCHECK,
                    Some(WPARAM(if enabled { BST_CHECKED.0 as usize } else { 0 })),
                    Some(LPARAM(0)),
                );
            }
        }
        self.save_preferences();
    }

    pub(crate) fn update_hotkey_labels(&mut self) {
        let start_str = vk_to_string(self.start_hotkey);
        let stop_str = vk_to_string(self.stop_hotkey);
        let close_str = vk_to_string(self.close_hotkey);

        set_text(self.start_button, &format!("{start_str} - Start"));
        set_text(self.stop_button, &format!("{stop_str} - Stop"));
        set_text(self.close_button, &format!("{close_str} - Close"));

        set_text(self.classical_start_button, &format!("{start_str} - Start"));
        set_text(self.classical_stop_button, &format!("{stop_str} - Stop"));
        set_text(self.classical_close_button, &format!("{close_str} - Close"));

        set_text(self.start_hotkey_input, &start_str);
        set_text(self.stop_hotkey_input, &stop_str);
        set_text(self.close_hotkey_input, &close_str);

        set_text(
            self.menu_hotkey_label,
            &format!(
                "Global Hotkeys:  [{start_str}] Start   •   [{stop_str}] Stop   •   [{close_str}] Close"
            ),
        );
        set_text(
            self.menu_tips_label,
            &format!("Tip: Open Classical or Images, then press {start_str} to start."),
        );

        let classical_running = self
            .classical_worker
            .as_ref()
            .is_some_and(|w| w.is_running());
        if !classical_running {
            set_text(
                self.classical_status_label,
                &format!("Ready  |  Press {start_str} to start, {stop_str} to stop."),
            );
        }

        let images_running = self
            .worker
            .as_ref()
            .is_some_and(|w| w.running.load(Ordering::Acquire));
        if !images_running {
            let has_images = self.paths[..self.image_count].iter().any(|p| p.is_some());
            if has_images {
                set_text(
                    self.status_label,
                    &format!("Ready  |  Press {start_str} to start, {stop_str} to stop."),
                );
            } else {
                set_text(
                    self.status_label,
                    &format!("Ready  |  Select target image, then press {start_str} to start."),
                );
            }
        }
    }

    pub(crate) fn set_hotkey(&mut self, window: HWND, id: i32, vk: u32) {
        let _ = unsafe {
            windows::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey(Some(window), id)
        };
        let reg = unsafe {
            windows::Win32::UI::Input::KeyboardAndMouse::RegisterHotKey(
                Some(window),
                id,
                windows::Win32::UI::Input::KeyboardAndMouse::MOD_NOREPEAT,
                vk,
            )
        };
        match id {
            1 => self.start_hotkey = vk,
            2 => self.stop_hotkey = vk,
            3 => self.close_hotkey = vk,
            _ => {}
        }
        self.update_hotkey_labels();
        self.save_preferences();
        if reg.is_err() {
            self.show_message(
                &format!(
                    "Warning: Could not register global hotkey for {}. It will work only while focused.",
                    vk_to_string(vk)
                ),
                MB_ICONWARNING,
            );
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
        for checkbox in [
            self.always_on_top_checkbox,
            self.classical_always_on_top_checkbox,
        ] {
            if !checkbox.0.is_null() {
                unsafe {
                    SendMessageW(
                        checkbox,
                        BM_SETCHECK,
                        Some(WPARAM(if enabled { BST_CHECKED.0 as usize } else { 0 })),
                        Some(LPARAM(0)),
                    );
                }
            }
        }
        self.save_preferences();
    }
}

pub(crate) fn set_text(window: HWND, text: &str) {
    if window.0.is_null() {
        return;
    }
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let _ = SetWindowTextW(window, PCWSTR(text.as_ptr()));
    }
}
