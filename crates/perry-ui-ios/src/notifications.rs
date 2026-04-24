use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::NSString;

fn str_from_header(ptr: *const u8) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    unsafe {
        let header = ptr as *const perry_runtime::string::StringHeader;
        let len = (*header).byte_len as usize;
        let data = ptr.add(std::mem::size_of::<perry_runtime::string::StringHeader>());
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(data, len))
    }
}

/// Ask the user for alert + badge + sound permission (options bitmask = 7).
/// Called once from app bootstrap (`PerryAppDelegate.application:didFinishLaunchingWithOptions:`)
/// so the permission prompt fires at launch, not on every `notificationSend` call.
pub fn request_authorization() {
    unsafe {
        let Some(center_cls) = AnyClass::get(c"UNUserNotificationCenter") else {
            return;
        };
        let center: Retained<AnyObject> = msg_send![center_cls, currentNotificationCenter];
        let _: () = msg_send![
            &*center,
            requestAuthorizationWithOptions: 7i64,
            completionHandler: std::ptr::null::<AnyObject>()
        ];
    }
}

/// Send a local notification. Relies on authorization already having been granted
/// via `request_authorization()` at app bootstrap.
pub fn send(title_ptr: *const u8, body_ptr: *const u8) {
    let title = str_from_header(title_ptr);
    let body = str_from_header(body_ptr);

    unsafe {
        let Some(content_cls) = AnyClass::get(c"UNMutableNotificationContent") else {
            return;
        };
        let content: Retained<AnyObject> = msg_send![content_cls, new];

        let ns_title = NSString::from_str(title);
        let _: () = msg_send![&*content, setTitle: &*ns_title];

        let ns_body = NSString::from_str(body);
        let _: () = msg_send![&*content, setBody: &*ns_body];

        let Some(trigger_cls) = AnyClass::get(c"UNTimeIntervalNotificationTrigger") else {
            return;
        };
        let trigger: Retained<AnyObject> = msg_send![
            trigger_cls,
            triggerWithTimeInterval: 0.1f64,
            repeats: false
        ];

        let Some(request_cls) = AnyClass::get(c"UNNotificationRequest") else {
            return;
        };
        let ident = NSString::from_str("perry_notification");
        let request: Retained<AnyObject> = msg_send![
            request_cls,
            requestWithIdentifier: &*ident,
            content: &*content,
            trigger: &*trigger
        ];

        let Some(center_cls) = AnyClass::get(c"UNUserNotificationCenter") else {
            return;
        };
        let center: Retained<AnyObject> = msg_send![center_cls, currentNotificationCenter];

        let _: () = msg_send![
            &*center,
            addNotificationRequest: &*request,
            withCompletionHandler: std::ptr::null::<AnyObject>()
        ];
    }
}
