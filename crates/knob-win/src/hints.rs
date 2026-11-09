//! Finds what can be clicked in a window, through Windows UI Automation.

use std::cell::RefCell;
use std::collections::HashSet;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationCondition, TreeScope_Descendants,
    UIA_BoundingRectanglePropertyId, UIA_ControlTypePropertyId, UIA_EditControlTypeId,
    UIA_HyperlinkControlTypeId, UIA_IsEnabledPropertyId,
    UIA_IsExpandCollapsePatternAvailablePropertyId, UIA_IsInvokePatternAvailablePropertyId,
    UIA_IsOffscreenPropertyId, UIA_IsSelectionItemPatternAvailablePropertyId,
    UIA_IsTogglePatternAvailablePropertyId, UIA_PROPERTY_ID,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetClassNameW, GetWindowRect, SMTO_ABORTIFHUNG, SendMessageTimeoutW,
    WM_GETOBJECT,
};
use windows::core::{BOOL, Result};

/// More labels than this can't be read anyway.
const MAX_TARGETS: usize = 400;
/// Chromium builds a page's accessibility tree only once something asks for
/// it, so the first look at a browser window waits for it to appear.
const CHROMIUM_POLLS: u32 = 12;
const CHROMIUM_POLL_WAIT: Duration = Duration::from_millis(150);
/// `UiaRootObjectId`: what a UI Automation client asks a window for.
const UIA_ROOT_OBJECT: isize = -25;
const OBJID_CLIENT: isize = -4;
const ACTIVATE_TIMEOUT_MS: u32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Target {
    pub fn center(&self) -> (i32, i32) {
        ((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }
}

/// Finds clickable items. Create it on the thread that uses it.
pub struct HintFinder {
    automation: IUIAutomation,
    condition: IUIAutomationCondition,
    /// Browser windows already asked to build their page tree.
    activated: RefCell<HashSet<isize>>,
}

impl HintFinder {
    pub fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            let condition = clickable_condition(&automation)?;
            Ok(Self {
                automation,
                condition,
                activated: RefCell::default(),
            })
        }
    }

    /// The clickable items in `hwnd` that are on screen.
    pub fn find(&self, hwnd: HWND) -> Result<Vec<Target>> {
        let started = Instant::now();
        let first_look = is_chromium(hwnd) && self.activated.borrow_mut().insert(hwnd.0 as isize);
        if first_look {
            activate_chromium(hwnd);
        }
        let mut targets = self.find_once(hwnd)?;
        if first_look {
            let toolbar_only = targets.len();
            for _ in 0..CHROMIUM_POLLS {
                std::thread::sleep(CHROMIUM_POLL_WAIT);
                let next = self.find_once(hwnd)?;
                let settled = next.len() > toolbar_only && next.len() == targets.len();
                targets = next;
                if settled {
                    break;
                }
            }
        }
        log::info!(
            "hints: {} targets in {} ms",
            targets.len(),
            started.elapsed().as_millis()
        );
        Ok(targets)
    }

    fn find_once(&self, hwnd: HWND) -> Result<Vec<Target>> {
        unsafe {
            let cache = self.automation.CreateCacheRequest()?;
            cache.AddProperty(UIA_BoundingRectanglePropertyId)?;
            let root = self.automation.ElementFromHandle(hwnd)?;
            let found = root.FindAllBuildCache(TreeScope_Descendants, &self.condition, &cache)?;

            let mut window = RECT::default();
            GetWindowRect(hwnd, &mut window)?;

            let mut rects = Vec::new();
            for i in 0..found.Length()? {
                if let Ok(rect) = found
                    .GetElement(i)
                    .and_then(|e| e.CachedBoundingRectangle())
                {
                    rects.push(Target {
                        left: rect.left,
                        top: rect.top,
                        right: rect.right,
                        bottom: rect.bottom,
                    });
                }
            }
            Ok(usable(rects, &window))
        }
    }
}

unsafe fn clickable_condition(automation: &IUIAutomation) -> Result<IUIAutomationCondition> {
    let flag = |id: UIA_PROPERTY_ID, value: bool| unsafe {
        automation.CreatePropertyCondition(id, &VARIANT::from(value))
    };
    let control = |id: i32| unsafe {
        automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(id))
    };
    unsafe {
        let mut any = flag(UIA_IsInvokePatternAvailablePropertyId, true)?;
        for next in [
            flag(UIA_IsTogglePatternAvailablePropertyId, true)?,
            flag(UIA_IsSelectionItemPatternAvailablePropertyId, true)?,
            flag(UIA_IsExpandCollapsePatternAvailablePropertyId, true)?,
            control(UIA_HyperlinkControlTypeId.0)?,
            control(UIA_EditControlTypeId.0)?,
        ] {
            any = automation.CreateOrCondition(&any, &next)?;
        }
        let visible = automation.CreateAndCondition(
            &flag(UIA_IsEnabledPropertyId, true)?,
            &flag(UIA_IsOffscreenPropertyId, false)?,
        )?;
        automation.CreateAndCondition(&any, &visible)
    }
}

/// Drops items that are too small, outside the window, or duplicates.
fn usable(rects: Vec<Target>, window: &RECT) -> Vec<Target> {
    let mut targets: Vec<Target> = Vec::new();
    for rect in rects {
        let (x, y) = rect.center();
        let inside = x >= window.left && x < window.right && y >= window.top && y < window.bottom;
        let big_enough = rect.right - rect.left >= 4 && rect.bottom - rect.top >= 4;
        let duplicate = targets.iter().any(|t| {
            let (tx, ty) = t.center();
            (tx - x).abs() <= 3 && (ty - y).abs() <= 3
        });
        if inside && big_enough && !duplicate {
            targets.push(rect);
            if targets.len() == MAX_TARGETS {
                break;
            }
        }
    }
    targets
}

fn class_name(hwnd: HWND) -> String {
    let mut name = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, &mut name) };
    String::from_utf16_lossy(&name[..len.max(0) as usize])
}

fn is_chromium(hwnd: HWND) -> bool {
    class_name(hwnd).starts_with("Chrome_WidgetWin")
}

/// Asks each of the browser's page widgets for its accessibility object, the
/// signal that makes Chromium build the tree.
fn activate_chromium(hwnd: HWND) {
    unsafe extern "system" fn visit(child: HWND, _: LPARAM) -> BOOL {
        if class_name(child).starts_with("Chrome_RenderWidgetHostHWND") {
            for object in [OBJID_CLIENT, UIA_ROOT_OBJECT] {
                unsafe {
                    SendMessageTimeoutW(
                        child,
                        WM_GETOBJECT,
                        WPARAM(0),
                        LPARAM(object),
                        SMTO_ABORTIFHUNG,
                        ACTIVATE_TIMEOUT_MS,
                        None,
                    );
                }
            }
        }
        BOOL(1)
    }
    unsafe {
        let _ = EnumChildWindows(Some(hwnd), Some(visit), LPARAM(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: RECT = RECT {
        left: 0,
        top: 0,
        right: 800,
        bottom: 600,
    };

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Target {
        Target {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn the_center_is_where_a_click_lands() {
        assert_eq!(rect(10, 20, 30, 60).center(), (20, 40));
    }

    #[test]
    fn items_outside_the_window_are_dropped() {
        let kept = usable(vec![rect(900, 10, 950, 40), rect(10, 10, 50, 40)], &WINDOW);
        assert_eq!(kept, [rect(10, 10, 50, 40)]);
    }

    #[test]
    fn tiny_items_are_dropped() {
        assert!(usable(vec![rect(10, 10, 12, 40)], &WINDOW).is_empty());
    }

    #[test]
    fn nested_items_with_the_same_center_get_one_label() {
        let kept = usable(vec![rect(10, 10, 50, 50), rect(20, 20, 40, 40)], &WINDOW);
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn the_number_of_labels_is_capped() {
        let many = (0..700)
            .map(|i| rect(i, i % 590, i + 8, i % 590 + 8))
            .map(|r| Target {
                left: r.left % 790,
                right: r.right % 790 + 8,
                ..r
            })
            .collect();
        assert!(usable(many, &WINDOW).len() <= MAX_TARGETS);
    }
}
