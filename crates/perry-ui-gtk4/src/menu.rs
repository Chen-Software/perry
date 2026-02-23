use gtk4::prelude::*;
use gtk4::gio;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// Menu entries: each is a list of (title, callback_f64)
    static MENUS: RefCell<Vec<Vec<(String, f64)>>> = RefCell::new(Vec::new());
    /// Map from action name to closure pointer (f64 NaN-boxed)
    static MENU_ITEM_CALLBACKS: RefCell<HashMap<String, f64>> = RefCell::new(HashMap::new());
    /// Counter for generating unique action names
    static NEXT_ACTION_ID: RefCell<usize> = RefCell::new(1);
}

extern "C" {
    fn js_closure_call0(closure: *const u8) -> f64;
    fn js_nanbox_get_pointer(value: f64) -> i64;
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

/// Create a new context menu. Returns menu handle (1-based).
pub fn create() -> i64 {
    MENUS.with(|m| {
        let mut menus = m.borrow_mut();
        menus.push(Vec::new());
        menus.len() as i64
    })
}

/// Add an item to a menu with a title and callback.
pub fn add_item(menu_handle: i64, title_ptr: *const u8, callback: f64) {
    let title = str_from_header(title_ptr).to_string();
    MENUS.with(|m| {
        let mut menus = m.borrow_mut();
        let idx = (menu_handle - 1) as usize;
        if idx < menus.len() {
            menus[idx].push((title, callback));
        }
    });
}

/// Set a context menu on a widget. Right-click will show this menu.
pub fn set_context_menu(widget_handle: i64, menu_handle: i64) {
    if let Some(widget) = crate::widgets::get_widget(widget_handle) {
        // Build a GIO menu model from our stored menu items
        let gio_menu = gio::Menu::new();

        let items: Vec<(String, f64)> = MENUS.with(|m| {
            let menus = m.borrow();
            let idx = (menu_handle - 1) as usize;
            if idx < menus.len() {
                menus[idx].clone()
            } else {
                Vec::new()
            }
        });

        // Create an action group on the widget
        let action_group = gio::SimpleActionGroup::new();

        for (title, callback) in items {
            let action_name = NEXT_ACTION_ID.with(|id| {
                let mut id = id.borrow_mut();
                let name = format!("ctx{}", *id);
                *id += 1;
                name
            });

            MENU_ITEM_CALLBACKS.with(|cbs| {
                cbs.borrow_mut().insert(action_name.clone(), callback);
            });

            let action = gio::SimpleAction::new(&action_name, None);
            let action_name_clone = action_name.clone();
            action.connect_activate(move |_action, _param| {
                let closure_f64 = MENU_ITEM_CALLBACKS.with(|cbs| {
                    cbs.borrow().get(&action_name_clone).copied()
                });
                if let Some(closure_f64) = closure_f64 {
                    let closure_ptr = unsafe { js_nanbox_get_pointer(closure_f64) };
                    unsafe {
                        js_closure_call0(closure_ptr as *const u8);
                    }
                }
            });

            action_group.add_action(&action);
            gio_menu.append(Some(&title), Some(&format!("ctx.{}", action_name)));
        }

        widget.insert_action_group("ctx", Some(&action_group));

        // Create a PopoverMenu from the GIO menu and attach it
        let popover = gtk4::PopoverMenu::from_model(Some(&gio_menu));
        popover.set_parent(&widget);
        popover.set_has_arrow(false);

        // Attach a right-click gesture controller
        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3); // Right-click
        gesture.connect_pressed(move |gesture, _n_press, x, y| {
            let rect = gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            popover.set_pointing_to(Some(&rect));
            popover.popup();
            gesture.set_state(gtk4::EventSequenceState::Claimed);
        });
        widget.add_controller(gesture);
    }
}
