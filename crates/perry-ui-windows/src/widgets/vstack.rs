//! VStack widget — custom window class for vertical layout container

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::*;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{HBRUSH, FillRect};
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use super::{WidgetKind, register_widget_with_layout};

#[cfg(target_os = "windows")]
static VSTACK_CLASS_REGISTERED: std::sync::Once = std::sync::Once::new();

#[cfg(target_os = "windows")]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn ensure_class_registered() {
    VSTACK_CLASS_REGISTERED.call_once(|| {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let class_name = to_wide("PerryVStack");
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(container_wnd_proc),
                hInstance: HINSTANCE::from(hinstance),
                hbrBackground: HBRUSH(std::ptr::null_mut()), // transparent
                lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            RegisterClassExW(&wc);
        }
    });
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn container_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Forward child notifications to the top-level window so button clicks,
        // text color, and context menus are handled by the main wnd_proc.
        WM_COMMAND | WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CONTEXTMENU | WM_DRAWITEM => {
            if let Ok(parent) = GetParent(hwnd) {
                return SendMessageW(parent, msg, wparam, lparam);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_ERASEBKGND => {
            // Try own brush first, then walk parent chain for nearest background
            let handle = super::find_handle_by_hwnd(hwnd);
            let brush = if handle > 0 {
                super::get_bg_brush(handle)
            } else {
                None
            };
            let brush = brush.or_else(|| find_ancestor_brush(hwnd));
            if let Some(brush) = brush {
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let _ = FillRect(hdc, &rect, brush);
                return LRESULT(1);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_PAINT => {
            let handle = super::find_handle_by_hwnd(hwnd);
            let brush = if handle > 0 {
                super::get_bg_brush(handle)
            } else {
                None
            };
            let brush = brush.or_else(|| find_ancestor_brush(hwnd));
            if let Some(brush) = brush {
                let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
                let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                let _ = GetClientRect(hwnd, &mut rect);
                let _ = FillRect(hdc, &rect, brush);
                windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Walk the HWND parent chain to find the nearest ancestor with a background brush.
#[cfg(target_os = "windows")]
fn find_ancestor_brush(mut hwnd: HWND) -> Option<HBRUSH> {
    for _ in 0..10 { // max 10 levels deep
        if let Ok(parent) = unsafe { GetParent(hwnd) } {
            if parent.0.is_null() { break; }
            let parent_handle = super::find_handle_by_hwnd(parent);
            if parent_handle > 0 {
                if let Some(brush) = super::get_bg_brush(parent_handle) {
                    return Some(brush);
                }
            }
            hwnd = parent;
        } else {
            break;
        }
    }
    None
}

/// Create a VStack. Returns widget handle.
pub fn create(spacing: f64) -> i64 {
    create_with_insets(spacing, 0.0, 0.0, 0.0, 0.0)
}

/// Create a VStack with custom insets. Returns widget handle.
pub fn create_with_insets(spacing: f64, top: f64, left: f64, bottom: f64, right: f64) -> i64 {
    #[cfg(target_os = "windows")]
    {
        ensure_class_registered();
        let class_name = to_wide("PerryVStack");
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::PCWSTR(class_name.as_ptr()),
                windows::core::PCWSTR(to_wide("").as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
                0, 0, 100, 100,
                super::get_parking_hwnd(),
                None,
                HINSTANCE::from(hinstance),
                None,
            ).unwrap();

            register_widget_with_layout(hwnd, WidgetKind::VStack, spacing, (top, left, bottom, right))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        register_widget_with_layout(0, WidgetKind::VStack, spacing, (top, left, bottom, right))
    }
}
