use super::dxgi::DxgiCapture;
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

pub enum ScreenCapture {
    Dxgi(DxgiCapture),
    Gdi(GdiCapture),
}

pub struct GdiCapture {
    memory_dc: HDC,
    bitmap: HBITMAP,
    previous_bitmap: HGDIOBJ,
    pixels: *mut u8,
    width: i32,
    height: i32,
}

impl GdiCapture {
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
            println!("[GDI DEBUG] bitmap={:?}, type={}, prev={:?}", bitmap, windows::Win32::Graphics::Gdi::GetObjectType(HGDIOBJ(bitmap.0)), previous_bitmap);
            ReleaseDC(None, screen_dc);
            if previous_bitmap.0.is_null() || previous_bitmap.0 as isize == -1 {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                let _ = DeleteDC(memory_dc);
                return Err(CoreError::CaptureBitmapSelection);
            }
            Ok(Self {
                memory_dc,
                bitmap,
                previous_bitmap,
                pixels: pixels.cast(),
                width,
                height,
            })
        }
    }

    pub(crate) fn capture(
        &mut self,
        gray: &mut Vec<u8>,
        origin_x: i32,
        origin_y: i32,
        width: i32,
        height: i32,
    ) -> Result<(u32, u32, i32, i32), CoreError> {
        unsafe {
            let screen_dc = GetDC(None);
            if screen_dc.0.is_null() {
                return Err(CoreError::CaptureScreenDeviceContext);
            }
            let blt_res = BitBlt(
                self.memory_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
                origin_x,
                origin_y,
                SRCCOPY | CAPTUREBLT,
            );
            if blt_res.is_err() {
                BitBlt(
                    self.memory_dc,
                    0,
                    0,
                    width,
                    height,
                    Some(screen_dc),
                    origin_x,
                    origin_y,
                    SRCCOPY,
                )
                .map_err(|error| {
                    ReleaseDC(None, screen_dc);
                    CoreError::CapturePixels(error.to_string())
                })?;
            }
            ReleaseDC(None, screen_dc);

            let pixel_bytes = (width as usize)
                .checked_mul(height as usize)
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or(CoreError::DimensionsTooLarge("Screenshot"))?;
            let bgra = std::slice::from_raw_parts(self.pixels, pixel_bytes);
            bgra_to_gray(bgra, width as u32, height as u32, gray)?;
        }
        Ok((width as u32, height as u32, origin_x, origin_y))
    }

    pub fn capture_bgra(
        &mut self,
        bgra_out: &mut Vec<u8>,
        origin_x: i32,
        origin_y: i32,
        width: i32,
        height: i32,
    ) -> Result<(u32, u32, i32, i32), CoreError> {
        unsafe {
            let screen_dc = GetDC(None);
            if screen_dc.0.is_null() {
                return Err(CoreError::CaptureScreenDeviceContext);
            }
            let blt_res = BitBlt(
                self.memory_dc,
                0,
                0,
                width,
                height,
                Some(screen_dc),
                origin_x,
                origin_y,
                SRCCOPY | CAPTUREBLT,
            );
            if blt_res.is_err() {
                BitBlt(
                    self.memory_dc,
                    0,
                    0,
                    width,
                    height,
                    Some(screen_dc),
                    origin_x,
                    origin_y,
                    SRCCOPY,
                )
                .map_err(|error| {
                    ReleaseDC(None, screen_dc);
                    CoreError::CapturePixels(error.to_string())
                })?;
            }
            ReleaseDC(None, screen_dc);

            let pixel_bytes = (width as usize)
                .checked_mul(height as usize)
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or(CoreError::DimensionsTooLarge("Screenshot"))?;
            let slice = std::slice::from_raw_parts(self.pixels, pixel_bytes);
            bgra_out.clear();
            bgra_out.extend_from_slice(slice);
        }
        Ok((width as u32, height as u32, origin_x, origin_y))
    }
}

impl Drop for GdiCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.memory_dc, self.previous_bitmap);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.memory_dc);
        }
    }
}

unsafe impl Send for GdiCapture {}
unsafe impl Sync for GdiCapture {}

/// Ensures the calling thread is attached to the active input desktop.
/// Crucial for background threads, Python scripts, and non-window processes on Windows.
pub fn attach_input_desktop() {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn OpenInputDesktop(flags: u32, inherit: i32, access: u32) -> *mut std::ffi::c_void;
        fn SetThreadDesktop(desktop: *mut std::ffi::c_void) -> i32;
        fn CloseDesktop(desktop: *mut std::ffi::c_void) -> i32;
    }
    unsafe {
        let desk = OpenInputDesktop(0, 0, 0x0100);
        if !desk.is_null() {
            let _ = SetThreadDesktop(desk);
            let _ = CloseDesktop(desk);
        }
    }
}

/// Captures the screen via GPU-accelerated DXGI Desktop Duplication,
/// falling back seamlessly to GdiCapture if hardware duplication is unavailable or lost.
pub fn capture_screen(
    capture: &mut Option<ScreenCapture>,
    gray: &mut Vec<u8>,
) -> Result<(u32, u32, i32, i32), CoreError> {
    attach_input_desktop();
    if capture.is_none() {
        // Try GPU-accelerated DXGI first
        match DxgiCapture::new() {
            Ok(dxgi) => {
                *capture = Some(ScreenCapture::Dxgi(dxgi));
            }
            Err(_) => {
                let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
                let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
                if width <= 0 || height <= 0 {
                    return Err(CoreError::CaptureScreenSize);
                }
                *capture = Some(ScreenCapture::Gdi(GdiCapture::new(width, height)?));
            }
        }
    }

    match capture.as_mut() {
        Some(ScreenCapture::Dxgi(dxgi)) => match dxgi.capture(gray) {
            Ok(res) => Ok(res),
            Err(_) => {
                // If DXGI fails (e.g. resolution change or UAC prompt), try re-creating DXGI once
                if let Ok(mut new_dxgi) = DxgiCapture::new() {
                    if let Ok(res) = new_dxgi.capture(gray) {
                        *dxgi = new_dxgi;
                        return Ok(res);
                    }
                }
                // Fall back seamlessly to GDI
                let origin_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
                let origin_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
                let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
                let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
                if width <= 0 || height <= 0 {
                    return Err(CoreError::CaptureScreenSize);
                }
                let mut gdi = GdiCapture::new(width, height)?;
                let res = gdi.capture(gray, origin_x, origin_y, width, height)?;
                *capture = Some(ScreenCapture::Gdi(gdi));
                Ok(res)
            }
        },
        Some(ScreenCapture::Gdi(gdi)) => {
            let origin_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let origin_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
            let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
            let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
            if width <= 0 || height <= 0 {
                return Err(CoreError::CaptureScreenSize);
            }
            if gdi.width != width || gdi.height != height {
                *gdi = GdiCapture::new(width, height)?;
            }
            gdi.capture(gray, origin_x, origin_y, width, height)
        }
        None => Err(CoreError::CaptureNotInitialized),
    }
}

/// Captures the screen to a 32-bit BGRA buffer using DXGI or GDI.
pub fn capture_screen_bgra(
    capture: &mut Option<ScreenCapture>,
    bgra: &mut Vec<u8>,
) -> Result<(u32, u32, i32, i32), CoreError> {
    attach_input_desktop();
    if capture.is_none() {
        match DxgiCapture::new() {
            Ok(dxgi) => {
                *capture = Some(ScreenCapture::Dxgi(dxgi));
            }
            Err(_) => {
                let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
                let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
                if width <= 0 || height <= 0 {
                    return Err(CoreError::CaptureScreenSize);
                }
                *capture = Some(ScreenCapture::Gdi(GdiCapture::new(width, height)?));
            }
        }
    }

    match capture.as_mut() {
        Some(ScreenCapture::Dxgi(dxgi)) => match dxgi.capture_bgra(bgra) {
            Ok(res) => Ok(res),
            Err(_) => {
                if let Ok(mut new_dxgi) = DxgiCapture::new() {
                    if let Ok(res) = new_dxgi.capture_bgra(bgra) {
                        *dxgi = new_dxgi;
                        return Ok(res);
                    }
                }
                let origin_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
                let origin_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
                let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
                let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
                if width <= 0 || height <= 0 {
                    return Err(CoreError::CaptureScreenSize);
                }
                let mut gdi = GdiCapture::new(width, height)?;
                let res = gdi.capture_bgra(bgra, origin_x, origin_y, width, height)?;
                *capture = Some(ScreenCapture::Gdi(gdi));
                Ok(res)
            }
        },
        Some(ScreenCapture::Gdi(gdi)) => {
            let origin_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let origin_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
            let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
            let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
            if width <= 0 || height <= 0 {
                return Err(CoreError::CaptureScreenSize);
            }
            if gdi.width != width || gdi.height != height {
                *gdi = GdiCapture::new(width, height)?;
            }
            gdi.capture_bgra(bgra, origin_x, origin_y, width, height)
        }
        None => Err(CoreError::CaptureNotInitialized),
    }
}

/// Fast in-memory cropping of a 32-bit BGRA buffer.
pub fn crop_bgra(
    src: &[u8],
    src_width: u32,
    src_height: u32,
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
) -> Option<Vec<u8>> {
    if crop_x + crop_w > src_width || crop_y + crop_h > src_height || crop_w == 0 || crop_h == 0 {
        return None;
    }
    let src_stride = (src_width * 4) as usize;
    let crop_stride = (crop_w * 4) as usize;
    let mut out = Vec::with_capacity(crop_stride * (crop_h as usize));

    for y in 0..crop_h {
        let src_offset = ((crop_y + y) as usize) * src_stride + ((crop_x * 4) as usize);
        let row_slice = &src[src_offset..src_offset + crop_stride];
        out.extend_from_slice(row_slice);
    }

    Some(out)
}
