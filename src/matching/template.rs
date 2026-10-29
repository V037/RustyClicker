use super::grayscale::rgb_to_gray;
use super::pool::match_pool;
use crate::core::CoreError;
use image::RgbaImage;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchResult {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub score: f32,
}

impl MatchResult {
    pub fn center_x(&self) -> u32 {
        self.x + self.width / 2
    }

    pub fn center_y(&self) -> u32 {
        self.y + self.height / 2
    }
}

pub struct PreparedTemplate {
    pub width: u32,
    pub height: u32,
    gray_energy: f64,
    matcher: Option<corrmatch::Matcher>,
}

impl PreparedTemplate {
    /// Prepares a grayscale, mean-centered template for normalized correlation.
    pub fn new(source: RgbaImage) -> Result<Self, CoreError> {
        let (width, height) = source.dimensions();
        if width == 0 || height == 0 {
            return Err(CoreError::InvalidImageDimensions);
        }
        let rgb: Vec<u8> = source
            .pixels()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        let gray = rgb_to_gray(&rgb, width, height)?;
        let pixel_count = gray.len() as i64;
        let gray_sum: i64 = gray.iter().map(|&pixel| i64::from(pixel)).sum();
        let integer_mean = gray_sum / pixel_count;
        let gray_energy: i64 = gray
            .iter()
            .map(|&pixel| {
                let centered = i64::from(pixel) - integer_mean;
                centered * centered
            })
            .sum();
        let gray_energy = gray_energy as f64;
        let width_usize =
            usize::try_from(width).map_err(|_| CoreError::DimensionsTooLarge("Image width"))?;
        let height_usize =
            usize::try_from(height).map_err(|_| CoreError::DimensionsTooLarge("Image height"))?;
        let matcher = if gray_energy > 1e-12 {
            let matcher_template = corrmatch::Template::new(gray, width_usize, height_usize)
                .map_err(|error| CoreError::TemplateCreation(error.to_string()))?;
            let mut compile_config = corrmatch::CompileConfigNoRot::default();
            compile_config.max_levels = 4;
            let compiled_template =
                corrmatch::CompiledTemplate::compile_unrotated(&matcher_template, compile_config)
                    .map_err(|error| CoreError::TemplateCompilation(error.to_string()))?;
            let mut match_config = corrmatch::MatchConfig::default();
            match_config.parallel = true;
            match_config.max_image_levels = 4;
            Some(corrmatch::Matcher::new(compiled_template).with_config(match_config))
        } else {
            None
        };
        Ok(Self {
            width,
            height,
            gray_energy,
            matcher,
        })
    }

    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0 && self.gray_energy > 1e-12 && self.matcher.is_some()
    }

    pub fn find_match_detailed(
        &self,
        screen_gray: &[u8],
        screen_width: u32,
        screen_height: u32,
        confidence: f32,
        stop: Option<&AtomicBool>,
        thread_count: usize,
    ) -> Result<Option<MatchResult>, CoreError> {
        if !self.is_valid()
            || self.width > screen_width
            || self.height > screen_height
            || stop.is_some_and(|flag| flag.load(Ordering::Acquire))
        {
            return Ok(None);
        }
        let Some(matcher) = &self.matcher else {
            return Ok(None);
        };
        let width = usize::try_from(screen_width)
            .map_err(|_| CoreError::DimensionsTooLarge("Screenshot width"))?;
        let height = usize::try_from(screen_height)
            .map_err(|_| CoreError::DimensionsTooLarge("Screenshot height"))?;
        let image = corrmatch::ImageView::from_slice(screen_gray, width, height)
            .map_err(|error| CoreError::MatcherInput(error.to_string()))?;
        let pool = match_pool(thread_count)?;
        let result = match pool.install(|| matcher.match_image(image)) {
            Ok(result) => result,
            Err(corrmatch::CorrMatchError::NoCandidates { .. }) => return Ok(None),
            Err(error) => {
                let msg = error.to_string();
                if msg.contains("no candidates") {
                    return Ok(None);
                }
                return Err(CoreError::MatcherExecution(msg));
            }
        };
        if stop.is_some_and(|flag| flag.load(Ordering::Acquire)) || result.score <= confidence {
            return Ok(None);
        }
        let max_x = screen_width - self.width;
        let max_y = screen_height - self.height;
        let x = (result.x.round() as u32).min(max_x);
        let y = (result.y.round() as u32).min(max_y);
        Ok(Some(MatchResult {
            x,
            y,
            width: self.width,
            height: self.height,
            score: result.score,
        }))
    }

    pub fn find_match(
        &self,
        screen_gray: &[u8],
        screen_width: u32,
        screen_height: u32,
        confidence: f32,
        stop: Option<&AtomicBool>,
        thread_count: usize,
    ) -> Result<Option<(u32, u32, u32, u32)>, CoreError> {
        self.find_match_detailed(
            screen_gray,
            screen_width,
            screen_height,
            confidence,
            stop,
            thread_count,
        )
        .map(|opt| opt.map(|m| (m.x, m.y, m.width, m.height)))
    }
}
