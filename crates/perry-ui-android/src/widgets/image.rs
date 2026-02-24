//! Image — ImageView for file images and system icons

use jni::objects::JValue;
use crate::jni_bridge;

fn str_from_header(ptr: *const u8) -> &'static str {
    crate::app::str_from_header(ptr)
}

/// Create an image from a file path.
pub fn create_file(path_ptr: *const u8) -> i64 {
    let path = str_from_header(path_ptr);
    let mut env = jni_bridge::get_env();
    let _ = env.push_local_frame(32);

    let activity = super::get_activity(&mut env);
    let image_view = env.new_object(
        "android/widget/ImageView",
        "(Landroid/content/Context;)V",
        &[JValue::Object(&activity)],
    ).expect("Failed to create ImageView");

    // Try to load bitmap from file
    let jpath = env.new_string(path).expect("Failed to create JNI string");
    let bitmap = env.call_static_method(
        "android/graphics/BitmapFactory",
        "decodeFile",
        "(Ljava/lang/String;)Landroid/graphics/Bitmap;",
        &[JValue::Object(&jpath)],
    );

    if let Ok(bmp_val) = bitmap {
        if let Ok(bmp) = bmp_val.l() {
            if !bmp.is_null() {
                let _ = env.call_method(
                    &image_view,
                    "setImageBitmap",
                    "(Landroid/graphics/Bitmap;)V",
                    &[JValue::Object(&bmp)],
                );
            }
        }
    }

    let global = env.new_global_ref(image_view).expect("Failed to create global ref");
    let handle = super::register_widget(global);
    unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    handle
}

/// Create an image from a named system icon.
pub fn create_symbol(name_ptr: *const u8) -> i64 {
    let _name = str_from_header(name_ptr);
    let mut env = jni_bridge::get_env();
    let _ = env.push_local_frame(32);

    let activity = super::get_activity(&mut env);
    let image_view = env.new_object(
        "android/widget/ImageView",
        "(Landroid/content/Context;)V",
        &[JValue::Object(&activity)],
    ).expect("Failed to create ImageView");

    // Android doesn't have SF Symbols; create placeholder
    // Could use Material Icons via resource lookup, but keep it simple
    let global = env.new_global_ref(image_view).expect("Failed to create global ref");
    let handle = super::register_widget(global);
    unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    handle
}

/// Set the size of an image widget.
pub fn set_size(handle: i64, width: f64, height: f64) {
    if let Some(view_ref) = super::get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(16);

        let w = super::dp_to_px(&mut env, width as f32);
        let h = super::dp_to_px(&mut env, height as f32);

        // Create LayoutParams
        let params = env.new_object(
            "android/view/ViewGroup$LayoutParams",
            "(II)V",
            &[JValue::Int(w), JValue::Int(h)],
        ).expect("LayoutParams");

        let _ = env.call_method(
            view_ref.as_obj(),
            "setLayoutParams",
            "(Landroid/view/ViewGroup$LayoutParams;)V",
            &[JValue::Object(&params)],
        );

        unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    }
}

/// Set the tint color of an image.
pub fn set_tint(handle: i64, r: f64, g: f64, b: f64, a: f64) {
    if let Some(view_ref) = super::get_widget(handle) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(8);

        let ai = (a * 255.0) as u32;
        let ri = (r * 255.0) as u32;
        let gi = (g * 255.0) as u32;
        let bi = (b * 255.0) as u32;
        let color = ((ai << 24) | (ri << 16) | (gi << 8) | bi) as i32;

        // setColorFilter(int color, PorterDuff.Mode mode)
        let mode_class = env.find_class("android/graphics/PorterDuff$Mode").expect("PorterDuff$Mode");
        let src_in = env.get_static_field(
            &mode_class,
            "SRC_IN",
            "Landroid/graphics/PorterDuff$Mode;",
        ).expect("SRC_IN").l().expect("mode");

        let _ = env.call_method(
            view_ref.as_obj(),
            "setColorFilter",
            "(ILandroid/graphics/PorterDuff$Mode;)V",
            &[JValue::Int(color), JValue::Object(&src_in)],
        );

        unsafe { env.pop_local_frame(&jni::objects::JObject::null()); }
    }
}
