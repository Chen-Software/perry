use gtk4::prelude::*;
use gtk4::ProgressBar;

/// Create an indeterminate progress bar.
pub fn create() -> i64 {
    crate::app::ensure_gtk_init();
    let bar = ProgressBar::new();
    bar.pulse(); // Start in indeterminate mode
    bar.set_hexpand(true);
    super::register_widget(bar.upcast())
}

/// Set the progress value (0.0-1.0). Negative = indeterminate (pulse).
pub fn set_value(handle: i64, value: f64) {
    if let Some(widget) = super::get_widget(handle) {
        if let Some(bar) = widget.downcast_ref::<ProgressBar>() {
            if value < 0.0 {
                bar.pulse();
            } else {
                bar.set_fraction(value.clamp(0.0, 1.0));
            }
        }
    }
}
