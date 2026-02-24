//! Keychain — delegates to system.rs

pub fn save(key_ptr: *const u8, value_ptr: *const u8) {
    crate::system::keychain_save(key_ptr, value_ptr);
}

pub fn get(key_ptr: *const u8) -> f64 {
    crate::system::keychain_get(key_ptr)
}

pub fn delete(key_ptr: *const u8) {
    crate::system::keychain_delete(key_ptr);
}
