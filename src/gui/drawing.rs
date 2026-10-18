use super::{
    ID_CLASSICAL_NAV_BUTTON, ID_IMAGES_NAV_BUTTON, ID_SETTINGS,
    gdi::{GdiHandle, OwnedGdiObject},
    layout::scale,
    preferences::rgb,
    state::{ActivePanel, AppState},
};
use windows::Win32::{
    Foundation::{COLORREF, POINT},
    Graphics::Gdi::{
        CreatePen, CreateSolidBrush, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DrawTextW, Ellipse,
        FillRect, LineTo, MoveToEx, PS_NULL, PS_SOLID, Polygon, RoundRect, SelectObject,
        SetBkColor, SetTextColor,
    },
    UI::{
        Controls::{DRAWITEMSTRUCT, ODS_SELECTED},
        WindowsAndMessaging::GetWindowTextW,
    },
};

pub(crate) unsafe fn draw_button(state: &AppState, item: &DRAWITEMSTRUCT) {
    let selected = item.itemState.0 & ODS_SELECTED.0 != 0;
    let ctl_id = usize::try_from(item.CtlID).unwrap_or_default();
    let (fill, text, border) = state.theme.button_colors(ctl_id, selected);
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
        let corner_radius = scale(10, state.dpi_scale);

        if ctl_id == ID_CLASSICAL_NAV_BUTTON {
            let _ = RoundRect(
                item.hDC,
                item.rcItem.left + 1,
                item.rcItem.top + 1,
                item.rcItem.right + corner_radius,
                item.rcItem.bottom - 1,
                corner_radius,
                corner_radius,
            );
            let _ = MoveToEx(item.hDC, item.rcItem.right - 1, item.rcItem.top + 1, None);
            let _ = LineTo(item.hDC, item.rcItem.right - 1, item.rcItem.bottom - 1);
        } else if ctl_id == ID_IMAGES_NAV_BUTTON {
            let _ = RoundRect(
                item.hDC,
                item.rcItem.left - corner_radius,
                item.rcItem.top + 1,
                item.rcItem.right - 1,
                item.rcItem.bottom - 1,
                corner_radius,
                corner_radius,
            );
        } else {
            let _ = RoundRect(
                item.hDC,
                item.rcItem.left + 1,
                item.rcItem.top + 1,
                item.rcItem.right - 1,
                item.rcItem.bottom - 1,
                scale(12, state.dpi_scale),
                scale(12, state.dpi_scale),
            );
        }

        SelectObject(item.hDC, old_brush);
        SelectObject(item.hDC, old_pen);

        if ctl_id == ID_SETTINGS && state.active_panel == ActivePanel::MainMenu {
            let cx = (item.rcItem.left + item.rcItem.right) / 2;
            let cy = (item.rcItem.top + item.rcItem.bottom) / 2;
            let r_outer = scale(9, state.dpi_scale) as f64;
            let r_mid = scale(6, state.dpi_scale) as f64;
            let r_hole = scale(3, state.dpi_scale);

            let mut pts = Vec::with_capacity(32);
            let tooth_count = 8;
            for i in 0..tooth_count {
                let base_angle = (i as f64) * std::f64::consts::PI * 2.0 / (tooth_count as f64);
                let half_tooth = std::f64::consts::PI / (tooth_count as f64 * 2.2);
                let half_tip = half_tooth * 0.65;

                let a0 = base_angle - half_tooth;
                pts.push(POINT {
                    x: cx + (r_mid * a0.cos()).round() as i32,
                    y: cy + (r_mid * a0.sin()).round() as i32,
                });
                let a1 = base_angle - half_tip;
                pts.push(POINT {
                    x: cx + (r_outer * a1.cos()).round() as i32,
                    y: cy + (r_outer * a1.sin()).round() as i32,
                });
                let a2 = base_angle + half_tip;
                pts.push(POINT {
                    x: cx + (r_outer * a2.cos()).round() as i32,
                    y: cy + (r_outer * a2.sin()).round() as i32,
                });
                let a3 = base_angle + half_tooth;
                pts.push(POINT {
                    x: cx + (r_mid * a3.cos()).round() as i32,
                    y: cy + (r_mid * a3.sin()).round() as i32,
                });
            }

            let gear_color = if selected {
                COLORREF(rgb(130, 130, 130))
            } else {
                COLORREF(rgb(165, 165, 165))
            };
            let gear_brush = OwnedGdiObject::new(CreateSolidBrush(gear_color));
            let null_pen = OwnedGdiObject::new(CreatePen(PS_NULL, 0, COLORREF(0)));
            if !gear_brush.is_null() && !null_pen.is_null() {
                let old_gear_brush = SelectObject(item.hDC, gear_brush.get().as_gdi_object());
                let old_null_pen = SelectObject(item.hDC, null_pen.get().as_gdi_object());
                let _ = Polygon(item.hDC, &pts);

                let old_hole_brush = SelectObject(item.hDC, fill_brush.get().as_gdi_object());
                let _ = Ellipse(
                    item.hDC,
                    cx - r_hole,
                    cy - r_hole,
                    cx + r_hole + 1,
                    cy + r_hole + 1,
                );
                SelectObject(item.hDC, old_hole_brush);
                SelectObject(item.hDC, old_gear_brush);
                SelectObject(item.hDC, old_null_pen);
            }
        } else {
            let mut text_rect = item.rcItem;
            text_rect.left += 4;
            text_rect.right -= 4;
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
