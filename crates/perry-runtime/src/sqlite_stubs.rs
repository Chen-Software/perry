#[cfg(not(feature = "stdlib"))]
mod sqlite_stubs_impl {
    #[no_mangle] pub extern "C" fn js_sqlite_transaction() -> i64 { 0 }
    #[no_mangle] pub extern "C" fn js_sqlite_transaction_commit() -> i64 { 0 }
    #[no_mangle] pub extern "C" fn js_sqlite_transaction_rollback() -> i64 { 0 }
}
