//! A small rounded popup near the bottom of the screen ("Scroll",
//! "Spotify 40%"). It never takes focus and ignores the mouse.
//! Plain GDI for now; a nicer renderer can replace it later.

use std::cell::RefCell;

use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateRoundRectRgn,
    CreateSolidBrush, DEFAULT_CHARSET, DT_CENTER, DT_END_ELLIPSIS, DT_SINGLELINE, DT_VCENTER,
    DeleteObject, DrawTextW, EndPaint, FW_SEMIBOLD, FillRect, HFONT, InvalidateRect,
    OUT_DEFAULT_PRECIS, PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor, SetWindowRgn,
    TRANSPARENT,
};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, HWND_TOPMOST, KillTimer, LWA_ALPHA,
    RegisterClassW, SPI_GETWORKAREA, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetLayeredWindowAttributes, SetTimer, SetWindowPos,
    ShowWindow, SystemParametersInfoW, WM_PAINT, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{Result, w};

const WIDTH: i32 = 300;
const HEIGHT: i32 = 56;
const MARGIN_BOTTOM: i32 = 72;
const VISIBLE_MS: u32 = 1300;
const HIDE_TIMER: usize = 1;

const BACKGROUND: COLORREF = COLORREF(0x00_20_20_20); // 0x00BBGGRR
const TEXT: COLORREF = COLORREF(0x00_FF_FF_FF);

struct Osd {
    hwnd: HWND,
    font: HFONT,
    text: Vec<u16>,
    scale: f32,
}

thread_local! {
    static OSD: RefCell<Option<Osd>> = const { RefCell::new(None) };
}

fn scaled(value: i32, scale: f32) -> i32 {
    (value as f32 * scale).round() as i32
}

pub fn init(instance: HINSTANCE) -> Result<()> {
    unsafe {
        let class = w!("Ak820KnobOsd");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        let scale = GetDpiForSystem() as f32 / 96.0;
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
            class,
            w!(""),
            WS_POPUP,
            0,
            0,
            scaled(WIDTH, scale),
            scaled(HEIGHT, scale),
            None,
            None,
            Some(instance),
            None,
        )?;
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 235, LWA_ALPHA)?;
        let radius = scaled(18, scale);
        let region = CreateRoundRectRgn(
            0,
            0,
            scaled(WIDTH, scale) + 1,
            scaled(HEIGHT, scale) + 1,
            radius,
            radius,
        );
        SetWindowRgn(hwnd, Some(region), false);

        let font = CreateFontW(
            -scaled(22, scale),
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Segoe UI"),
        );
        OSD.with(|o| {
            *o.borrow_mut() = Some(Osd {
                hwnd,
                font,
                text: Vec::new(),
                scale,
            })
        });
    }
    Ok(())
}

/// Shows `text` for a moment. Must be called on the UI thread.
pub fn show(text: &str) {
    OSD.with(|o| {
        let mut o = o.borrow_mut();
        let Some(osd) = o.as_mut() else { return };
        osd.text = text.encode_utf16().collect();
        let (w, h) = (scaled(WIDTH, osd.scale), scaled(HEIGHT, osd.scale));
        unsafe {
            let mut area = RECT::default();
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut area as *mut RECT as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
            let x = (area.left + area.right - w) / 2;
            let y = area.bottom - h - scaled(MARGIN_BOTTOM, osd.scale);
            let _ = SetWindowPos(
                osd.hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                w,
                h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = InvalidateRect(Some(osd.hwnd), None, true);
            SetTimer(Some(osd.hwnd), HIDE_TIMER, VISIBLE_MS, None);
        }
    });
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    log::trace!("osd wndproc: 0x{msg:04X}");
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let brush = CreateSolidBrush(BACKGROUND);
                FillRect(hdc, &rect, brush);
                let _ = DeleteObject(brush.into());
                OSD.with(|o| {
                    // An empty slice has a dangling pointer, which DrawTextW
                    // dereferences: never draw before there is text.
                    if let Some(osd) = o.borrow_mut().as_mut().filter(|osd| !osd.text.is_empty()) {
                        SetBkMode(hdc, TRANSPARENT);
                        SetTextColor(hdc, TEXT);
                        let old = SelectObject(hdc, osd.font.into());
                        rect.left += scaled(16, osd.scale);
                        rect.right -= scaled(16, osd.scale);
                        let mut text = osd.text.clone();
                        DrawTextW(
                            hdc,
                            &mut text,
                            &mut rect,
                            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                        );
                        SelectObject(hdc, old);
                    }
                });
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == HIDE_TIMER => {
                let _ = KillTimer(Some(hwnd), HIDE_TIMER);
                let _ = ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
