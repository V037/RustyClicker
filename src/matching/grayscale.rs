use crate::core::CoreError;

/// Converts RGB bytes to grayscale using OpenCV-compatible fixed-point coefficients.
pub fn rgb_to_gray(rgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, CoreError> {
    let expected_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or(CoreError::DimensionsTooLarge("Image"))?;
    if rgb.len() != expected_bytes {
        return Err(CoreError::InvalidPixelBuffer {
            format: "RGB",
            expected: expected_bytes,
            actual: rgb.len(),
        });
    }
    let mut gray = Vec::with_capacity(expected_bytes / 3);
    for pixel in rgb.chunks_exact(3) {
        gray.push(rgb_pixel_to_gray(pixel[0], pixel[1], pixel[2]));
    }
    Ok(gray)
}

#[inline]
pub fn rgb_pixel_to_gray(red: u8, green: u8, blue: u8) -> u8 {
    let gray = 4_899 * u32::from(red) + 9_617 * u32::from(green) + 1_868 * u32::from(blue) + 8_192;
    (gray >> 14) as u8
}

pub fn bgra_to_gray(
    bgra: &[u8],
    width: u32,
    height: u32,
    gray: &mut Vec<u8>,
) -> Result<(), CoreError> {
    let expected_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(CoreError::DimensionsTooLarge("Screenshot"))?;
    if bgra.len() != expected_bytes {
        return Err(CoreError::InvalidPixelBuffer {
            format: "Captured BGRA",
            expected: expected_bytes,
            actual: bgra.len(),
        });
    }
    let pixel_count = expected_bytes / 4;
    gray.resize(pixel_count, 0);
    for (pixel, output) in bgra.chunks_exact(4).zip(gray) {
        *output = rgb_pixel_to_gray(pixel[2], pixel[1], pixel[0]);
    }
    Ok(())
}
