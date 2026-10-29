use crate::core::CoreError;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;

#[derive(Clone, Debug, PartialEq)]
pub struct OcrWord {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl OcrWord {
    pub fn center_x(&self) -> i32 {
        self.x + self.width / 2
    }

    pub fn center_y(&self) -> i32 {
        self.y + self.height / 2
    }
}

#[derive(Clone)]
pub struct FastOcr {
    engine: OcrEngine,
}

impl FastOcr {
    pub fn new() -> Result<Self, CoreError> {
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|e| CoreError::MatcherExecution(format!("Failed to create Windows OCR engine: {}", e)))?;
        Ok(Self { engine })
    }

    pub fn recognize_bgra(&self, bgra: &[u8], width: u32, height: u32) -> Result<(String, Vec<OcrWord>), CoreError> {
        if width == 0 || height == 0 || bgra.len() < (width * height * 4) as usize {
            return Err(CoreError::InvalidImageDimensions);
        }

        let writer = DataWriter::new()
            .map_err(|e| CoreError::MatcherExecution(format!("DataWriter error: {}", e)))?;
        writer.WriteBytes(bgra)
            .map_err(|e| CoreError::MatcherExecution(format!("DataWriter write error: {}", e)))?;
        let buffer = writer.DetachBuffer()
            .map_err(|e| CoreError::MatcherExecution(format!("DetachBuffer error: {}", e)))?;

        let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &buffer,
            BitmapPixelFormat::Bgra8,
            width as i32,
            height as i32,
        ).map_err(|e| CoreError::MatcherExecution(format!("SoftwareBitmap error: {}", e)))?;

        let op = self.engine.RecognizeAsync(&bitmap)
            .map_err(|e| CoreError::MatcherExecution(format!("RecognizeAsync error: {}", e)))?;

        while op.Status().map_err(|e| CoreError::MatcherExecution(e.to_string()))?.0 == 0 {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        let result = op.GetResults()
            .map_err(|e| CoreError::MatcherExecution(format!("GetResults error: {}", e)))?;

        let full_text = result.Text()
            .map_err(|e| CoreError::MatcherExecution(e.to_string()))?
            .to_string();

        let mut words_list = Vec::new();
        if let Ok(lines) = result.Lines() {
            for line in lines {
                if let Ok(words) = line.Words() {
                    for word in words {
                        let text = word.Text().map(|t| t.to_string()).unwrap_or_default();
                        let rect = word.BoundingRect().unwrap_or_default();
                        words_list.push(OcrWord {
                            text,
                            x: rect.X as i32,
                            y: rect.Y as i32,
                            width: rect.Width as i32,
                            height: rect.Height as i32,
                        });
                    }
                }
            }
        }

        Ok((full_text, words_list))
    }

    pub fn recognize_screen(
        &self,
        capture: &mut Option<crate::capture::ScreenCapture>,
        bgra_buffer: &mut Vec<u8>,
        region: Option<(u32, u32, u32, u32)>,
    ) -> Result<(String, Vec<OcrWord>), CoreError> {
        let (screen_w, screen_h, orig_x, orig_y) = crate::capture::capture_screen_bgra(capture, bgra_buffer)?;

        let (pixels_to_ocr, ocr_w, ocr_h, offset_x, offset_y) = if let Some((rx, ry, rw, rh)) = region {
            let cropped = crate::capture::crop_bgra(bgra_buffer, screen_w, screen_h, rx, ry, rw, rh)
                .ok_or(CoreError::InvalidImageDimensions)?;
            (cropped, rw, rh, rx as i32 + orig_x, ry as i32 + orig_y)
        } else {
            (bgra_buffer.clone(), screen_w, screen_h, orig_x, orig_y)
        };

        let (text, mut words) = self.recognize_bgra(&pixels_to_ocr, ocr_w, ocr_h)?;
        for word in &mut words {
            word.x += offset_x;
            word.y += offset_y;
        }

        Ok((text, words))
    }
}

pub fn normalize_text(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn compact_text(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub fn text_contains_phrase(text: &str, phrase: &str) -> bool {
    let norm_text = normalize_text(text);
    let norm_phrase = normalize_text(phrase);
    if norm_text.contains(&norm_phrase) {
        return true;
    }
    let comp_text = compact_text(text);
    let comp_phrase = compact_text(phrase);
    comp_text.contains(&comp_phrase)
}

pub fn find_phrase_box(words: &[OcrWord], target_phrase: &str) -> Option<(i32, i32, i32, i32, i32, i32)> {
    let target_words: Vec<String> = normalize_text(target_phrase)
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if target_words.is_empty() {
        return None;
    }

    let norm_words: Vec<String> = words.iter().map(|w| normalize_text(&w.text)).collect();

    for start in 0..norm_words.len().saturating_sub(target_words.len()) + 1 {
        let slice = &norm_words[start..start + target_words.len()];
        if slice == target_words.as_slice() {
            let matched = &words[start..start + target_words.len()];
            let left = matched.iter().map(|w| w.x).min().unwrap_or(0);
            let top = matched.iter().map(|w| w.y).min().unwrap_or(0);
            let right = matched.iter().map(|w| w.x + w.width).max().unwrap_or(0);
            let bottom = matched.iter().map(|w| w.y + w.height).max().unwrap_or(0);
            let width = right - left;
            let height = bottom - top;
            let center_x = left + width / 2;
            let center_y = top + height / 2;
            return Some((center_x, center_y, left, top, width, height));
        }
    }
    None
}

pub fn extract_coords_from_text(text: &str) -> Option<(f64, f64, f64)> {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_ascii_digit() || c == '.' || c == '-' { c } else { ' ' })
        .collect();
    let mut nums = Vec::new();
    for token in cleaned.split_whitespace() {
        if let Ok(val) = token.parse::<f64>() {
            nums.push(val);
        }
    }
    if nums.len() >= 3 {
        Some((nums[0], nums[1], nums[2]))
    } else {
        None
    }
}
