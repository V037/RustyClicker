use crate::core::CoreError;
use crate::matching::rgb_pixel_to_gray;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::Interface;

pub struct DxgiCapture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging_texture: Option<ID3D11Texture2D>,
    width: u32,
    height: u32,
    origin_x: i32,
    origin_y: i32,
    has_valid_frame: bool,
}

impl DxgiCapture {
    pub(crate) fn new() -> Result<Self, CoreError> {
        unsafe {
            let mut device = None;
            let mut context = None;
            let mut feature_level = D3D_FEATURE_LEVEL_11_0;
            let feature_levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];

            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature_level),
                Some(&mut context),
            )
            .map_err(|e| {
                CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(e.to_string())))
            })?;

            let device = device.ok_or_else(|| {
                CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(
                    "D3D11 device creation returned None".to_owned(),
                )))
            })?;
            let context = context.ok_or_else(|| {
                CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(
                    "D3D11 context creation returned None".to_owned(),
                )))
            })?;

            let dxgi_device: IDXGIDevice = device.cast().map_err(|e| {
                CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(e.to_string())))
            })?;
            let adapter = dxgi_device.GetAdapter().map_err(|e| {
                CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(e.to_string())))
            })?;

            // Find an attached desktop display, prioritizing primary monitor (0, 0)
            let mut selected_output = None;
            let mut i = 0;
            while let Ok(output) = adapter.EnumOutputs(i) {
                if let Ok(desc) = output.GetDesc() {
                    if desc.AttachedToDesktop.as_bool() {
                        let is_primary =
                            desc.DesktopCoordinates.left == 0 && desc.DesktopCoordinates.top == 0;
                        if is_primary {
                            selected_output = Some((output, desc));
                            break;
                        } else if selected_output.is_none() {
                            selected_output = Some((output, desc));
                        }
                    }
                }
                i += 1;
            }

            let (output, output_desc) = selected_output.ok_or_else(|| {
                CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(
                    "No attached desktop display output found".to_owned(),
                )))
            })?;

            let width = (output_desc.DesktopCoordinates.right - output_desc.DesktopCoordinates.left)
                .max(1) as u32;
            let height = (output_desc.DesktopCoordinates.bottom
                - output_desc.DesktopCoordinates.top)
                .max(1) as u32;
            let origin_x = output_desc.DesktopCoordinates.left;
            let origin_y = output_desc.DesktopCoordinates.top;

            let output1: IDXGIOutput1 = output.cast().map_err(|e| {
                CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(e.to_string())))
            })?;
            let duplication = output1.DuplicateOutput(&device).map_err(|e| {
                CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(e.to_string())))
            })?;

            Ok(Self {
                device,
                context,
                duplication,
                staging_texture: None,
                width,
                height,
                origin_x,
                origin_y,
                has_valid_frame: false,
            })
        }
    }

    fn ensure_staging_texture(&mut self) -> Result<&ID3D11Texture2D, CoreError> {
        if self.staging_texture.is_none() {
            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: self.width,
                Height: self.height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut staging = None;
            unsafe {
                self.device
                    .CreateTexture2D(&staging_desc, None, Some(&mut staging))
                    .map_err(|e| {
                        CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(e.to_string())))
                    })?;
            }
            self.staging_texture = staging;
        }
        self.staging_texture.as_ref().ok_or_else(|| {
            CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(
                "Staging texture is missing".to_owned(),
            )))
        })
    }

    pub(crate) fn capture(
        &mut self,
        gray: &mut Vec<u8>,
    ) -> Result<(u32, u32, i32, i32), CoreError> {
        let staging_texture = self.ensure_staging_texture()?.clone();

        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;

            // Wait up to 10ms for a new frame (or 50ms on first frame)
            let timeout = if self.has_valid_frame { 10 } else { 50 };
            let acquire_result =
                self.duplication
                    .AcquireNextFrame(timeout, &mut frame_info, &mut resource);

            match acquire_result {
                Ok(()) => {
                    let Some(desktop_resource) = resource else {
                        let _ = self.duplication.ReleaseFrame();
                        return Ok((self.width, self.height, self.origin_x, self.origin_y));
                    };
                    let desktop_texture: ID3D11Texture2D =
                        desktop_resource.cast().map_err(|e| {
                            let _ = self.duplication.ReleaseFrame();
                            CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(
                                e.to_string(),
                            )))
                        })?;

                    // Fast GPU-to-GPU copy to staging texture
                    self.context
                        .CopyResource(&staging_texture, &desktop_texture);
                    let _ = self.duplication.ReleaseFrame();

                    // Map staging texture to CPU memory
                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    self.context
                        .Map(&staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                        .map_err(|e| {
                            CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(
                                e.to_string(),
                            )))
                        })?;

                    let row_pitch = mapped.RowPitch as usize;
                    let width = self.width as usize;
                    let height = self.height as usize;
                    gray.resize(width * height, 0);

                    let src_ptr = mapped.pData as *const u8;
                    for y in 0..height {
                        let row_src =
                            std::slice::from_raw_parts(src_ptr.add(y * row_pitch), width * 4);
                        let row_dst = &mut gray[y * width..(y + 1) * width];
                        for (pixel, out) in row_src.chunks_exact(4).zip(row_dst.iter_mut()) {
                            *out = rgb_pixel_to_gray(pixel[2], pixel[1], pixel[0]);
                        }
                    }

                    self.context.Unmap(&staging_texture, 0);
                    self.has_valid_frame = true;
                }
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    if !self.has_valid_frame {
                        return Err(CoreError::ScreenCapture(Box::new(
                            CoreError::CaptureBitmap("DXGI initial frame timeout".to_owned()),
                        )));
                    }
                    // Frame hasn't changed; existing grayscale buffer is valid
                }
                Err(e) => {
                    return Err(CoreError::ScreenCapture(Box::new(
                        CoreError::CaptureBitmap(e.to_string()),
                    )));
                }
            }

            Ok((self.width, self.height, self.origin_x, self.origin_y))
        }
    }

    pub fn capture_bgra(
        &mut self,
        bgra: &mut Vec<u8>,
    ) -> Result<(u32, u32, i32, i32), CoreError> {
        let staging_texture = self.ensure_staging_texture()?.clone();

        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource = None;

            let timeout = if self.has_valid_frame { 10 } else { 50 };
            let acquire_result =
                self.duplication
                    .AcquireNextFrame(timeout, &mut frame_info, &mut resource);

            match acquire_result {
                Ok(()) => {
                    let Some(desktop_resource) = resource else {
                        let _ = self.duplication.ReleaseFrame();
                        return Ok((self.width, self.height, self.origin_x, self.origin_y));
                    };
                    let desktop_texture: ID3D11Texture2D =
                        desktop_resource.cast().map_err(|e| {
                            let _ = self.duplication.ReleaseFrame();
                            CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(
                                e.to_string(),
                            )))
                        })?;

                    self.context
                        .CopyResource(&staging_texture, &desktop_texture);
                    let _ = self.duplication.ReleaseFrame();

                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    self.context
                        .Map(&staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                        .map_err(|e| {
                            CoreError::ScreenCapture(Box::new(CoreError::CaptureBitmap(
                                e.to_string(),
                            )))
                        })?;

                    let row_pitch = mapped.RowPitch as usize;
                    let width = self.width as usize;
                    let height = self.height as usize;
                    bgra.resize(width * height * 4, 0);

                    let src_ptr = mapped.pData as *const u8;
                    for y in 0..height {
                        let row_src =
                            std::slice::from_raw_parts(src_ptr.add(y * row_pitch), width * 4);
                        let row_dst = &mut bgra[y * width * 4..(y + 1) * width * 4];
                        row_dst.copy_from_slice(row_src);
                    }

                    self.context.Unmap(&staging_texture, 0);
                    self.has_valid_frame = true;
                }
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    if !self.has_valid_frame {
                        return Err(CoreError::ScreenCapture(Box::new(
                            CoreError::CaptureBitmap("DXGI initial frame timeout".to_owned()),
                        )));
                    }
                }
                Err(e) => {
                    return Err(CoreError::ScreenCapture(Box::new(
                        CoreError::CaptureBitmap(e.to_string()),
                    )));
                }
            }

            Ok((self.width, self.height, self.origin_x, self.origin_y))
        }
    }
}
