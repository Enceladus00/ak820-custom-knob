//! Draws the click-hint labels over the screen. The window never takes focus
//! and lets the mouse through; keys are handled by the input hook.

use std::cell::RefCell;

use knob_win::hints::Target;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET, DT_CENTER, DT_NOPREFIX,
    DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint, FW_BOLD, FillRect,
    GetTextExtentPoint32W, HFONT, InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT, RoundRect,
    SRCCOPY, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetSystemMetrics, HWND_TOPMOST, LWA_COLORKEY, RegisterClassW,
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_HIDE,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow, WM_PAINT,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP,
};
use windows::core::{Result, w};

const KEY: COLORREF = COLORREF(0x00_01_00_01); // transparent background (BBGGRR)
const LABEL: COLORREF = COLORREF(0x00_33_D6_FF);
const LABEL_TYPED: COLORREF = COLORREF(0x00_58_C8_4C);
const TEXT: COLORREF = COLORREF(0x00_10_10_10);
const FONT_SIZE: i32 = 15;
const PAD_X: i32 = 5;
const PAD_Y: i32 = 2;

struct Overlay {
    hwnd: HWND,
    font: HFONT,
    scale: f32,
    origin: (i32, i32),
    size: (i32, i32),
    labels: Vec<(String, Target)>,
    typed: String,
}

thread_local! {
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

fn scaled(value: i32, scale: f32) -> i32 {
    (value as f32 * scale).round() as i32
}

pub fn init(instance: HINSTANCE) -> Result<()> {
    unsafe {
        let class = w!("Ak820KnobHints");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
            class,
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(instance),
            None,
        )?;
        SetLayeredWindowAttributes(hwnd, KEY, 0, LWA_COLORKEY)?;
        let scale = GetDpiForSystem() as f32 / 96.0;
        let font = CreateFontW(
            -scaled(FONT_SIZE, scale),
            0,
            0,
            0,
            FW_BOLD.0 as i32,
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
        OVERLAY.with(|o| {
            *o.borrow_mut() = Some(Overlay {
                hwnd,
                font,
                scale,
                origin: (0, 0),
                size: (1, 1),
                labels: Vec::new(),
                typed: String::new(),
            })
        });
    }
    Ok(())
}

pub fn show(labels: Vec<(String, Target)>) {
    OVERLAY.with(|o| {
        let mut o = o.borrow_mut();
        let Some(overlay) = o.as_mut() else { return };
        overlay.labels = labels;
        overlay.typed.clear();
        unsafe {
            overlay.origin = (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
            );
            overlay.size = (
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            );
            let _ = SetWindowPos(
                overlay.hwnd,
                Some(HWND_TOPMOST),
                overlay.origin.0,
                overlay.origin.1,
                overlay.size.0,
                overlay.size.1,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = InvalidateRect(Some(overlay.hwnd), None, false);
        }
    });
}

pub fn typed(text: String) {
    OVERLAY.with(|o| {
        let mut o = o.borrow_mut();
        let Some(overlay) = o.as_mut() else { return };
        overlay.typed = text;
        unsafe {
            let _ = InvalidateRect(Some(overlay.hwnd), None, false);
        }
    });
}

pub fn hide() {
    OVERLAY.with(|o| {
        let mut o = o.borrow_mut();
        let Some(overlay) = o.as_mut() else { return };
        overlay.labels.clear();
        overlay.typed.clear();
        unsafe {
            let _ = ShowWindow(overlay.hwnd, SW_HIDE);
        }
    });
}

unsafe fn paint(overlay: &Overlay, hwnd: HWND) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let screen = BeginPaint(hwnd, &mut ps);
        let (width, height) = overlay.size;
        // Drawn off-screen first, so that the labels don't flicker as you type.
        let dc = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        let old_bitmap = SelectObject(dc, bitmap.into());
        let background = CreateSolidBrush(KEY);
        let full = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        FillRect(dc, &full, background);
        let _ = DeleteObject(background.into());

        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, TEXT);
        let old_font = SelectObject(dc, overlay.font.into());
        let normal = CreateSolidBrush(LABEL);
        let matched = CreateSolidBrush(LABEL_TYPED);

        for (label, target) in &overlay.labels {
            if !label.starts_with(&overlay.typed) {
                continue;
            }
            let wide: Vec<u16> = label.encode_utf16().collect();
            let mut size = Default::default();
            let _ = GetTextExtentPoint32W(dc, &wide, &mut size);
            let w = size.cx + scaled(PAD_X * 2, overlay.scale);
            let h = size.cy + scaled(PAD_Y * 2, overlay.scale);
            let (_, cy) = target.center();
            let left = (target.left - overlay.origin.0).clamp(0, (width - w).max(0));
            let top = (cy - overlay.origin.1 - h / 2).clamp(0, (height - h).max(0));
            let mut rect = RECT {
                left,
                top,
                right: left + w,
                bottom: top + h,
            };
            let brush = if overlay.typed.is_empty() {
                normal
            } else {
                matched
            };
            let old_brush = SelectObject(dc, brush.into());
            let radius = scaled(6, overlay.scale);
            let _ = RoundRect(
                dc,
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                radius,
                radius,
            );
            SelectObject(dc, old_brush);
            let mut text = wide;
            DrawTextW(
                dc,
                &mut text,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
        }

        let _ = DeleteObject(normal.into());
        let _ = DeleteObject(matched.into());
        SelectObject(dc, old_font);
        let _ = BitBlt(screen, 0, 0, width, height, Some(dc), 0, 0, SRCCOPY);
        SelectObject(dc, old_bitmap);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        let _ = EndPaint(hwnd, &ps);
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_PAINT {
            OVERLAY.with(|o| {
                if let Some(overlay) = o.borrow().as_ref() {
                    paint(overlay, hwnd);
                }
            });
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
