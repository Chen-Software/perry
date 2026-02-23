pub mod text;
pub mod button;
pub mod vstack;
pub mod hstack;
pub mod spacer;
pub mod divider;
pub mod textfield;
pub mod toggle;
pub mod slider;
pub mod scrollview;

use gtk4::prelude::*;
use gtk4::Widget;
use std::cell::RefCell;

thread_local! {
    /// Map from widget handle (1-based) to gtk4::Widget
    static WIDGETS: RefCell<Vec<Widget>> = RefCell::new(Vec::new());
}

/// Store a widget and return its handle (1-based i64).
pub fn register_widget(widget: Widget) -> i64 {
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        widgets.push(widget);
        widgets.len() as i64
    })
}

/// Retrieve the Widget for a given handle.
pub fn get_widget(handle: i64) -> Option<Widget> {
    WIDGETS.with(|w| {
        let widgets = w.borrow();
        let idx = (handle - 1) as usize;
        widgets.get(idx).cloned()
    })
}

/// Set the hidden state of a widget.
pub fn set_hidden(handle: i64, hidden: bool) {
    if let Some(widget) = get_widget(handle) {
        widget.set_visible(!hidden);
    }
}

/// Remove all children from a container (GtkBox).
pub fn clear_children(handle: i64) {
    if let Some(parent) = get_widget(handle) {
        if let Some(container) = parent.downcast_ref::<gtk4::Box>() {
            // Remove all children
            while let Some(child) = container.first_child() {
                container.remove(&child);
            }
        } else if let Some(scrolled) = parent.downcast_ref::<gtk4::ScrolledWindow>() {
            // ScrolledWindow has a single child
            scrolled.set_child(None::<&Widget>);
        }
    }
}

/// Add a child widget to a parent widget at a specific index.
pub fn add_child_at(parent_handle: i64, child_handle: i64, index: i64) {
    if let (Some(parent), Some(child)) = (get_widget(parent_handle), get_widget(child_handle)) {
        if let Some(container) = parent.downcast_ref::<gtk4::Box>() {
            // Find the child currently at the index position
            let mut i = 0;
            let mut sibling = container.first_child();
            while i < index {
                if let Some(s) = sibling {
                    sibling = s.next_sibling();
                } else {
                    break;
                }
                i += 1;
            }
            // Insert before the found sibling
            if let Some(before) = sibling {
                child.insert_before(container, Some(&before));
            } else {
                container.append(&child);
            }
        } else {
            // Fallback: just append
            if let Some(container) = parent.downcast_ref::<gtk4::Box>() {
                container.append(&child);
            }
        }
    }
}

/// Add a child view to a parent view.
/// If the parent is a GtkBox, appends the child.
pub fn add_child(parent_handle: i64, child_handle: i64) {
    if let (Some(parent), Some(child)) = (get_widget(parent_handle), get_widget(child_handle)) {
        if let Some(container) = parent.downcast_ref::<gtk4::Box>() {
            container.append(&child);
        } else if let Some(scrolled) = parent.downcast_ref::<gtk4::ScrolledWindow>() {
            scrolled.set_child(Some(&child));
        } else {
            // Generic fallback: try to set as child if parent supports it
            // This shouldn't normally happen with Perry's UI model
            eprintln!("perry-ui-gtk4: add_child called on unsupported parent type");
        }
    }
}
