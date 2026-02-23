use gtk4::prelude::*;
use gtk4::Separator;
use gtk4::Orientation;

/// Create a horizontal separator line.
pub fn create() -> i64 {
    let separator = Separator::new(Orientation::Horizontal);
    super::register_widget(separator.upcast())
}
