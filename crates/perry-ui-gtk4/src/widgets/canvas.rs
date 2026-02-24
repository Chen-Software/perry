use gtk4::prelude::*;
use gtk4::DrawingArea;
use std::cell::RefCell;
use std::collections::HashMap;

/// Drawing commands that are replayed in the draw function.
#[derive(Clone)]
enum DrawCmd {
    BeginPath,
    MoveTo(f64, f64),
    LineTo(f64, f64),
    Stroke(f64, f64, f64, f64, f64), // r, g, b, a, line_width
    FillGradient(f64, f64, f64, f64, f64, f64, f64, f64, f64), // r1,g1,b1,a1, r2,g2,b2,a2, direction
    Clear,
}

thread_local! {
    /// Map from canvas handle -> command buffer
    static CANVAS_CMDS: RefCell<HashMap<i64, Vec<DrawCmd>>> = RefCell::new(HashMap::new());
    /// Map from canvas handle -> DrawingArea reference for queue_draw
    static CANVAS_AREAS: RefCell<HashMap<i64, DrawingArea>> = RefCell::new(HashMap::new());
}

/// Create a canvas widget with the given dimensions.
pub fn create(width: f64, height: f64) -> i64 {
    crate::app::ensure_gtk_init();
    let area = DrawingArea::new();
    area.set_content_width(width as i32);
    area.set_content_height(height as i32);

    let handle = super::register_widget(area.clone().upcast());

    CANVAS_CMDS.with(|c| c.borrow_mut().insert(handle, Vec::new()));
    CANVAS_AREAS.with(|a| a.borrow_mut().insert(handle, area.clone()));

    let h = handle;
    area.set_draw_func(move |_area, cr, _w, _h| {
        CANVAS_CMDS.with(|c| {
            if let Some(cmds) = c.borrow().get(&h) {
                let mut path_x = 0.0;
                let mut path_y = 0.0;
                for cmd in cmds {
                    match cmd {
                        DrawCmd::Clear => {
                            // Clear is handled by GTK redraw
                        }
                        DrawCmd::BeginPath => {
                            cr.new_path();
                        }
                        DrawCmd::MoveTo(x, y) => {
                            cr.move_to(*x, *y);
                            path_x = *x;
                            path_y = *y;
                        }
                        DrawCmd::LineTo(x, y) => {
                            cr.line_to(*x, *y);
                            path_x = *x;
                            path_y = *y;
                        }
                        DrawCmd::Stroke(r, g, b, a, lw) => {
                            cr.set_source_rgba(*r, *g, *b, *a);
                            cr.set_line_width(*lw);
                            let _ = cr.stroke();
                        }
                        DrawCmd::FillGradient(r1, g1, b1, _a1, r2, g2, b2, _a2, direction) => {
                            let pattern = if *direction < 0.5 {
                                // Vertical gradient
                                cairo::LinearGradient::new(0.0, 0.0, 0.0, path_y)
                            } else {
                                // Horizontal gradient
                                cairo::LinearGradient::new(0.0, 0.0, path_x, 0.0)
                            };
                            pattern.add_color_stop_rgb(0.0, *r1, *g1, *b1);
                            pattern.add_color_stop_rgb(1.0, *r2, *g2, *b2);
                            let _ = cr.set_source(&pattern);
                            let _ = cr.fill();
                        }
                    }
                }
            }
        });
    });

    handle
}

fn queue_redraw(handle: i64) {
    CANVAS_AREAS.with(|a| {
        if let Some(area) = a.borrow().get(&handle) {
            area.queue_draw();
        }
    });
}

pub fn clear(handle: i64) {
    CANVAS_CMDS.with(|c| {
        if let Some(cmds) = c.borrow_mut().get_mut(&handle) {
            cmds.clear();
            cmds.push(DrawCmd::Clear);
        }
    });
    queue_redraw(handle);
}

pub fn begin_path(handle: i64) {
    CANVAS_CMDS.with(|c| {
        if let Some(cmds) = c.borrow_mut().get_mut(&handle) {
            cmds.push(DrawCmd::BeginPath);
        }
    });
}

pub fn move_to(handle: i64, x: f64, y: f64) {
    CANVAS_CMDS.with(|c| {
        if let Some(cmds) = c.borrow_mut().get_mut(&handle) {
            cmds.push(DrawCmd::MoveTo(x, y));
        }
    });
}

pub fn line_to(handle: i64, x: f64, y: f64) {
    CANVAS_CMDS.with(|c| {
        if let Some(cmds) = c.borrow_mut().get_mut(&handle) {
            cmds.push(DrawCmd::LineTo(x, y));
        }
    });
}

pub fn stroke(handle: i64, r: f64, g: f64, b: f64, a: f64, line_width: f64) {
    CANVAS_CMDS.with(|c| {
        if let Some(cmds) = c.borrow_mut().get_mut(&handle) {
            cmds.push(DrawCmd::Stroke(r, g, b, a, line_width));
        }
    });
    queue_redraw(handle);
}

pub fn fill_gradient(handle: i64, r1: f64, g1: f64, b1: f64, a1: f64, r2: f64, g2: f64, b2: f64, a2: f64, direction: f64) {
    CANVAS_CMDS.with(|c| {
        if let Some(cmds) = c.borrow_mut().get_mut(&handle) {
            cmds.push(DrawCmd::FillGradient(r1, g1, b1, a1, r2, g2, b2, a2, direction));
        }
    });
    queue_redraw(handle);
}
