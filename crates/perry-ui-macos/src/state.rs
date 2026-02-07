use std::cell::RefCell;
use std::collections::HashMap;

use crate::widgets;

struct StateEntry {
    value: f64,
}

struct TextBinding {
    text_handle: i64,
    prefix: String,
}

thread_local! {
    static STATES: RefCell<Vec<StateEntry>> = RefCell::new(Vec::new());
    /// Map from state_handle -> list of text bindings to update when state changes
    static TEXT_BINDINGS: RefCell<HashMap<i64, Vec<TextBinding>>> = RefCell::new(HashMap::new());
}

/// Extract a &str from a *const StringHeader pointer.
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

/// Create a new state cell with an initial value. Returns state handle (1-based).
pub fn state_create(initial: f64) -> i64 {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        states.push(StateEntry { value: initial });
        states.len() as i64 // 1-based handle
    })
}

/// Get the current value of a state cell.
pub fn state_get(handle: i64) -> f64 {
    STATES.with(|s| {
        let states = s.borrow();
        let idx = (handle - 1) as usize;
        if idx < states.len() {
            states[idx].value
        } else {
            f64::from_bits(0x7FFC_0000_0000_0001) // undefined
        }
    })
}

/// Set a new value on a state cell and update bound text widgets.
pub fn state_set(handle: i64, value: f64) {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        let idx = (handle - 1) as usize;
        if idx < states.len() {
            states[idx].value = value;
        }
    });

    // Update bound text widgets
    TEXT_BINDINGS.with(|b| {
        if let Some(bindings) = b.borrow().get(&handle) {
            for binding in bindings {
                // Format value like JavaScript: integers without decimal point
                let text = if value.fract() == 0.0 && value.abs() < 1e15 {
                    format!("{}{}", binding.prefix, value as i64)
                } else {
                    format!("{}{}", binding.prefix, value)
                };
                widgets::text::set_text_str(binding.text_handle, &text);
            }
        }
    });
}

/// Bind a text widget to a state cell with a prefix string.
/// When the state changes, the text widget will be updated to "{prefix}{value}".
pub fn bind_text_numeric(state_handle: i64, text_handle: i64, prefix_ptr: *const u8) {
    let prefix = str_from_header(prefix_ptr).to_string();
    TEXT_BINDINGS.with(|b| {
        b.borrow_mut()
            .entry(state_handle)
            .or_default()
            .push(TextBinding { text_handle, prefix });
    });
}
