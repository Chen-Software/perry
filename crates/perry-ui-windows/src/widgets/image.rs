//! Image widget — Win32 STATIC control with SS_BITMAP for file images,
//! or system icon display for symbol images.

use std::cell::RefCell;
use std::collections::HashMap;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::*;
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use super::{WidgetKind, alloc_control_id, register_widget};

fn str_from_header(ptr: *const u8) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    unsafe {
        let header = ptr as *const perry_runtime::string::StringHeader;
        let len = (*header).length as usize;
        let data = ptr.add(std::mem::size_of::<perry_runtime::string::StringHeader>());
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(data, len))
    }
}

#[cfg(target_os = "windows")]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// STM_SETIMAGE message
#[cfg(target_os = "windows")]
const STM_SETIMAGE: u32 = 0x0172;

/// Per-widget tint color (limited use on Win32 — stored for potential custom draw)
struct ImageTint {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

thread_local! {
    static IMAGE_TINTS: RefCell<HashMap<i64, ImageTint>> = RefCell::new(HashMap::new());
}

/// Create an Image from a file path. Returns widget handle.
pub fn create_file(path_ptr: *const u8) -> i64 {
    let path = str_from_header(path_ptr);
    let control_id = alloc_control_id();

    #[cfg(target_os = "windows")]
    {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::PCWSTR(to_wide("STATIC").as_ptr()),
                windows::core::PCWSTR(to_wide("").as_ptr()),
                WINDOW_STYLE(SS_BITMAP as u32 | WS_CHILD.0 | WS_VISIBLE.0),
                0, 0, 100, 100,
                None,
                HMENU(control_id as *mut _),
                Some(hinstance.into()),
                None,
            )
            .unwrap();

            // Load the image from file using LoadImageW
            let wide_path = to_wide(path);
            let hbitmap = LoadImageW(
                None,
                windows::core::PCWSTR(wide_path.as_ptr()),
                IMAGE_BITMAP,
                0, 0, // use actual size
                LR_LOADFROMFILE | LR_DEFAULTSIZE,
            );

            if let Ok(hbitmap) = hbitmap {
                SendMessageW(
                    hwnd,
                    STM_SETIMAGE,
                    WPARAM(IMAGE_BITMAP.0 as usize),
                    LPARAM(hbitmap.0 as isize),
                );
            }

            register_widget(hwnd, WidgetKind::Image, control_id)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        register_widget(0, WidgetKind::Image, control_id)
    }
}

/// Create an Image from a system symbol/icon name. Returns widget handle.
pub fn create_symbol(name_ptr: *const u8) -> i64 {
    let name = str_from_header(name_ptr);
    let control_id = alloc_control_id();

    #[cfg(target_os = "windows")]
    {
        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::PCWSTR(to_wide("STATIC").as_ptr()),
                windows::core::PCWSTR(to_wide("").as_ptr()),
                WINDOW_STYLE(SS_ICON as u32 | WS_CHILD.0 | WS_VISIBLE.0),
                0, 0, 32, 32,
                None,
                HMENU(control_id as *mut _),
                Some(hinstance.into()),
                None,
            )
            .unwrap();

            // Map common symbol names to system icons
            let icon_id = match name {
                "exclamationmark.triangle" | "warning" => IDI_WARNING,
                "info.circle" | "info" => IDI_INFORMATION,
                "xmark.circle" | "error" => IDI_ERROR,
                "questionmark.circle" | "question" => IDI_QUESTION,
                "app" | "application" => IDI_APPLICATION,
                "shield" | "shield.fill" => IDI_SHIELD,
                _ => IDI_APPLICATION,
            };

            let hicon = LoadIconW(None, icon_id);
            if let Ok(hicon) = hicon {
                SendMessageW(
                    hwnd,
                    STM_SETIMAGE,
                    WPARAM(IMAGE_ICON.0 as usize),
                    LPARAM(hicon.0 as isize),
                );
            }

            register_widget(hwnd, WidgetKind::Image, control_id)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = name;
        register_widget(0, WidgetKind::Image, control_id)
    }
}

/// Set the size of an Image widget.
pub fn set_size(handle: i64, width: f64, height: f64) {
    #[cfg(target_os = "windows")]
    {
        if let Some(hwnd) = super::get_hwnd(handle) {
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0, 0,
                    width as i32, height as i32,
                    SWP_NOMOVE | SWP_NOZORDER,
                );
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, width, height);
    }
}

/// Set the tint color for an Image widget.
/// On Win32, tinting is limited — we store the color for potential custom-draw use.
pub fn set_tint(handle: i64, r: f64, g: f64, b: f64, a: f64) {
    IMAGE_TINTS.with(|tints| {
        tints.borrow_mut().insert(handle, ImageTint {
            r: (r * 255.0) as u8,
            g: (g * 255.0) as u8,
            b: (b * 255.0) as u8,
            a: (a * 255.0) as u8,
        });
    });

    #[cfg(target_os = "windows")]
    {
        // Force repaint (custom-draw could use the tint if implemented)
        if let Some(hwnd) = super::get_hwnd(handle) {
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
    }
}
