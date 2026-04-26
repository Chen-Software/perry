use gtk4::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static SHEETS: RefCell<HashMap<i64, gtk4::Window>> = RefCell::new(HashMap::new());
    static NEXT_SHEET_ID: RefCell<i64> = RefCell::new(1);
}

fn str_from_header(ptr: *const u8) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    unsafe {
        let header = ptr as *const perry_runtime::string::StringHeader;
        let len = (*header).byte_len as usize;
        let data = ptr.add(std::mem::size_of::<perry_runtime::string::StringHeader>());
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(data, len))
    }
}

/// Create a sheet (modal window). title_val is a NaN-boxed string (or 0 for no title).
pub fn create(width: f64, height: f64, title_val: f64) -> i64 {
    crate::app::ensure_gtk_init();

    // Extract title from NaN-boxed value
    let title = {
        extern "C" {
            fn js_get_string_pointer_unified(value: f64) -> *const u8;
        }
        let ptr = unsafe { js_get_string_pointer_unified(title_val) };
        if ptr.is_null() {
            "Sheet".to_string()
        } else {
            str_from_header(ptr).to_string()
        }
    };

    let window = gtk4::Window::new();
    window.set_title(Some(&title));
    window.set_default_size(width as i32, height as i32);
    window.set_modal(true);
    window.set_resizable(true);

    let id = NEXT_SHEET_ID.with(|id| {
        let mut id = id.borrow_mut();
        let current = *id;
        *id += 1;
        current
    });

    SHEETS.with(|s| s.borrow_mut().insert(id, window));
    id
}

/// Present (show) a sheet.
pub fn present(sheet_handle: i64) {
    SHEETS.with(|s| {
        if let Some(window) = s.borrow().get(&sheet_handle) {
            // Try to set transient for the active GTK app window
            crate::app::GTK_APP.with(|ga| {
                if let Some(app) = ga.borrow().as_ref() {
                    if let Some(active) = app.active_window() {
                        window.set_transient_for(Some(&active));
                    }
                }
            });
            window.present();
        }
    });
}

/// Dismiss (close) a sheet.
pub fn dismiss(sheet_handle: i64) {
    SHEETS.with(|s| {
        if let Some(window) = s.borrow().get(&sheet_handle) {
            window.close();
        }
    });
}
