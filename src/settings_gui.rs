use super::*;
use serde::{Deserialize, Serialize};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, InvalidateRect};

pub(super) const DEFAULT_IMAGE_COUNT: usize = 10;
pub(super) const ID_SETTINGS_DELAY_LABEL: usize = 300;
pub(super) const ID_SETTINGS_THEME_LABEL: usize = 301;
pub(super) const ID_SETTINGS_SLOT_COUNT_LABEL: usize = 305;
pub(super) const THEMES: [Theme; 5] = [
    Theme::Light,
    Theme::Dark,
    Theme::Blue,
    Theme::Green,
    Theme::Purple,
];

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Theme {
    Light,
    #[default]
    Dark,
    Blue,
    Green,
    Purple,
}

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Preferences {
    #[serde(default)]
    pub(crate) image_paths: Vec<Option<String>>,
    #[serde(default)]
    pub(crate) always_on_top: bool,
    #[serde(default = "default_click_delay_ms")]
    pub(crate) click_delay_ms: u64,
    #[serde(default)]
    pub(crate) theme: Theme,
    #[serde(default = "default_image_count")]
    pub(crate) image_count: usize,
}

const fn default_image_count() -> usize {
    DEFAULT_IMAGE_COUNT
}

const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | (green as u32) << 8 | (blue as u32) << 16
}

const fn darken(color: u32) -> u32 {
    let red = (color & 0xff) * 3 / 4;
    let green = ((color >> 8) & 0xff) * 3 / 4;
    let blue = ((color >> 16) & 0xff) * 3 / 4;
    red | green << 8 | blue << 16
}

impl Theme {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::Blue => "Blue",
            Self::Green => "Green",
            Self::Purple => "Purple",
        }
    }

    pub(super) fn colors(self) -> (COLORREF, COLORREF) {
        let (background, text) = match self {
            Self::Light => (rgb(248, 250, 252), rgb(30, 41, 59)),
            Self::Dark => (rgb(17, 24, 39), rgb(229, 231, 235)),
            Self::Blue => (rgb(15, 28, 46), rgb(224, 242, 254)),
            Self::Green => (rgb(18, 35, 30), rgb(220, 252, 231)),
            Self::Purple => (rgb(34, 24, 44), rgb(243, 232, 255)),
        };
        (COLORREF(background), COLORREF(text))
    }

    pub(super) fn button_colors(self, id: usize, selected: bool) -> (u32, u32, u32) {
        let (_, text) = self.colors();
        let accent = self.accent();
        let surface = self.surface_color();
        let border = match self {
            Self::Light => rgb(203, 213, 225),
            Self::Dark => rgb(55, 65, 81),
            Self::Blue => rgb(51, 65, 85),
            Self::Green => rgb(55, 75, 63),
            Self::Purple => rgb(76, 61, 91),
        };
        let primary = id == ID_START || id == ID_SETTINGS;
        let danger = id == ID_CLOSE || id == ID_CLEAR_IMAGE;
        let danger_accent = rgb(244, 63, 63);
        let fill = if primary {
            if selected { darken(surface) } else { surface }
        } else if danger {
            if selected {
                darken(danger_accent)
            } else {
                danger_accent
            }
        } else if selected {
            border
        } else {
            surface
        };
        let foreground = if primary {
            accent
        } else if danger {
            rgb(255, 255, 255)
        } else {
            text.0
        };
        (
            fill,
            foreground,
            if primary {
                accent
            } else if danger {
                danger_accent
            } else {
                border
            },
        )
    }

    pub(super) fn accent(self) -> u32 {
        match self {
            Self::Light => rgb(37, 99, 235),
            Self::Dark => rgb(0, 230, 170),
            Self::Blue => rgb(0, 211, 255),
            Self::Green => rgb(0, 240, 145),
            Self::Purple => rgb(208, 105, 255),
        }
    }

    pub(super) fn brush(self) -> HBRUSH {
        let (background, _) = self.colors();
        unsafe { CreateSolidBrush(background) }
    }

    pub(super) fn surface_brush(self) -> HBRUSH {
        unsafe { CreateSolidBrush(COLORREF(self.surface_color())) }
    }

    pub(super) fn surface_color(self) -> u32 {
        match self {
            Self::Light => rgb(255, 255, 255),
            Self::Dark => rgb(31, 41, 55),
            Self::Blue => rgb(30, 48, 70),
            Self::Green => rgb(30, 51, 43),
            Self::Purple => rgb(51, 39, 64),
        }
    }
}

/// Owns settings-page interactions and applies persisted setting changes to the running UI.
impl AppState {
    pub(super) fn set_settings_open(&mut self, window: HWND, open: bool) {
        self.settings_open = open;
        let visibility = if open { SW_HIDE } else { SW_SHOW };
        let settings_visibility = if open { SW_SHOW } else { SW_HIDE };
        unsafe {
            for control in &self.main_controls {
                let _ = ShowWindow(*control, visibility);
            }
            for control in &self.settings_controls {
                let _ = ShowWindow(*control, settings_visibility);
            }
        }
        set_text(self.main_heading, "RustyClicker");
        set_text(self.settings_button, if open { "Back" } else { "Settings" });
        layout_controls(window, self);
    }

    pub(super) fn set_click_delay(&mut self, selection: usize) {
        let Some(delay) = CLICK_DELAYS_MS.get(selection).copied() else {
            self.show_message("Select a valid click delay.", MB_ICONWARNING);
            return;
        };
        self.click_delay_ms = delay;
        if let Some(worker) = &self.worker {
            worker.click_delay_ms.store(delay, Ordering::Release);
        }
        self.save_preferences();
    }

    pub(super) fn set_image_count(&mut self, window: HWND, count: usize) {
        if !(1..=MAX_IMAGE_COUNT).contains(&count) {
            self.show_message("Choose between 1 and 99 image slots.", MB_ICONWARNING);
            return;
        }
        self.image_count = count;
        if self.selected_slot >= count {
            self.selected_slot = count - 1;
        }
        unsafe {
            SendMessageW(
                self.slot_combo,
                CB_SETCURSEL,
                Some(WPARAM(self.selected_slot)),
                Some(LPARAM(0)),
            );
        }
        self.refresh_slot_names();
        if let Err(error) = self.refresh_preview(window) {
            self.show_message(&error, MB_ICONERROR);
        }
        self.save_preferences();
    }

    pub(super) fn set_theme(&mut self, window: HWND, selection: usize) {
        let Some(theme) = THEMES.get(selection).copied() else {
            self.show_message("Select a valid window color.", MB_ICONWARNING);
            return;
        };
        unsafe {
            let new_brush = theme.brush();
            let new_surface_brush = theme.surface_brush();
            if new_brush.0.is_null() || new_surface_brush.0.is_null() {
                if !new_brush.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(new_brush.0));
                }
                if !new_surface_brush.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(new_surface_brush.0));
                }
                self.show_message("Could not create the window-color brush.", MB_ICONERROR);
                return;
            }
            if !self.background_brush.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.background_brush.0));
            }
            if !self.surface_brush.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.surface_brush.0));
            }
            self.theme = theme;
            self.background_brush = new_brush;
            self.surface_brush = new_surface_brush;
            let _ = InvalidateRect(Some(window), None, true);
            for control in self
                .main_controls
                .iter()
                .chain(self.settings_controls.iter())
                .chain(std::iter::once(&self.settings_button))
            {
                let _ = InvalidateRect(Some(*control), None, true);
            }
            let dark_titlebar = i32::from(theme != Theme::Light);
            let _ = DwmSetWindowAttribute(
                window,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_titlebar as *const i32 as *const _,
                std::mem::size_of_val(&dark_titlebar) as u32,
            );
        }
        self.save_preferences();
    }
}
