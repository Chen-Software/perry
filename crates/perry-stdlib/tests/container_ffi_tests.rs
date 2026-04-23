use perry_runtime::StringHeader;
use perry_stdlib::js_container_getBackend;
use std::ptr;

#[test]
fn test_js_container_get_backend_unknown() {
    // Before any detection, it should be "unknown"
    let header_ptr = js_container_getBackend();
    assert!(!header_ptr.is_null());
    unsafe {
        let header = &*header_ptr;
        let bytes = std::slice::from_raw_parts(header_ptr.add(1) as *const u8, header.byte_len as usize);
        let name = String::from_utf8_lossy(bytes);
        assert_eq!(name, "unknown");
    }
}
