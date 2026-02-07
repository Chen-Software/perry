use std::cell::RefCell;

thread_local! {
    static MENU_COUNT: RefCell<i64> = RefCell::new(0);
}

/// Create a context menu (stub — UIContextMenuInteraction not yet implemented).
/// Returns a menu handle.
pub fn create() -> i64 {
    MENU_COUNT.with(|c| {
        let mut count = c.borrow_mut();
        *count += 1;
        *count
    })
}

/// Add an item to a context menu (stub).
pub fn add_item(_menu_handle: i64, _title_ptr: *const u8, _callback: f64) {
    // No-op on iOS for now
}

/// Set a context menu on a widget (stub).
pub fn set_context_menu(_widget_handle: i64, _menu_handle: i64) {
    // No-op on iOS for now
}
