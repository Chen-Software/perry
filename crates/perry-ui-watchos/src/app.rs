//! App lifecycle for watchOS.
//!
//! Since watchOS uses SwiftUI's @main App, the actual app lifecycle is managed
//! by the fixed PerryWatchApp.swift. This module stores config and provides
//! the entry point that Swift calls to run the compiled TypeScript init code.

use std::cell::RefCell;

use crate::tree::{self, NodeData, NodeKind};

thread_local! {
    static PENDING_BODY: RefCell<Option<i64>> = RefCell::new(None);
}

pub fn app_create(_title_ptr: *const u8, _width: f64, _height: f64) -> i64 {
    // On watchOS, the app is created by the SwiftUI @main struct.
    // We just return a handle to satisfy the API contract.
    1
}

pub fn app_set_body(_app_handle: i64, root_handle: i64) {
    tree::set_root(root_handle);
    PENDING_BODY.with(|b| {
        *b.borrow_mut() = Some(root_handle);
    });
}

pub fn app_run(_app_handle: i64) {
    // On watchOS, the SwiftUI run loop is managed by PerryWatchApp.swift.
    // The compiled TypeScript calls perry_ui_app_run() at the end of init,
    // but on watchOS this is a no-op — the Swift @main struct drives the loop.
    //
    // perry_main_init() is called from Swift before the app body is rendered,
    // so by the time SwiftUI queries the tree, it's fully built.
}
