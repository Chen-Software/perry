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
pub mod securefield;
pub mod progressview;
pub mod form;
pub mod zstack;
pub mod picker;
pub mod canvas;
pub mod navstack;
pub mod lazyvstack;
pub mod image;

use jni::objects::{GlobalRef, JObject, JValue};
use std::cell::RefCell;

use crate::jni_bridge;

extern "C" {
    fn __android_log_print(prio: i32, tag: *const u8, fmt: *const u8, ...) -> i32;
}

thread_local! {
    /// Map from widget handle (1-based) to Android View (JNI global ref).
    static WIDGETS: RefCell<Vec<GlobalRef>> = RefCell::new(Vec::new());
}

/// Store an Android View and return its handle (1-based i64).
pub fn register_widget(view: GlobalRef) -> i64 {
    WIDGETS.with(|w| {
        let mut widgets = w.borrow_mut();
        widgets.push(view);
        widgets.len() as i64
    })
}

/// Retrieve the JNI GlobalRef for a given widget handle.
pub fn get_widget(handle: i64) -> Option<GlobalRef> {
    WIDGETS.with(|w| {
        let widgets = w.borrow();
        let idx = (handle - 1) as usize;
        widgets.get(idx).cloned()
    })
}

/// Set the hidden state of a widget (View.VISIBLE=0, View.GONE=8).
pub fn set_hidden(handle: i64, hidden: bool) {
    if let Some(view_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let visibility = if hidden { 8i32 } else { 0i32 }; // View.GONE=8, View.VISIBLE=0
        let _ = env.call_method(
            view_ref.as_obj(),
            "setVisibility",
            "(I)V",
            &[JValue::Int(visibility)],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Remove all child views from a ViewGroup container.
pub fn clear_children(handle: i64) {
    unsafe {
        __android_log_print(
            3, b"PerryWidgets\0".as_ptr(),
            b"clear_children: handle=%lld\0".as_ptr(),
            handle,
        );
    }
    if let Some(parent_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let _ = env.call_method(
            parent_ref.as_obj(),
            "removeAllViews",
            "()V",
            &[],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Add a child view to a parent ViewGroup.
pub fn add_child(parent_handle: i64, child_handle: i64) {
    if let (Some(parent_ref), Some(child_ref)) = (get_widget(parent_handle), get_widget(child_handle)) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let _ = env.call_method(
            parent_ref.as_obj(),
            "addView",
            "(Landroid/view/View;)V",
            &[JValue::Object(child_ref.as_obj())],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Add a child view to a parent ViewGroup at a specific index.
pub fn add_child_at(parent_handle: i64, child_handle: i64, index: i64) {
    if let (Some(parent_ref), Some(child_ref)) = (get_widget(parent_handle), get_widget(child_handle)) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let _ = env.call_method(
            parent_ref.as_obj(),
            "addView",
            "(Landroid/view/View;I)V",
            &[JValue::Object(child_ref.as_obj()), JValue::Int(index as i32)],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Get the Activity context via PerryBridge.
pub fn get_activity<'a>(env: &mut jni::JNIEnv<'a>) -> JObject<'a> {
    let bridge_class = jni_bridge::with_cache(|c| {
        env.new_local_ref(c.perry_bridge_class.as_obj()).unwrap()
    });
    let bridge_cls: &jni::objects::JClass = (&bridge_class).into();
    let result = env.call_static_method(
        bridge_cls,
        "getActivity",
        "()Landroid/app/Activity;",
        &[],
    ).expect("Failed to get Activity");
    result.l().expect("Activity is not an object")
}

/// Set enabled/disabled on a widget.
pub fn set_enabled(handle: i64, enabled: bool) {
    if let Some(view_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let _ = env.call_method(
            view_ref.as_obj(),
            "setEnabled",
            "(Z)V",
            &[JValue::Bool(enabled as u8)],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Set tooltip (API 26+).
pub fn set_tooltip(handle: i64, text_ptr: *const u8) {
    let text = crate::app::str_from_header(text_ptr);
    if let Some(view_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let jstr = env.new_string(text).expect("tooltip string");
        let _ = env.call_method(
            view_ref.as_obj(),
            "setTooltipText",
            "(Ljava/lang/CharSequence;)V",
            &[JValue::Object(&jstr)],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Set control size (map to scale).
pub fn set_control_size(handle: i64, size: i64) {
    if let Some(view_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let scale = match size {
            0 => 0.75f32,  // mini
            1 => 0.85f32,  // small
            3 => 1.15f32,  // large
            _ => 1.0f32,   // regular
        };
        let _ = env.call_method(view_ref.as_obj(), "setScaleX", "(F)V", &[JValue::Float(scale)]);
        let _ = env.call_method(view_ref.as_obj(), "setScaleY", "(F)V", &[JValue::Float(scale)]);
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Set corner radius via GradientDrawable.
pub fn set_corner_radius(handle: i64, radius: f64) {
    if let Some(view_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(16);
        let gd = env.new_object("android/graphics/drawable/GradientDrawable", "()V", &[])
            .expect("GradientDrawable");
        let _ = env.call_method(&gd, "setCornerRadius", "(F)V", &[JValue::Float(radius as f32)]);
        // Set transparent fill
        let _ = env.call_method(&gd, "setColor", "(I)V", &[JValue::Int(0)]);
        let _ = env.call_method(
            view_ref.as_obj(),
            "setBackground",
            "(Landroid/graphics/drawable/Drawable;)V",
            &[JValue::Object(&gd)],
        );
        let _ = env.call_method(view_ref.as_obj(), "setClipToOutline", "(Z)V", &[JValue::Bool(1)]);
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Set background color.
pub fn set_background_color(handle: i64, r: f64, g: f64, b: f64, a: f64) {
    if let Some(view_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let ai = (a * 255.0) as u32;
        let ri = (r * 255.0) as u32;
        let gi = (g * 255.0) as u32;
        let bi = (b * 255.0) as u32;
        let color = ((ai << 24) | (ri << 16) | (gi << 8) | bi) as i32;
        let _ = env.call_method(
            view_ref.as_obj(),
            "setBackgroundColor",
            "(I)V",
            &[JValue::Int(color)],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Set background gradient.
pub fn set_background_gradient(handle: i64, r1: f64, g1: f64, b1: f64, a1: f64, r2: f64, g2: f64, b2: f64, a2: f64, direction: f64) {
    if let Some(view_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(16);

        let c1 = argb_color(a1, r1, g1, b1);
        let c2 = argb_color(a2, r2, g2, b2);

        let gd = env.new_object("android/graphics/drawable/GradientDrawable", "()V", &[])
            .expect("GradientDrawable");

        // Set colors
        let colors = env.new_int_array(2).expect("int array");
        let _ = env.set_int_array_region(&colors, 0, &[c1, c2]);
        let _ = env.call_method(
            &gd,
            "setColors",
            "([I)V",
            &[JValue::Object(&colors)],
        );

        // Set orientation
        let orient_name = if direction < 0.5 { "TOP_BOTTOM" } else { "LEFT_RIGHT" };
        let orient_class = env.find_class("android/graphics/drawable/GradientDrawable$Orientation")
            .expect("Orientation");
        let orient = env.get_static_field(
            &orient_class,
            orient_name,
            "Landroid/graphics/drawable/GradientDrawable$Orientation;",
        ).expect("orient").l().expect("orient obj");
        let _ = env.call_method(
            &gd,
            "setOrientation",
            "(Landroid/graphics/drawable/GradientDrawable$Orientation;)V",
            &[JValue::Object(&orient)],
        );

        let _ = env.call_method(
            view_ref.as_obj(),
            "setBackground",
            "(Landroid/graphics/drawable/Drawable;)V",
            &[JValue::Object(&gd)],
        );

        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Set on-hover callback (no-op on Android touch devices).
pub fn set_on_hover(_handle: i64, _callback: f64) {
    // No-op — hover events are uncommon on touch devices
}

/// Set double-click (double-tap) callback.
pub fn set_on_double_click(_handle: i64, _callback: f64) {
    // Would require GestureDetector setup via PerryBridge
    // No-op for now
}

/// Animate opacity.
pub fn animate_opacity(handle: i64, target: f64, duration_ms: f64) {
    if let Some(view_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let animator = env.call_method(view_ref.as_obj(), "animate", "()Landroid/view/ViewPropertyAnimator;", &[])
            .expect("animate").l().expect("animator");
        let _ = env.call_method(&animator, "alpha", "(F)Landroid/view/ViewPropertyAnimator;", &[JValue::Float(target as f32)]);
        let _ = env.call_method(&animator, "setDuration", "(J)Landroid/view/ViewPropertyAnimator;", &[JValue::Long(duration_ms as i64)]);
        let _ = env.call_method(&animator, "start", "()V", &[]);
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

/// Animate position.
pub fn animate_position(handle: i64, dx: f64, dy: f64, duration_ms: f64) {
    if let Some(view_ref) = get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);
        let animator = env.call_method(view_ref.as_obj(), "animate", "()Landroid/view/ViewPropertyAnimator;", &[])
            .expect("animate").l().expect("animator");
        let _ = env.call_method(&animator, "translationXBy", "(F)Landroid/view/ViewPropertyAnimator;", &[JValue::Float(dx as f32)]);
        let _ = env.call_method(&animator, "translationYBy", "(F)Landroid/view/ViewPropertyAnimator;", &[JValue::Float(dy as f32)]);
        let _ = env.call_method(&animator, "setDuration", "(J)Landroid/view/ViewPropertyAnimator;", &[JValue::Long(duration_ms as i64)]);
        let _ = env.call_method(&animator, "start", "()V", &[]);
        unsafe { env.pop_local_frame(&JObject::null()); }
    }
}

fn argb_color(a: f64, r: f64, g: f64, b: f64) -> i32 {
    let ai = (a * 255.0) as u32;
    let ri = (r * 255.0) as u32;
    let gi = (g * 255.0) as u32;
    let bi = (b * 255.0) as u32;
    ((ai << 24) | (ri << 16) | (gi << 8) | bi) as i32
}

/// Convert dp to pixels via PerryBridge.
pub fn dp_to_px(env: &mut jni::JNIEnv, dp: f32) -> i32 {
    let bridge_class = jni_bridge::with_cache(|c| {
        env.new_local_ref(c.perry_bridge_class.as_obj()).unwrap()
    });
    let bridge_cls: &jni::objects::JClass = (&bridge_class).into();
    let result = env.call_static_method(
        bridge_cls,
        "dpToPx",
        "(F)I",
        &[JValue::Float(dp)],
    ).expect("Failed to convert dp to px");
    result.i().expect("dpToPx did not return int")
}
