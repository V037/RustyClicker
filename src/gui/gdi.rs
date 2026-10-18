use windows::Win32::Graphics::Gdi::{DeleteObject, HBITMAP, HBRUSH, HFONT, HGDIOBJ, HPEN};

pub(crate) trait GdiHandle: Copy + Default {
    fn is_null(self) -> bool;
    fn as_gdi_object(self) -> HGDIOBJ;
}

macro_rules! impl_gdi_handle {
    ($($handle:ty),+ $(,)?) => {
        $(
            impl GdiHandle for $handle {
                fn is_null(self) -> bool {
                    self.0.is_null()
                }

                fn as_gdi_object(self) -> HGDIOBJ {
                    HGDIOBJ(self.0)
                }
            }
        )+
    };
}

impl_gdi_handle!(HBRUSH, HBITMAP, HFONT, HPEN);

pub(crate) struct OwnedGdiObject<T: GdiHandle> {
    handle: T,
}

impl<T: GdiHandle> OwnedGdiObject<T> {
    pub(crate) fn new(handle: T) -> Self {
        Self { handle }
    }

    pub(crate) fn get(&self) -> T {
        self.handle
    }

    pub(crate) fn is_null(&self) -> bool {
        self.handle.is_null()
    }
}

impl<T: GdiHandle> Default for OwnedGdiObject<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: GdiHandle> Drop for OwnedGdiObject<T> {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                let _ = DeleteObject(self.handle.as_gdi_object());
            }
        }
    }
}
