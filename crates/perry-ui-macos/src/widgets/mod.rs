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
pub mod canvas;

use objc2::rc::Retained;
use objc2::runtime::AnyClass;
use objc2_app_kit::{NSView, NSStackView};
use objc2_foundation::NSObjectProtocol;
use std::cell::RefCell;

thread_local! {
    /// Map from widget handle (1-based) to NSView
    static WIDGETS: RefCell<Vec<Retained<NSView>>> = RefCell::new(Vec::new());
}

/// Store an NSView and return its handle (1-based i64).
pub fn register_widget(view: Retained<NSView>) -> i64 {
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        widgets.push(view);
        widgets.len() as i64
    })
}

/// Retrieve the NSView for a given handle.
pub fn get_widget(handle: i64) -> Option<Retained<NSView>> {
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

/// Remove all arranged subviews from a container (NSStackView).
pub fn clear_children(handle: i64) {
    if let Some(parent) = get_widget(handle) {
        let is_stack = if let Some(cls) = AnyClass::get(c"NSStackView") {
            parent.isKindOfClass(cls)
        } else {
            false
        };
        if is_stack {
            let stack: &NSStackView = unsafe { &*(Retained::as_ptr(&parent) as *const NSStackView) };
            let subviews = stack.arrangedSubviews();
            for sv in subviews.iter() {
                stack.removeArrangedSubview(&*sv);
                sv.removeFromSuperview();
            }
        }
    }
}

/// Add a child view to a parent view at a specific index.
pub fn add_child_at(parent_handle: i64, child_handle: i64, index: i64) {
    if let (Some(parent), Some(child)) = (get_widget(parent_handle), get_widget(child_handle)) {
        let is_stack = if let Some(cls) = AnyClass::get(c"NSStackView") {
            parent.isKindOfClass(cls)
        } else {
            false
        };

        if is_stack {
            let stack: &NSStackView = unsafe { &*(Retained::as_ptr(&parent) as *const NSStackView) };
            unsafe {
                let _: () = objc2::msg_send![stack, insertArrangedSubview: &*child, atIndex: index as usize];
            }
        } else {
            parent.addSubview(&child);
        }
    }
}

/// Add a child view to a parent view.
/// If the parent is an NSStackView, uses addArrangedSubview for proper layout.
pub fn add_child(parent_handle: i64, child_handle: i64) {
    if let (Some(parent), Some(child)) = (get_widget(parent_handle), get_widget(child_handle)) {
        // Check if parent is an NSStackView
        let is_stack = if let Some(cls) = AnyClass::get(c"NSStackView") {
            parent.isKindOfClass(cls)
        } else {
            false
        };

        if is_stack {
            // Safety: we verified the type with isKindOfClass
            let stack: &NSStackView = unsafe { &*(Retained::as_ptr(&parent) as *const NSStackView) };
            stack.addArrangedSubview(&child);
        } else {
            parent.addSubview(&child);
        }
    }
}

// =============================================================================
// Widget Styling (Background, Gradient, Corner Radius)
// =============================================================================

use objc2::runtime::AnyObject;
use std::ffi::c_void;

type CGFloat = f64;

extern "C" {
    fn CGColorCreateGenericRGB(r: CGFloat, g: CGFloat, b: CGFloat, a: CGFloat) -> *mut c_void;
    fn CGColorRelease(color: *mut c_void);
}

/// Set a solid background color on any widget via its layer.
pub fn set_background_color(handle: i64, r: f64, g: f64, b: f64, a: f64) {
    if let Some(view) = get_widget(handle) {
        unsafe {
            let _: () = objc2::msg_send![&*view, setWantsLayer: true];
            let layer: *mut AnyObject = objc2::msg_send![&*view, layer];
            if !layer.is_null() {
                let cg_color = CGColorCreateGenericRGB(r, g, b, a);
                let _: () = objc2::msg_send![layer, setBackgroundColor: cg_color];
                CGColorRelease(cg_color);
            }
        }
    }
}

/// Set a linear gradient background on any widget via CAGradientLayer.
pub fn set_background_gradient(
    handle: i64, r1: f64, g1: f64, b1: f64, a1: f64,
    r2: f64, g2: f64, b2: f64, a2: f64, direction: f64,
) {
    if let Some(view) = get_widget(handle) {
        unsafe {
            let _: () = objc2::msg_send![&*view, setWantsLayer: true];
            let layer: *mut AnyObject = objc2::msg_send![&*view, layer];
            if layer.is_null() { return; }

            // Remove any existing gradient sublayer (tagged by name "PerryGradient")
            let sublayers: *mut AnyObject = objc2::msg_send![layer, sublayers];
            if !sublayers.is_null() {
                let count: usize = objc2::msg_send![sublayers, count];
                // Iterate backwards to safely remove
                let mut i = count;
                while i > 0 {
                    i -= 1;
                    let sub: *mut AnyObject = objc2::msg_send![sublayers, objectAtIndex: i];
                    let name: *mut AnyObject = objc2::msg_send![sub, name];
                    if !name.is_null() {
                        let is_ours: bool = objc2::msg_send![name, isEqualToString:
                            &*objc2_foundation::NSString::from_str("PerryGradient")];
                        if is_ours {
                            let _: () = objc2::msg_send![sub, removeFromSuperlayer];
                        }
                    }
                }
            }

            // Create CAGradientLayer
            let gradient_cls = AnyClass::get(c"CAGradientLayer")
                .expect("CAGradientLayer class not found");
            let gradient: *mut AnyObject = objc2::msg_send![gradient_cls, layer];

            // Set name for later removal
            let name = objc2_foundation::NSString::from_str("PerryGradient");
            let _: () = objc2::msg_send![gradient, setName: &*name];

            // Set frame to match layer bounds
            let bounds: objc2_core_foundation::CGRect = objc2::msg_send![layer, bounds];
            let _: () = objc2::msg_send![gradient, setFrame: bounds];

            // Create colors array
            let color1 = CGColorCreateGenericRGB(r1, g1, b1, a1);
            let color2 = CGColorCreateGenericRGB(r2, g2, b2, a2);

            // Wrap in NSArray via obj-c id
            let colors: Retained<AnyObject> = {
                let arr_cls = AnyClass::get(c"NSMutableArray").unwrap();
                let arr: *mut AnyObject = objc2::msg_send![arr_cls, arrayWithCapacity: 2usize];
                let _: () = objc2::msg_send![arr, addObject: color1 as *mut AnyObject];
                let _: () = objc2::msg_send![arr, addObject: color2 as *mut AnyObject];
                Retained::retain(arr).unwrap()
            };

            let _: () = objc2::msg_send![gradient, setColors: &*colors];

            CGColorRelease(color1);
            CGColorRelease(color2);

            // Set direction
            if direction < 0.5 {
                // Vertical: top to bottom
                let start = objc2_core_foundation::CGPoint::new(0.5, 0.0);
                let end = objc2_core_foundation::CGPoint::new(0.5, 1.0);
                let _: () = objc2::msg_send![gradient, setStartPoint: start];
                let _: () = objc2::msg_send![gradient, setEndPoint: end];
            } else {
                // Horizontal: left to right
                let start = objc2_core_foundation::CGPoint::new(0.0, 0.5);
                let end = objc2_core_foundation::CGPoint::new(1.0, 0.5);
                let _: () = objc2::msg_send![gradient, setStartPoint: start];
                let _: () = objc2::msg_send![gradient, setEndPoint: end];
            }

            // Insert at index 0 (behind other sublayers)
            let _: () = objc2::msg_send![layer, insertSublayer: gradient, atIndex: 0u32];

            // Auto-resize gradient with the layer
            let mask: u32 = (1 << 1) | (1 << 4); // kCALayerWidthSizable | kCALayerHeightSizable
            let _: () = objc2::msg_send![gradient, setAutoresizingMask: mask];
        }
    }
}

/// Set corner radius on any widget via its layer.
pub fn set_corner_radius(handle: i64, radius: f64) {
    if let Some(view) = get_widget(handle) {
        unsafe {
            let _: () = objc2::msg_send![&*view, setWantsLayer: true];
            let layer: *mut AnyObject = objc2::msg_send![&*view, layer];
            if !layer.is_null() {
                let _: () = objc2::msg_send![layer, setCornerRadius: radius];
                let _: () = objc2::msg_send![layer, setMasksToBounds: true];
            }
        }
    }
}
