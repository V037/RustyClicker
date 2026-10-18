use crate::core::CoreError;
use crate::matching::bgra_to_gray;
use windows::Win32::{
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CreateCompatibleDC,
        CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, HGDIOBJ,
        ReleaseDC, SRCCOPY, SelectObject,
    },
    UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    },
};

pub(crate) struct ScreenCapture {
    screen_dc: HDC,
    memory_dc: HDC,
    bitmap: HBITMAP,
    previous_bitmap: HGDIOBJ,
    pixels: *mut u8,
    width: i32,
    height: i32,
}

impl ScreenCapture {
    pub(crate) fn new(width: i32, height: i32) -> Result<Self, CoreError> {
        usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4))
            .filter(|&bytes| bytes <= isize::MAX as usize)
            .ok_or(CoreError::CaptureDimensionsTooLarge)?;
        unsafe {
            let screen_dc = GetDC(None);
            if screen_dc.0.is_null() {
                return Err(CoreError::CaptureScreenDeviceContext);
            }
            let memory_dc = CreateCompatibleDC(Some(screen_dc));
            if memory_dc.0.is_null() {
                ReleaseDC(None, screen_dc);
                return Err(CoreError::CaptureMemoryDeviceContext);
            }
            let bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(
                Some(screen_dc),
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut pixels,
                None,
                0,
            ) {
                Ok(bitmap) => bitmap,
                Err(error) => {
                    let _ = DeleteDC(memory_dc);
                    ReleaseDC(None, screen_dc);
                    return Err(CoreError::CaptureBitmap(error.to_string()));
                }
            };
            if bitmap.0.is_null() || pixels.is_null() {
                if !bitmap.0.is_null() {
                    let _ = DeleteObject(HGDIOBJ(bitmap.0));
                }
                let _ = DeleteDC(memory_dc);
                ReleaseDC(None, screen_dc);
                return Err(CoreError::CaptureBitmap(
                    "Windows returned an invalid bitmap or pixel buffer.".to_owned(),
                ));
            }
            let previous_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
            if previous_bitmap.0.is_null() || previous_bitmap.0 as isize == -1 {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                let _ = DeleteDC(memory_dc);
                ReleaseDC(None, screen_dc);
                return Err(CoreError::CaptureBitmapSelection);
            }
            Ok(Self {
                screen_dc,
                memory_dc,
                bitmap,
                previous_bitmap,
                pixels: pixels.cast(),
                width,
                height,
            })
        }
    }
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.memory_dc, self.previous_bitmap);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.memory_dc);
            ReleaseDC(None, self.screen_dc);
        }
    }
}

/// Reuses a top-down DIB so capture needs only BitBlt and a direct grayscale conversion.
pub(crate) fn capture_screen(
    capture: &mut Option<ScreenCapture>,
    gray: &mut Vec<u8>,
) -> Result<(u32, u32, i32, i32), CoreError> {
    let origin_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let origin_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    if width <= 0 || height <= 0 {
        return Err(CoreError::CaptureScreenSize);
    }
    if capture
        .as_ref()
        .is_none_or(|capture| capture.width != width || capture.height != height)
    {
        *capture = Some(ScreenCapture::new(width, height)?);
    }
    let capture = capture.as_mut().ok_or(CoreError::CaptureNotInitialized)?;
    unsafe {
        BitBlt(
            capture.memory_dc,
            0,
            0,
            width,
            height,
            Some(capture.screen_dc),
            origin_x,
            origin_y,
            SRCCOPY | CAPTUREBLT,
        )
        .map_err(|error| CoreError::CapturePixels(error.to_string()))?;
        let pixel_bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(CoreError::DimensionsTooLarge("Screenshot"))?;
        let bgra = std::slice::from_raw_parts(capture.pixels, pixel_bytes);
        bgra_to_gray(bgra, width as u32, height as u32, gray)?;
    }
    Ok((width as u32, height as u32, origin_x, origin_y))
}
