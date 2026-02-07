use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{MainThreadMarker, NSObject, NSString};
use objc2_ui_kit::{UIView, UIViewController, UIWindow};

use std::cell::RefCell;

use crate::widgets;

thread_local! {
    static PENDING_CONFIG: RefCell<Option<AppConfig>> = RefCell::new(None);
    static PENDING_BODY: RefCell<Option<i64>> = RefCell::new(None);
    static APPS: RefCell<Vec<AppEntry>> = RefCell::new(Vec::new());
}

struct AppConfig {
    title: String,
    _width: f64,
    _height: f64,
}

struct AppEntry {
    window: Retained<UIWindow>,
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

/// Create an app. Stores config in thread-local for deferred creation.
/// Returns app handle (i64).
pub fn app_create(title_ptr: *const u8, width: f64, height: f64) -> i64 {
    let title = if title_ptr.is_null() {
        "Perry App".to_string()
    } else {
        str_from_header(title_ptr).to_string()
    };

    let w = if width > 0.0 { width } else { 400.0 };
    let h = if height > 0.0 { height } else { 300.0 };

    PENDING_CONFIG.with(|c| {
        *c.borrow_mut() = Some(AppConfig {
            title,
            _width: w,
            _height: h,
        });
    });

    1 // Single app handle
}

/// Set the root widget (body) of the app.
pub fn app_set_body(_app_handle: i64, root_handle: i64) {
    PENDING_BODY.with(|b| {
        *b.borrow_mut() = Some(root_handle);
    });
}

/// Define the PerryAppDelegate class for UIApplicationDelegate protocol.
pub struct PerryAppDelegateIvars {}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "PerryAppDelegate"]
    #[ivars = PerryAppDelegateIvars]
    pub struct PerryAppDelegate;

    impl PerryAppDelegate {
        #[unsafe(method(application:didFinishLaunchingWithOptions:))]
        fn did_finish_launching(&self, _application: &AnyObject, _options: *const AnyObject) -> bool {
            unsafe {
                let mtm = MainThreadMarker::new().expect("perry/ui must run on the main thread");

                // Create UIWindow with the main screen bounds
                let screen: Retained<AnyObject> = msg_send![
                    objc2::runtime::AnyClass::get(c"UIScreen").unwrap(),
                    mainScreen
                ];
                let bounds: CGRect = msg_send![&*screen, bounds];
                let window = UIWindow::initWithFrame(UIWindow::alloc(mtm), bounds);

                // Create root UIViewController
                let vc: Retained<UIViewController> = msg_send![
                    objc2::runtime::AnyClass::get(c"UIViewController").unwrap(),
                    new
                ];

                // Set white background
                let white: Retained<AnyObject> = msg_send![
                    objc2::runtime::AnyClass::get(c"UIColor").unwrap(),
                    whiteColor
                ];
                let vc_view: Retained<UIView> = msg_send![&*vc, view];
                let _: () = msg_send![&*vc_view, setBackgroundColor: &*white];

                // Set window title (not visible on iOS, but stored)
                PENDING_CONFIG.with(|c| {
                    let _config = c.borrow();
                    // iOS doesn't have window titles, but we've stored the config
                });

                // Attach the root widget if set
                PENDING_BODY.with(|b| {
                    if let Some(root_handle) = b.borrow().as_ref() {
                        if let Some(root_view) = widgets::get_widget(*root_handle) {
                            let _: () = msg_send![&*root_view, setTranslatesAutoresizingMaskIntoConstraints: false];

                            vc_view.addSubview(&root_view);

                            // Pin root widget to safe area layout guide
                            let safe_area: *const AnyObject = msg_send![&*vc_view, safeAreaLayoutGuide];

                            let root_leading: *const AnyObject = msg_send![&*root_view, leadingAnchor];
                            let root_trailing: *const AnyObject = msg_send![&*root_view, trailingAnchor];
                            let root_top: *const AnyObject = msg_send![&*root_view, topAnchor];
                            let root_bottom: *const AnyObject = msg_send![&*root_view, bottomAnchor];

                            let safe_leading: *const AnyObject = msg_send![safe_area, leadingAnchor];
                            let safe_trailing: *const AnyObject = msg_send![safe_area, trailingAnchor];
                            let safe_top: *const AnyObject = msg_send![safe_area, topAnchor];
                            let safe_bottom: *const AnyObject = msg_send![safe_area, bottomAnchor];

                            let c1: Retained<AnyObject> = msg_send![root_leading, constraintEqualToAnchor: safe_leading];
                            let c2: Retained<AnyObject> = msg_send![root_trailing, constraintEqualToAnchor: safe_trailing];
                            let c3: Retained<AnyObject> = msg_send![root_top, constraintEqualToAnchor: safe_top];
                            let c4: Retained<AnyObject> = msg_send![root_bottom, constraintEqualToAnchor: safe_bottom];

                            let _: () = msg_send![&*c1, setActive: true];
                            let _: () = msg_send![&*c2, setActive: true];
                            let _: () = msg_send![&*c3, setActive: true];
                            let _: () = msg_send![&*c4, setActive: true];
                        }
                    }
                });

                window.setRootViewController(Some(&vc));
                window.makeKeyAndVisible();

                APPS.with(|a| {
                    a.borrow_mut().push(AppEntry { window });
                });
            }
            true
        }
    }
);

/// Run the iOS app event loop (calls UIApplicationMain, blocks forever).
pub fn app_run(_app_handle: i64) {
    unsafe {
        let argc = 0i32;
        let argv: *const *const u8 = std::ptr::null();
        let principal = std::ptr::null::<NSString>();
        let delegate_class_name = NSString::from_str("PerryAppDelegate");

        // UIApplicationMain(argc, argv, nil, @"PerryAppDelegate")
        extern "C" {
            fn UIApplicationMain(
                argc: i32,
                argv: *const *const u8,
                principalClassName: *const NSString,
                delegateClassName: *const NSString,
            ) -> i32;
        }

        UIApplicationMain(argc, argv, principal, &*delegate_class_name);
    }
}

/// Set minimum window size (no-op on iOS — windows are always full-screen).
pub fn set_min_size(_app_handle: i64, _w: f64, _h: f64) {
    // No-op on iOS
}

/// Set maximum window size (no-op on iOS — windows are always full-screen).
pub fn set_max_size(_app_handle: i64, _w: f64, _h: f64) {
    // No-op on iOS
}

/// Add a keyboard shortcut (stub on iOS — UIKeyCommand not yet implemented).
pub fn add_keyboard_shortcut(_key_ptr: *const u8, _modifiers: f64, _callback: f64) {
    // No-op on iOS for now
}
