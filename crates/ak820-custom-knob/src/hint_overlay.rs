//! Draws the click-hint labels over the screen as one per-pixel-alpha window,
//! which gives them smooth corners, translucency and soft shadows. The window
//! never takes focus and lets the mouse through; keys go through the input hook.

use std::cell::RefCell;
use std::ffi::c_void;

use knob_core::hint_style::{Canvas, HintPosition, HintStyle, LabelBox, parse_color, place_labels};
use knob_win::hints::Target;
use windows::Win32::Foundation::{
    COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER,
    BLENDFUNCTION, CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection, CreateFontW,
    DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteDC,
    DeleteObject, DrawTextW, FW_SEMIBOLD, GetTextExtentPoint32W, HBITMAP, HDC, HFONT, HGDIOBJ,
    OUT_DEFAULT_PRECIS, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetSystemMetrics, HWND_TOPMOST, RegisterClassW,
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_HIDE,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP,
};
use windows::core::{Result, w};

const PAD_X: f32 = 6.0;
const PAD_Y: f32 = 2.0;
const GAP: f32 = 2.0;
const SHADOW_BLUR: f32 = 6.0;
const SHADOW_ALPHA: f32 = 0.38;
const BORDER_ALPHA: f32 = 0.16;

/// A 32-bit top-down bitmap selected into its own device context.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u32,
    width: i32,
    height: i32,
}

impl Surface {
    unsafe fn new(width: i32, height: i32) -> Option<Self> {
        unsafe {
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // negative: top row first
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let dc = CreateCompatibleDC(None);
            let mut bits: *mut c_void = std::ptr::null_mut();
            let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            else {
                let _ = DeleteDC(dc);
                return None;
            };
            let previous = SelectObject(dc, bitmap.into());
            Some(Self {
                dc,
                bitmap,
                previous,
                bits: bits.cast(),
                width,
                height,
            })
        }
    }

    fn pixels(&mut self) -> &mut [u32] {
        // The bitmap holds exactly width × height pixels and lives as long as self.
        unsafe { std::slice::from_raw_parts_mut(self.bits, (self.width * self.height) as usize) }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

/// A label ready to draw.
struct Label {
    text: String,
    place: LabelBox,
    /// Text coverage for each pixel of the label.
    mask: Vec<u8>,
    /// Where each letter ends, measured from the label's left edge.
    letter_ends: Vec<i32>,
}

struct Colors {
    background: [u8; 3],
    text: [u8; 3],
    typed: [u8; 3],
    border: [u8; 3],
    opacity: f32,
    radius: f32,
    shadow: bool,
}

impl Colors {
    fn new(style: &HintStyle, scale: f32) -> Self {
        let color = |text: &str, fallback| parse_color(text).unwrap_or(fallback);
        let background = color(&style.background, [23, 27, 36]);
        // A faint edge that stands out from the label's own colour.
        let light = background.iter().map(|&c| u32::from(c)).sum::<u32>() > 384;
        Self {
            background,
            text: color(&style.text, [255, 255, 255]),
            typed: color(&style.typed, [122, 167, 255]),
            border: if light { [0, 0, 0] } else { [255, 255, 255] },
            opacity: style.opacity as f32 / 100.0,
            radius: style.radius as f32 * scale,
            shadow: style.shadow,
        }
    }
}

struct Overlay {
    hwnd: HWND,
    scale: f32,
    origin: (i32, i32),
    surface: Option<Surface>,
    labels: Vec<Label>,
    colors: Option<Colors>,
    typed: String,
}

thread_local! {
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

fn scaled(value: f32, scale: f32) -> i32 {
    (value * scale).round() as i32
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
        OVERLAY.with(|o| {
            *o.borrow_mut() = Some(Overlay {
                hwnd,
                scale: GetDpiForSystem() as f32 / 96.0,
                origin: (0, 0),
                surface: None,
                labels: Vec::new(),
                colors: None,
                typed: String::new(),
            })
        });
    }
    Ok(())
}

unsafe fn create_font(size: u32, scale: f32) -> HFONT {
    unsafe {
        CreateFontW(
            -scaled(size as f32, scale),
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
            ANTIALIASED_QUALITY,
            0,
            w!("Segoe UI"),
        )
    }
}

/// Draws text white on black and returns how much each pixel is covered.
unsafe fn text_mask(font: HFONT, text: &[u16], width: i32, height: i32) -> Vec<u8> {
    unsafe {
        let Some(mut surface) = Surface::new(width, height) else {
            return vec![0; (width * height) as usize];
        };
        SetBkMode(surface.dc, TRANSPARENT);
        SetTextColor(surface.dc, COLORREF(0x00FF_FFFF));
        let previous = SelectObject(surface.dc, font.into());
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        let mut owned = text.to_vec();
        DrawTextW(
            surface.dc,
            &mut owned,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(surface.dc, previous);
        surface.pixels().iter().map(|p| (*p & 0xFF) as u8).collect()
    }
}

/// Sizes and places the labels, and prepares their text.
unsafe fn build_labels(
    items: &[(String, Target)],
    style: &HintStyle,
    scale: f32,
    origin: (i32, i32),
    size: (i32, i32),
) -> Vec<Label> {
    unsafe {
        let font = create_font(style.size, scale);
        let dc = CreateCompatibleDC(None);
        let previous = SelectObject(dc, font.into());
        let measure = |text: &[u16]| {
            let mut extent = SIZE::default();
            let _ = GetTextExtentPoint32W(dc, text, &mut extent);
            extent
        };

        let mut boxes = Vec::with_capacity(items.len());
        let mut letter_ends = Vec::with_capacity(items.len());
        for (label, target) in items {
            let wide: Vec<u16> = label.encode_utf16().collect();
            let extent = measure(&wide);
            let h = extent.cy + scaled(PAD_Y * 2.0, scale);
            // At least square, so that single letters look even.
            let w = (extent.cx + scaled(PAD_X * 2.0, scale)).max(h);
            let (cx, cy) = target.center();
            let x = match style.position {
                HintPosition::Left => target.left,
                HintPosition::Center => cx - w / 2,
            } - origin.0;
            boxes.push(LabelBox {
                x,
                y: cy - origin.1 - h / 2,
                w,
                h,
            });
            let lead = (w - extent.cx) / 2;
            letter_ends.push(
                (1..=wide.len())
                    .map(|n| lead + measure(&wide[..n]).cx)
                    .collect::<Vec<i32>>(),
            );
        }
        SelectObject(dc, previous);
        let _ = DeleteDC(dc);

        let placed = place_labels(&boxes, size.0, size.1, scaled(GAP, scale));
        let labels = items
            .iter()
            .zip(placed)
            .zip(letter_ends)
            .map(|(((label, _), place), letter_ends)| {
                let wide: Vec<u16> = label.encode_utf16().collect();
                Label {
                    text: label.clone(),
                    place,
                    mask: text_mask(font, &wide, place.w, place.h),
                    letter_ends,
                }
            })
            .collect();
        let _ = DeleteObject(font.into());
        labels
    }
}

fn compose(canvas: &mut Canvas, labels: &[Label], colors: &Colors, typed: &str, scale: f32) {
    canvas.clear();
    for label in labels.iter().filter(|l| l.text.starts_with(typed)) {
        if colors.shadow {
            canvas.shadow(
                label.place,
                colors.radius,
                SHADOW_BLUR * scale,
                SHADOW_ALPHA,
            );
        }
        canvas.fill_rounded(
            label.place,
            colors.radius,
            colors.background,
            colors.opacity,
        );
        canvas.outline_rounded(label.place, colors.radius, colors.border, BORDER_ALPHA);
        let split = match typed.len() {
            0 => 0,
            n => label.letter_ends.get(n - 1).copied().unwrap_or(0),
        };
        canvas.draw_mask(
            label.place.x,
            label.place.y,
            label.place.w,
            &label.mask,
            split,
            colors.typed,
            colors.text,
        );
    }
}

fn present(overlay: &mut Overlay) {
    let hwnd = overlay.hwnd;
    let origin = POINT {
        x: overlay.origin.0,
        y: overlay.origin.1,
    };
    let (Some(surface), Some(colors)) = (overlay.surface.as_mut(), overlay.colors.as_ref()) else {
        return;
    };
    let (width, height) = (surface.width, surface.height);
    let mut canvas = Canvas::new(surface.pixels(), width, height);
    compose(
        &mut canvas,
        &overlay.labels,
        colors,
        &overlay.typed,
        overlay.scale,
    );
    let size = SIZE {
        cx: width,
        cy: height,
    };
    let source = POINT::default();
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    unsafe {
        if let Err(e) = UpdateLayeredWindow(
            hwnd,
            None,
            Some(&origin),
            Some(&size),
            Some(surface.dc),
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        ) {
            log::warn!("hints: cannot draw labels: {e}");
        }
    }
}

pub fn show(items: Vec<(String, Target)>, style: HintStyle) {
    OVERLAY.with(|o| {
        let mut o = o.borrow_mut();
        let Some(overlay) = o.as_mut() else { return };
        unsafe {
            overlay.origin = (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
            );
            let size = (
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            );
            overlay.surface = Surface::new(size.0, size.1);
            overlay.labels = build_labels(&items, &style, overlay.scale, overlay.origin, size);
            overlay.colors = Some(Colors::new(&style, overlay.scale));
            overlay.typed.clear();
            present(overlay);
            let _ = SetWindowPos(
                overlay.hwnd,
                Some(HWND_TOPMOST),
                overlay.origin.0,
                overlay.origin.1,
                size.0,
                size.1,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    });
}

pub fn typed(text: String) {
    OVERLAY.with(|o| {
        let mut o = o.borrow_mut();
        let Some(overlay) = o.as_mut() else { return };
        overlay.typed = text;
        present(overlay);
    });
}

pub fn hide() {
    OVERLAY.with(|o| {
        let mut o = o.borrow_mut();
        let Some(overlay) = o.as_mut() else { return };
        unsafe {
            let _ = ShowWindow(overlay.hwnd, SW_HIDE);
        }
        overlay.labels.clear();
        overlay.typed.clear();
        overlay.surface = None; // the buffer can be large; don't keep it around
    });
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
