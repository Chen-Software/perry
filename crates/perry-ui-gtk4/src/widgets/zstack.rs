use gtk4::prelude::*;
use gtk4::Overlay;

/// Create a ZStack (overlay container where children stack on top of each other).
pub fn create() -> i64 {
    crate::app::ensure_gtk_init();
    let overlay = Overlay::new();
    overlay.set_vexpand(true);
    overlay.set_hexpand(true);
    super::register_widget(overlay.upcast())
}
