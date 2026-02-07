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

use objc2::rc::Retained;
use objc2::runtime::AnyClass;
use objc2_foundation::NSObjectProtocol;
use objc2_ui_kit::{UIView, UIStackView};
use std::cell::RefCell;

thread_local! {
    /// Map from widget handle (1-based) to UIView
    static WIDGETS: RefCell<Vec<Retained<UIView>>> = RefCell::new(Vec::new());
}

/// Store a UIView and return its handle (1-based i64).
pub fn register_widget(view: Retained<UIView>) -> i64 {
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        widgets.push(view);
        widgets.len() as i64
    })
}

/// Retrieve the UIView for a given handle.
pub fn get_widget(handle: i64) -> Option<Retained<UIView>> {
    WIDGETS.with(|w| {
        let widgets = w.borrow();
        let idx = (handle - 1) as usize;
        widgets.get(idx).cloned()
    })
}

/// Set the hidden state of a widget.
pub fn set_hidden(handle: i64, hidden: bool) {
    if let Some(view) = get_widget(handle) {
        unsafe {
            let _: () = objc2::msg_send![&*view, setHidden: hidden];
        }
    }
}

/// Remove all arranged subviews from a container (UIStackView).
pub fn clear_children(handle: i64) {
    if let Some(parent) = get_widget(handle) {
        let is_stack = if let Some(cls) = AnyClass::get(c"UIStackView") {
            parent.isKindOfClass(cls)
        } else {
            false
        };
        if is_stack {
            let stack: &UIStackView = unsafe { &*(Retained::as_ptr(&parent) as *const UIStackView) };
            let subviews = stack.arrangedSubviews();
            for sv in subviews.iter() {
                unsafe {
                    let _: () = objc2::msg_send![stack, removeArrangedSubview: &**sv];
                    sv.removeFromSuperview();
                }
            }
        }
    }
}

/// Add a child view to a parent view at a specific index.
pub fn add_child_at(parent_handle: i64, child_handle: i64, index: i64) {
    if let (Some(parent), Some(child)) = (get_widget(parent_handle), get_widget(child_handle)) {
        let is_stack = if let Some(cls) = AnyClass::get(c"UIStackView") {
            parent.isKindOfClass(cls)
        } else {
            false
        };

        if is_stack {
            let stack: &UIStackView = unsafe { &*(Retained::as_ptr(&parent) as *const UIStackView) };
            unsafe {
                let _: () = objc2::msg_send![stack, insertArrangedSubview: &*child, atIndex: index as usize];
            }
        } else {
            parent.addSubview(&child);
        }
    }
}

/// Add a child view to a parent view.
/// If the parent is a UIStackView, uses addArrangedSubview for proper layout.
pub fn add_child(parent_handle: i64, child_handle: i64) {
    if let (Some(parent), Some(child)) = (get_widget(parent_handle), get_widget(child_handle)) {
        let is_stack = if let Some(cls) = AnyClass::get(c"UIStackView") {
            parent.isKindOfClass(cls)
        } else {
            false
        };

        if is_stack {
            let stack: &UIStackView = unsafe { &*(Retained::as_ptr(&parent) as *const UIStackView) };
            stack.addArrangedSubview(&child);
        } else {
            parent.addSubview(&child);
        }
    }
}
