use perry_container_compose::service::*;
use perry_container_compose::types::ComposeService;
use std::collections::HashMap;

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_generate_name_format() {
    let name = generate_name("redis:alpine", "cache");
    // Format: {service_name}-{md5_8chars}-{random_hex}
    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], "cache");
    assert_eq!(parts[1].len(), 8);
    assert_eq!(parts[2].len(), 8);
}

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_same_image_same_hash_prefix() {
    let name1 = generate_name("postgres:16", "db1");
    let name2 = generate_name("postgres:16", "db2");
    let parts1: Vec<&str> = name1.split('-').collect();
    let parts2: Vec<&str> = name2.split('-').collect();
    assert_eq!(parts1[1], parts2[1]);
}

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_sanitize_service_name() {
    let name = generate_name("nginx", "web.site!");
    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts[0], "web_site_");
}
