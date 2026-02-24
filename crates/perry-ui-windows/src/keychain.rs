//! Keychain — file-based key-value store in %APPDATA%/Perry/keychain (Win32)
//! Delegates to the existing keychain implementation in system.rs.

/// Save a value to the keychain.
pub fn save(key_ptr: *const u8, value_ptr: *const u8) {
    crate::system::keychain_save(key_ptr, value_ptr);
}

/// Get a value from the keychain. Returns NaN-boxed string or TAG_UNDEFINED.
pub fn get(key_ptr: *const u8) -> f64 {
    crate::system::keychain_get(key_ptr)
}

/// Delete a value from the keychain.
pub fn delete(key_ptr: *const u8) {
    crate::system::keychain_delete(key_ptr);
}
