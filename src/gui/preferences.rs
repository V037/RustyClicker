use super::{DEFAULT_IMAGE_COUNT, MAX_IMAGE_COUNT, state::AppState};
use crate::matching::{
    DEFAULT_MATCH_THREADS, default_click_delay_ms, normalize_click_delay_ms,
    normalize_match_threads,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_ICONWARNING};

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

#[derive(Serialize, Deserialize)]
pub(crate) struct Preferences {
    #[serde(default)]
    pub(crate) image_paths: Vec<Option<String>>,
    #[serde(default)]
    pub(crate) always_on_top: bool,
    #[serde(default = "default_click_delay_ms")]
    pub(crate) click_delay_ms: u64,
    #[serde(default = "default_match_threads")]
    pub(crate) match_threads: usize,
    #[serde(default)]
    pub(crate) theme: Theme,
    #[serde(default = "default_image_count")]
    pub(crate) image_count: usize,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            image_paths: Vec::new(),
            always_on_top: false,
            click_delay_ms: default_click_delay_ms(),
            match_threads: default_match_threads(),
            theme: Theme::default(),
            image_count: default_image_count(),
        }
    }
}

pub(crate) const fn default_image_count() -> usize {
    DEFAULT_IMAGE_COUNT
}

pub(crate) const fn default_match_threads() -> usize {
    DEFAULT_MATCH_THREADS
}

pub(crate) const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | (green as u32) << 8 | (blue as u32) << 16
}

pub(crate) const fn darken(color: u32) -> u32 {
    let red = (color & 0xff) * 3 / 4;
    let green = ((color >> 8) & 0xff) * 3 / 4;
    let blue = ((color >> 16) & 0xff) * 3 / 4;
    red | green << 8 | blue << 16
}

impl AppState {
    pub(super) fn load_preferences(&mut self) {
        let Some(preferences_path) = &self.preferences_path else {
            self.show_message(
                "Could not find a Windows app-data folder for saving preferences.",
                MB_ICONWARNING,
            );
            return;
        };
        let (contents, migrate_legacy_preferences) = match fs::read(preferences_path) {
            Ok(contents) => (contents, false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(legacy_path) = std::env::current_exe()
                    .ok()
                    .and_then(|path| path.parent().map(Path::to_path_buf))
                    .map(|directory| directory.join("image_clicker_preferences.json"))
                else {
                    return;
                };
                match fs::read(legacy_path) {
                    Ok(contents) => (contents, true),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                    Err(error) => {
                        self.show_message(
                            &format!("Could not read saved preferences: {error}"),
                            MB_ICONWARNING,
                        );
                        return;
                    }
                }
            }
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
        self.match_threads = normalize_match_threads(preferences.match_threads);
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
        if migrate_legacy_preferences {
            self.save_preferences();
        }
    }

    pub(super) fn save_preferences(&self) {
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
            match_threads: self.match_threads,
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
        let Some(preferences_path) = &self.preferences_path else {
            self.show_message(
                "Could not find a Windows app-data folder for saving preferences.",
                MB_ICONERROR,
            );
            return;
        };
        if let Some(directory) = preferences_path.parent()
            && let Err(error) = fs::create_dir_all(directory)
        {
            self.show_message(
                &format!(
                    "Could not create preferences folder {}: {error}",
                    directory.display()
                ),
                MB_ICONERROR,
            );
            return;
        }
        if let Err(error) = fs::write(preferences_path, contents) {
            self.show_message(
                &format!(
                    "Could not save preferences to {}: {error}",
                    preferences_path.display()
                ),
                MB_ICONERROR,
            );
        }
    }
}
