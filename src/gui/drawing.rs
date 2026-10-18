use super::{
    gdi::{GdiHandle, OwnedGdiObject},
    layout::scale,
    preferences::rgb,
    state::AppState,
};
use windows::Win32::{
    Foundation::{COLORREF, POINT},
    Graphics::Gdi::{
        CreatePen, CreateSolidBrush, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DrawTextW, FillRect,
        PS_SOLID, Polygon, RoundRect, SelectObject, SetBkColor, SetTextColor,
    },
    UI::{
        Controls::{DRAWITEMSTRUCT, ODS_SELECTED},
        WindowsAndMessaging::GetWindowTextW,
    },
};

pub(crate) unsafe fn draw_button(state: &AppState, item: &DRAWITEMSTRUCT) {
    let selected = item.itemState.0 & ODS_SELECTED.0 != 0;
    let (fill, text, border) = state
        .theme
        .button_colors(usize::try_from(item.CtlID).unwrap_or_default(), selected);
    let fill_brush = OwnedGdiObject::new(unsafe { CreateSolidBrush(COLORREF(fill)) });
    let border_pen = OwnedGdiObject::new(unsafe {
        CreatePen(PS_SOLID, scale(1, state.dpi_scale), COLORREF(border))
    });
    if fill_brush.is_null() || border_pen.is_null() {
        return;
    }

    unsafe {
        let old_brush = SelectObject(item.hDC, fill_brush.get().as_gdi_object());
        let old_pen = SelectObject(item.hDC, border_pen.get().as_gdi_object());
        let _ = RoundRect(
            item.hDC,
            item.rcItem.left + 1,
            item.rcItem.top + 1,
            item.rcItem.right - 1,
            item.rcItem.bottom - 1,
            scale(12, state.dpi_scale),
            scale(12, state.dpi_scale),
        );
        SelectObject(item.hDC, old_brush);
        SelectObject(item.hDC, old_pen);
        let mut text_rect = item.rcItem;
        text_rect.left += 8;
        text_rect.right -= 8;
        SetBkColor(item.hDC, COLORREF(fill));
        SetTextColor(item.hDC, COLORREF(text));
        let mut label = [0u16; 256];
        let length = GetWindowTextW(item.hwndItem, &mut label);
        if length > 0 {
            DrawTextW(
                item.hDC,
                &mut label[..length as usize],
                &mut text_rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
    }
}

pub(crate) unsafe fn draw_cursor_icon(state: &AppState, item: &DRAWITEMSTRUCT) {
    let x = item.rcItem.left;
    let y = item.rcItem.top;
    let width = item.rcItem.right - x;
    let height = item.rcItem.bottom - y;
    let outline = OwnedGdiObject::new(unsafe { CreateSolidBrush(COLORREF(rgb(52, 35, 24))) });
    let rust = OwnedGdiObject::new(unsafe { CreateSolidBrush(COLORREF(rgb(173, 83, 43))) });
    let outline_pen = OwnedGdiObject::new(unsafe {
        CreatePen(
            PS_SOLID,
            scale(1, state.dpi_scale),
            COLORREF(rgb(37, 27, 21)),
        )
    });
    if outline.is_null() || rust.is_null() || outline_pen.is_null() {
        return;
    }

    unsafe {
        if !state.background_brush.is_null() {
            let _ = FillRect(item.hDC, &item.rcItem, state.background_brush.get());
        }
        let old_brush = SelectObject(item.hDC, outline.get().as_gdi_object());
        let old_pen = SelectObject(item.hDC, outline_pen.get().as_gdi_object());
        let outer = [
            POINT { x: x + 3, y: y + 2 },
            POINT {
                x: x + 3,
                y: y + height - 2,
            },
            POINT {
                x: x + width / 3,
                y: y + height * 2 / 3,
            },
            POINT {
                x: x + width * 2 / 3,
                y: y + height - 3,
            },
            POINT {
                x: x + width * 4 / 5,
                y: y + height * 4 / 5,
            },
            POINT {
                x: x + width / 2,
                y: y + height * 3 / 5,
            },
            POINT { x: x + 3, y: y + 2 },
        ];
        let _ = Polygon(item.hDC, &outer);
        let inner = [
            POINT { x: x + 6, y: y + 7 },
            POINT {
                x: x + 6,
                y: y + height - 8,
            },
            POINT {
                x: x + width / 3,
                y: y + height * 2 / 3,
            },
            POINT {
                x: x + width * 2 / 3 - 3,
                y: y + height - 7,
            },
            POINT {
                x: x + width / 2,
                y: y + height * 3 / 5,
            },
            POINT { x: x + 6, y: y + 7 },
        ];
        SelectObject(item.hDC, rust.get().as_gdi_object());
        let _ = Polygon(item.hDC, &inner);
        SelectObject(item.hDC, old_brush);
        SelectObject(item.hDC, old_pen);
    }
}

pub(crate) unsafe fn draw_empty_preview(state: &AppState, item: &DRAWITEMSTRUCT) {
    let (_, text_color) = state.theme.colors();
    let mut text: Vec<u16> = "No image selected".encode_utf16().collect();
    let mut text_rect = item.rcItem;
    unsafe {
        let _ = FillRect(item.hDC, &item.rcItem, state.background_brush.get());
        SetBkColor(item.hDC, state.theme.colors().0);
        SetTextColor(item.hDC, text_color);
        DrawTextW(
            item.hDC,
            &mut text,
            &mut text_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}
