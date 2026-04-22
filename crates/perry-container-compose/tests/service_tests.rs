//! Unit tests for service name generation and state tracking.

use perry_container_compose::service::*;
use perry_container_compose::types::ComposeService;

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_generate_name_format() {
    let name = generate_name("nginx:latest", "web");
    let parts: Vec<&str> = name.split('_').collect();
    // {service_name}_{short_hash}{random_suffix_hex}
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "web");
    assert_eq!(parts[1].len(), 16);
}

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_generate_name_sanitization() {
    let name = generate_name("img", "my.service");
    // Format: {safe_name}_{hash+random}
    // "my.service" becomes "my_service"
    assert!(name.starts_with("my_service_"));
}

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_service_container_name_explicit() {
    let mut svc = ComposeService::default();
    svc.container_name = Some("custom-name".to_string());
    let name = service_container_name(&svc, "web");
    assert_eq!(name, "custom-name");
}

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_service_container_name_generated() {
    let svc = ComposeService::default();
    let name = service_container_name(&svc, "web");
    assert!(name.starts_with("web_"));
}

/*
Coverage Table:
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 6.13        | test_generate_name_format | unit |
| 6.13        | test_generate_name_sanitization | unit |
| 6.13        | test_service_container_name_explicit | unit |
| 6.13        | test_service_container_name_generated | unit |

Deferred Requirements:
- none
*/
